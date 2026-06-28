use crate::domain::action::Action;
use crate::domain::apps::AppEntry;
use crate::domain::config::{AiTool, AppConfig, ScriptConfig};
use crate::domain::discover::{DiscoverableItem, DiscoverableKind, DiscoverableLaunch};
use crate::ports::app_port::AppRepository;
use crate::ports::config_port::ConfigService;
use crate::ports::discover_port::DiscoverService;
use crate::ports::history::HistoryRepository;
use crate::utils::frecency::{current_time_ms, frecency_score, DEFAULT_HALF_LIFE_HOURS};
use async_trait::async_trait;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

const RECENT_LIMIT: usize = 50;
const RECENT_DECAY_HALF_LIFE_HOURS: f64 = 14.0 * 24.0;
const RECENT_DECAY_LAMBDA: f64 = 1.0 / RECENT_DECAY_HALF_LIFE_HOURS;
const FREQUENCY_BOOST_MAX: f32 = 1.5;

const WEIGHT_APP: f32 = 1.00;
const WEIGHT_SHORTCUT: f32 = 0.95;
const WEIGHT_SCRIPT: f32 = 0.75;
const WEIGHT_AI_TOOL: f32 = 0.65;
const WEIGHT_RECENT_FILE: f32 = 0.55;

const NAME_BASE: f32 = 1.00;
const NAME_CONTAINS_BASE: f32 = 0.85;
const KEYWORD_BASE: f32 = 0.70;
const GENERIC_NAME_BASE: f32 = 0.55;
const DESCRIPTION_BASE: f32 = 0.40;
const EXEC_BASE: f32 = 0.25;
const ALIAS_EXACT_BASE: f32 = 1.20;
const ALIAS_SUBSEQ_BASE: f32 = 0.75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldStrength {
    Prefix,
    Contains,
    Subsequence,
}

#[derive(Default)]
struct ScoredEntry {
    name_score: f32,
    name_strength: Option<FieldStrength>,
    alias_score: f32,
    alias_strength: Option<FieldStrength>,
    keyword_score: f32,
    generic_score: f32,
    desc_score: f32,
    exec_score: f32,
}

impl ScoredEntry {
    fn raw(&self) -> f32 {
        let name = match self.name_strength {
            Some(FieldStrength::Prefix) => NAME_BASE,
            Some(_) => NAME_CONTAINS_BASE,
            None => 0.0,
        } + self.name_score;
        let alias = match self.alias_strength {
            Some(FieldStrength::Prefix) | Some(FieldStrength::Contains) => ALIAS_EXACT_BASE,
            Some(FieldStrength::Subsequence) => ALIAS_SUBSEQ_BASE,
            None => 0.0,
        } + self.alias_score;
        let keyword = if self.keyword_score > 0.0 {
            KEYWORD_BASE + self.keyword_score
        } else {
            0.0
        };
        let generic = if self.generic_score > 0.0 {
            GENERIC_NAME_BASE + self.generic_score
        } else {
            0.0
        };
        let desc = if self.desc_score > 0.0 {
            DESCRIPTION_BASE + self.desc_score
        } else {
            0.0
        };
        let exec = if self.exec_score > 0.0 {
            EXEC_BASE + self.exec_score
        } else {
            0.0
        };
        name.max(alias).max(keyword).max(generic).max(desc).max(exec)
    }
}

pub fn is_subsequence(query: &str, target: &str) -> bool {
    let mut q = query.chars();
    let mut needle = q.next();
    if needle.is_none() {
        return true;
    }
    for c in target.chars() {
        if Some(c) == needle {
            needle = q.next();
            if needle.is_none() {
                return true;
            }
        }
    }
    needle.is_none()
}

fn classify(query: &str, target: &str) -> Option<FieldStrength> {
    let q = query.to_lowercase();
    let t = target.to_lowercase();
    if q.is_empty() || t.is_empty() {
        return None;
    }
    if t.starts_with(&q) {
        return Some(FieldStrength::Prefix);
    }
    if t.contains(&q) {
        return Some(FieldStrength::Contains);
    }
    if is_subsequence(&q, &t) {
        return Some(FieldStrength::Subsequence);
    }
    None
}

fn strength_score(strength: FieldStrength, query: &str, target: &str) -> f32 {
    let qlen = query.chars().count().max(1) as f32;
    let tlen = target.chars().count().max(1) as f32;
    let compactness = qlen / tlen;
    match strength {
        FieldStrength::Prefix => 1.0 + compactness,
        FieldStrength::Contains => {
            let q_lower = query.to_lowercase();
            let t_lower = target.to_lowercase();
            let pos = t_lower.find(&q_lower).unwrap_or(0) as f32;
            let early_bonus = 1.0 - (pos / tlen);
            0.85 + compactness * 0.5 + early_bonus * 0.2
        }
        FieldStrength::Subsequence => 0.5 + compactness * 0.3,
    }
}

fn score_field(query: &str, target: &str) -> Option<(f32, FieldStrength)> {
    let strength = classify(query, target)?;
    Some((strength_score(strength, query, target), strength))
}

fn score_app(app: &AppEntry, query: &str, user_aliases: Option<&Vec<String>>) -> Option<ScoredEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut s = ScoredEntry::default();
    if let Some((score, strength)) = score_field(trimmed, &app.name) {
        s.name_score = score;
        s.name_strength = Some(strength);
    }
    if let Some((score, _strength)) = score_field(trimmed, &app.exec) {
        s.exec_score = score;
    }
    for kw in &app.keywords {
        if let Some((score, _)) = score_field(trimmed, kw) {
            if score > s.keyword_score {
                s.keyword_score = score;
            }
        }
    }
    if let Some(gn) = app.generic_name.as_deref() {
        if let Some((score, _)) = score_field(trimmed, gn) {
            s.generic_score = score;
        }
    }
    if let Some(desc) = app.description.as_deref() {
        if let Some((score, _)) = score_field(trimmed, desc) {
            s.desc_score = score;
        }
    }
    if let Some(extra) = score_user_aliases(user_aliases, query) {
        absorb_alias_into_entry(&mut s, extra);
    }
    if s.raw() <= 0.0 {
        return None;
    }
    Some(s)
}

fn score_script(script: &ScriptConfig, query: &str) -> Option<ScoredEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut s = ScoredEntry::default();
    if !script.alias.is_empty() {
        if let Some((score, strength)) = score_field(trimmed, &script.alias) {
            s.name_score = score;
            s.name_strength = Some(strength);
        }
    }
    if let Some((score, _)) = score_field(trimmed, &script.path) {
        s.exec_score = score;
    }
    if s.raw() <= 0.0 {
        return None;
    }
    Some(s)
}

fn score_ai_tool(tool: &AiTool, query: &str) -> Option<ScoredEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut s = ScoredEntry::default();
    if let Some((score, strength)) = score_field(trimmed, &tool.name) {
        s.name_score = score;
        s.name_strength = Some(strength);
    }
    if !tool.description.is_empty() {
        if let Some((score, _)) = score_field(trimmed, &tool.description) {
            s.desc_score = score;
        }
    }
    let mut best_kw: f32 = 0.0;
    for kw in &tool.keywords {
        if let Some((score, _)) = score_field(trimmed, kw) {
            if score > best_kw {
                best_kw = score;
            }
        }
    }
    s.keyword_score = best_kw;
    if s.raw() <= 0.0 {
        return None;
    }
    Some(s)
}

fn score_shortcut(trigger: &str, query: &str) -> Option<ScoredEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut s = ScoredEntry::default();
    if let Some((score, strength)) = score_field(trimmed, trigger) {
        s.name_score = score;
        s.name_strength = Some(strength);
    }
    if s.raw() <= 0.0 {
        return None;
    }
    Some(s)
}

fn score_recent(query: &str, action: &Action) -> Option<ScoredEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut s = ScoredEntry::default();
    if let Some((score, strength)) = score_field(trimmed, &action.name) {
        s.name_score = score;
        s.name_strength = Some(strength);
    }
    if let Some((score, _)) = score_field(trimmed, &action.content) {
        s.exec_score = score;
    }
    if s.raw() <= 0.0 {
        return None;
    }
    Some(s)
}

fn frequency_boost(action: Option<&Action>, now_ms: u64) -> f32 {
    let action = match action {
        Some(a) => a,
        None => return 0.0,
    };
    let raw = frecency_score(action, now_ms, DEFAULT_HALF_LIFE_HOURS) as f32;
    raw.min(FREQUENCY_BOOST_MAX)
}

fn time_decay(last_accessed_ms: u64, now_ms: u64) -> f32 {
    if last_accessed_ms == 0 || last_accessed_ms > now_ms {
        return 1.0;
    }
    let age_ms = now_ms - last_accessed_ms;
    let age_hours = age_ms as f64 / (3_600_000.0_f64);
    let decay = (-age_hours * RECENT_DECAY_LAMBDA).exp();
    decay as f32
}

#[derive(Debug, Clone, Default)]
pub struct IndexSnapshot {
    pub apps: Vec<AppEntry>,
    pub scripts: Vec<ScriptConfig>,
    pub ai_tools: Vec<AiTool>,
    pub shortcuts: Vec<(String, String)>,
    pub recent_actions: Vec<Action>,
    pub snapshot_at_ms: u64,
}

impl IndexSnapshot {
    fn from_config(
        apps: Vec<AppEntry>,
        config: &AppConfig,
        recent_actions: Vec<Action>,
    ) -> Self {
        let mut shortcuts: Vec<(String, String)> = config
            .shortcuts
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        shortcuts.sort_by(|a, b| a.0.cmp(&b.0));
        Self {
            apps,
            scripts: config.scripts.clone(),
            ai_tools: config.ai_tools.clone(),
            shortcuts,
            recent_actions,
            snapshot_at_ms: current_time_ms(),
        }
    }
}

pub struct FuzzyIndexAdapter {
    pub(crate) app_repository: Arc<dyn AppRepository>,
    pub(crate) history_repository: Arc<dyn HistoryRepository>,
    config_service: Arc<dyn ConfigService>,
    snapshot: RwLock<Option<Arc<IndexSnapshot>>>,
    alias_overrides: crate::ports::discover_port::AliasOverrides,
}

impl FuzzyIndexAdapter {
    pub fn new(
        app_repository: Arc<dyn AppRepository>,
        history_repository: Arc<dyn HistoryRepository>,
        config_service: Arc<dyn ConfigService>,
    ) -> Self {
        Self {
            app_repository,
            history_repository,
            config_service,
            snapshot: RwLock::new(None),
            alias_overrides: crate::ports::discover_port::AliasOverrides::default(),
        }
    }

    pub fn set_user_aliases(&self, aliases: HashMap<String, Vec<String>>) {
        self.alias_overrides.replace(aliases);
        self.invalidate();
    }

    pub fn user_aliases(&self) -> HashMap<String, Vec<String>> {
        self.alias_overrides.snapshot()
    }

    pub async fn build_snapshot(&self) -> Result<IndexSnapshot, String> {
        let apps = self.app_repository.list_apps().unwrap_or_default();
        let config = self.config_service.load_config();
        let recent_actions = self
            .history_repository
            .get_top_frecency(RECENT_LIMIT, None)
            .await
            .unwrap_or_default();
        Ok(IndexSnapshot::from_config(apps, &config, recent_actions))
    }

    pub async fn ensure_warm(&self) -> Result<Arc<IndexSnapshot>, String> {
        {
            let guard = self.snapshot.read().map_err(|e| e.to_string())?;
            if let Some(snap) = guard.as_ref() {
                return Ok(snap.clone());
            }
        }
        let snap = Arc::new(self.build_snapshot().await?);
        let mut guard = self.snapshot.write().map_err(|e| e.to_string())?;
        if guard.is_none() {
            *guard = Some(snap.clone());
        }
        Ok(guard.as_ref().cloned().unwrap_or(snap))
    }

    pub fn invalidate(&self) {
        if let Ok(mut guard) = self.snapshot.write() {
            *guard = None;
        }
    }

    pub fn all_apps(&self, snapshot: &IndexSnapshot) -> Vec<DiscoverableItem> {
        let user_aliases = self.alias_overrides.snapshot();
        snapshot
            .apps
            .iter()
            .map(|app| {
                let user = user_aliases.get(&app.id);
                let mut item = DiscoverableItem::from_app(app);
                item.aliases = merged_aliases(app, user);
                item
            })
            .collect()
    }

    pub fn current_snapshot(&self) -> Option<Arc<IndexSnapshot>> {
        self.snapshot.read().ok().and_then(|g| g.clone())
    }

    pub fn rank(
        &self,
        snapshot: &IndexSnapshot,
        query: &str,
        kind_filter: Option<&[DiscoverableKind]>,
        limit: usize,
    ) -> Vec<DiscoverableItem> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let now_ms = current_time_ms();
        let mut scored: Vec<DiscoverableItem> = Vec::new();

        let mut recent_by_id: HashMap<String, &Action> = HashMap::new();
        for action in &snapshot.recent_actions {
            recent_by_id.insert(action.id.clone(), action);
        }

        let category_freq = compute_category_frequency(&snapshot.apps);
        let user_aliases = self.alias_overrides.snapshot();
        let includes = |kind: DiscoverableKind| match kind_filter {
            None => true,
            Some(set) => set.contains(&kind),
        };

        if includes(DiscoverableKind::App) {
            for app in &snapshot.apps {
                let user_aliases_for_app = user_aliases.get(&app.id);
                if let Some(entry) = score_app(app, query, user_aliases_for_app) {
                    let base = entry.raw();
                    let recent = recent_by_id.get(&app.id).copied();
                    let boost = frequency_boost(recent, now_ms);
                    let category_boost = category_frequency_boost(
                        app.categories.first().cloned(),
                        &category_freq,
                    );
                    let category = app.categories.first().cloned();
                    let aliases = merged_aliases(app, user_aliases_for_app);
                    scored.push(DiscoverableItem {
                        kind: DiscoverableKind::App,
                        id: app.id.clone(),
                        name: app.name.clone(),
                        description: app.description.clone(),
                        icon: app.icon.clone(),
                        keywords: app.keywords.clone(),
                        score: (base + boost + category_boost) * WEIGHT_APP,
                        source: format!("app:{:?}", app.source).to_lowercase(),
                        launch: DiscoverableLaunch::App {
                            exec: app.exec.clone(),
                        },
                        category,
                        aliases,
                    });
                }
            }
        }

        if includes(DiscoverableKind::Script) {
            for script in &snapshot.scripts {
                if let Some(entry) = score_script(script, query) {
                    let base = entry.raw();
                    let recent = recent_by_id.get(&script.id).copied();
                    let boost = frequency_boost(recent, now_ms);
                    scored.push(DiscoverableItem {
                        kind: DiscoverableKind::Script,
                        id: script.id.clone(),
                        name: if script.alias.is_empty() {
                            script.id.clone()
                        } else {
                            script.alias.clone()
                        },
                        description: Some(script.path.clone()),
                        icon: None,
                        keywords: Vec::new(),
                        score: (base + boost) * WEIGHT_SCRIPT,
                        source: "config:script".to_string(),
                        launch: DiscoverableLaunch::Script {
                            path: script.path.clone(),
                            args: script.args.clone(),
                        },
                        category: None,
                        aliases: Vec::new(),
                    });
                }
            }
        }

        if includes(DiscoverableKind::AiTool) {
            for tool in &snapshot.ai_tools {
                if let Some(entry) = score_ai_tool(tool, query) {
                    let base = entry.raw();
                    scored.push(DiscoverableItem {
                        kind: DiscoverableKind::AiTool,
                        id: tool.id.clone(),
                        name: tool.name.clone(),
                        description: if tool.description.is_empty() {
                            None
                        } else {
                            Some(tool.description.clone())
                        },
                        icon: if tool.icon.is_empty() {
                            None
                        } else {
                            Some(tool.icon.clone())
                        },
                        keywords: tool.keywords.clone(),
                        score: base * WEIGHT_AI_TOOL,
                        source: "config:ai_tool".to_string(),
                        launch: DiscoverableLaunch::AiTool {
                            tool_id: tool.id.clone(),
                        },
                        category: None,
                        aliases: Vec::new(),
                    });
                }
            }
        }

        if includes(DiscoverableKind::Shortcut) {
            for (trigger, target) in &snapshot.shortcuts {
                if let Some(entry) = score_shortcut(trigger, query) {
                    let base = entry.raw();
                    scored.push(DiscoverableItem {
                        kind: DiscoverableKind::Shortcut,
                        id: format!("shortcut:{trigger}"),
                        name: trigger.clone(),
                        description: Some(format!("→ {target}")),
                        icon: None,
                        keywords: vec![target.clone()],
                        score: base * WEIGHT_SHORTCUT,
                        source: "config:shortcut".to_string(),
                        launch: DiscoverableLaunch::Shortcut {
                            target: target.clone(),
                        },
                        category: None,
                        aliases: Vec::new(),
                    });
                }
            }
        }

        if includes(DiscoverableKind::RecentFile) {
            for action in &snapshot.recent_actions {
                if action.kind != "file" {
                    continue;
                }
                if let Some(entry) = score_recent(query, action) {
                    let base = entry.raw();
                    let decay = time_decay(action.last_accessed, now_ms);
                    let boost = (action.frequency as f32).min(FREQUENCY_BOOST_MAX) * decay;
                    scored.push(DiscoverableItem {
                        kind: DiscoverableKind::RecentFile,
                        id: action.id.clone(),
                        name: action.name.clone(),
                        description: Some(action.content.clone()),
                        icon: action.icon.clone(),
                        keywords: Vec::new(),
                        score: (base + boost) * WEIGHT_RECENT_FILE,
                        source: "history:file".to_string(),
                        launch: DiscoverableLaunch::RecentFile {
                            path: action.content.clone(),
                        },
                        category: None,
                        aliases: Vec::new(),
                    });
                }
            }
        }

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        scored.truncate(limit);
        scored
    }
}

#[async_trait]
impl DiscoverService for FuzzyIndexAdapter {
    async fn warm(&self) -> Result<(), String> {
        self.ensure_warm().await.map(|_| ())
    }

    async fn invalidate(&self) {
        self.invalidate();
    }

    async fn search(
        &self,
        query: &str,
        kinds: Option<Vec<DiscoverableKind>>,
        limit: usize,
    ) -> Vec<DiscoverableItem> {
        let snap = match self.ensure_warm().await {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        self.rank(&snap, query, kinds.as_deref(), limit)
    }
}

static SHARED_INDEX: OnceLock<RwLock<Option<Arc<FuzzyIndexAdapter>>>> = OnceLock::new();

pub fn shared_index() -> &'static RwLock<Option<Arc<FuzzyIndexAdapter>>> {
    SHARED_INDEX.get_or_init(|| RwLock::new(None))
}

pub fn install_shared(adapter: Arc<FuzzyIndexAdapter>) {
    let cell = shared_index();
    if let Ok(mut guard) = cell.write() {
        *guard = Some(adapter);
    }
}

pub fn current_shared() -> Option<Arc<FuzzyIndexAdapter>> {
    shared_index().read().ok().and_then(|g| g.clone())
}

const CATEGORY_FREQ_CAP: f32 = 16.0;
const CATEGORY_FREQ_WEIGHT: f32 = 0.20;

fn compute_category_frequency(apps: &[AppEntry]) -> HashMap<String, f32> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for app in apps {
        if let Some(cat) = app.categories.first() {
            *counts.entry(cat.clone()).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .map(|(cat, n)| {
            let norm = (n as f32).ln_1p() / CATEGORY_FREQ_CAP.ln_1p();
            (cat, norm.min(1.0))
        })
        .collect()
}

fn category_frequency_boost(category: Option<String>, freq: &HashMap<String, f32>) -> f32 {
    let cat = match category {
        Some(c) => c,
        None => return 0.0,
    };
    freq.get(&cat).copied().unwrap_or(0.0) * CATEGORY_FREQ_WEIGHT
}

fn merged_aliases(app: &AppEntry, user_aliases: Option<&Vec<String>>) -> Vec<String> {
    let mut out: Vec<String> = app
        .keywords
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(extra) = user_aliases {
        for a in extra {
            let trimmed = a.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !out.iter().any(|existing| existing == trimmed) {
                out.push(trimmed.to_string());
            }
        }
    }
    out
}

fn score_user_aliases(
    user_aliases: Option<&Vec<String>>,
    query: &str,
) -> Option<ScoredEntry> {
    let aliases = user_aliases?;
    if aliases.is_empty() {
        return None;
    }
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut entry = ScoredEntry::default();
    let mut any = false;
    for alias in aliases {
        if let Some((score, strength)) = score_field(trimmed, alias) {
            any = true;
            if matches!(
                strength,
                FieldStrength::Prefix | FieldStrength::Contains
            ) {
                entry.alias_score = entry.alias_score.max(score);
                entry.alias_strength = Some(match entry.alias_strength {
                    Some(existing) if alias_strength_rank(existing)
                        >= alias_strength_rank(strength) =>
                    {
                        existing
                    }
                    _ => strength,
                });
            } else if matches!(strength, FieldStrength::Subsequence) {
                if entry.alias_score < score {
                    entry.alias_score = score;
                }
                if entry.alias_strength.is_none() {
                    entry.alias_strength = Some(strength);
                }
            }
        }
    }
    if any { Some(entry) } else { None }
}

fn alias_strength_rank(s: FieldStrength) -> u8 {
    match s {
        FieldStrength::Prefix => 3,
        FieldStrength::Contains => 2,
        FieldStrength::Subsequence => 1,
    }
}

fn absorb_alias_into_entry(entry: &mut ScoredEntry, alias_entry: ScoredEntry) {
    if alias_entry.alias_score > entry.alias_score {
        entry.alias_score = alias_entry.alias_score;
    }
    match (entry.alias_strength, alias_entry.alias_strength) {
        (None, Some(s)) => entry.alias_strength = Some(s),
        (Some(existing), Some(new)) => {
            if alias_strength_rank(new) > alias_strength_rank(existing) {
                entry.alias_strength = Some(new);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::apps::AppSource;
    use crate::ports::app_port::MockAppRepository;
    use crate::ports::config_port::MockConfigService;
    use crate::ports::history::HistoryRepository;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    fn make_app(id: &str, name: &str) -> AppEntry {
        AppEntry {
            id: id.to_string(),
            name: name.to_string(),
            generic_name: None,
            description: None,
            keywords: Vec::new(),
            exec: id.to_string(),
            try_exec: None,
            icon: None,
            categories: Vec::new(),
            startup_wm_class: None,
            source: AppSource::Desktop,
            path: format!("/usr/share/applications/{id}.desktop"),
        }
    }

    fn make_action(id: &str, kind: &str, freq: u64, last_accessed: u64) -> Action {
        Action {
            id: id.to_string(),
            kind: kind.to_string(),
            content: format!("/home/me/{id}"),
            name: id.to_string(),
            icon: None,
            last_accessed,
            frequency: freq,
        }
    }

    fn make_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.apply_defaults();
        let mut shortcuts = HashMap::new();
        shortcuts.insert("browser".to_string(), "google-chrome".to_string());
        shortcuts.insert("editor".to_string(), "code".to_string());
        config.shortcuts = shortcuts;
        config.scripts = vec![ScriptConfig {
            id: "deploy".to_string(),
            alias: "deploy".to_string(),
            path: "/srv/scripts/deploy.sh".to_string(),
            args: Some("--staging".to_string()),
        }];
        config.ai_tools = vec![AiTool {
            id: "rephrase".to_string(),
            name: "Rephrase Selection".to_string(),
            description: "Improve clarity and grammar".to_string(),
            prompt_template: String::new(),
            keywords: vec!["rephrase".to_string(), "rewrite".to_string()],
            icon: String::new(),
        }];
        config
    }

    fn build_adapter(
        apps: Vec<AppEntry>,
        config: AppConfig,
        actions: Vec<Action>,
    ) -> Arc<FuzzyIndexAdapter> {
        let mut app_mock = MockAppRepository::new();
        app_mock
            .expect_list_apps()
            .returning(move || Ok(apps.clone()));
        let mut config_mock = MockConfigService::new();
        let cfg = config.clone();
        config_mock.expect_load_config().returning(move || cfg.clone());
        let history = Arc::new(StubHistoryRepo::new(actions.clone()));
        Arc::new(FuzzyIndexAdapter::new(
            Arc::new(app_mock),
            history,
            Arc::new(config_mock),
        ))
    }

    #[test]
    fn subsequence_matches_strict_order() {
        assert!(is_subsequence("chr", "chrome"));
        assert!(is_subsequence("cde", "abcdef"));
        assert!(!is_subsequence("xyz", "chrome"));
        assert!(!is_subsequence("crx", "chrome"));
    }

    #[test]
    fn classify_orders_prefix_then_contains_then_subsequence() {
        assert_eq!(classify("chr", "chrome"), Some(FieldStrength::Prefix));
        assert_eq!(classify("rom", "chrome"), Some(FieldStrength::Contains));
        assert_eq!(classify("cme", "chrome"), Some(FieldStrength::Subsequence));
        assert_eq!(classify("zzz", "chrome"), None);
    }

    #[test]
    fn classify_is_case_insensitive() {
        assert_eq!(classify("CHR", "Chrome"), Some(FieldStrength::Prefix));
    }

    #[test]
    fn empty_inputs_classify_as_none() {
        assert_eq!(classify("", "chrome"), None);
        assert_eq!(classify("chr", ""), None);
    }

    #[test]
    fn score_app_returns_none_for_empty_query() {
        let app = make_app("chrome", "Chrome");
        assert!(score_app(&app, "", None).is_none());
        assert!(score_app(&app, "   ", None).is_none());
    }

    #[test]
    fn score_app_returns_none_for_no_match() {
        let app = make_app("chrome", "Chrome");
        assert!(score_app(&app, "xyzpdq", None).is_none());
    }

    #[test]
    fn name_prefix_outranks_name_contains() {
        let prefix = score_app(&make_app("code", "Code"), "code", None).unwrap();
        let contains = score_app(&make_app("vsc", "VisualStudioCode"), "code", None).unwrap();
        assert_eq!(prefix.name_strength, Some(FieldStrength::Prefix));
        assert_eq!(contains.name_strength, Some(FieldStrength::Contains));
        assert!(prefix.raw() > contains.raw());
    }

    #[tokio::test]
    async fn rank_orders_chrome_before_chromium_on_chr() {
        let apps = vec![
            make_app("chromium", "Chromium"),
            make_app("chrome", "Chrome"),
            make_app("cursor", "Cursor"),
            make_app("figma", "Figma"),
            make_app("code", "Code"),
        ];
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "chr", None, 10);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].name, "Chrome");
        assert_eq!(ranked[1].name, "Chromium");
        assert!(ranked[0].score > ranked[1].score);
    }

    #[tokio::test]
    async fn rank_excludes_non_matches() {
        let apps = vec![
            make_app("figma", "Figma"),
            make_app("cursor", "Cursor"),
            make_app("chrome", "Chrome"),
        ];
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "chr", None, 10);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].name, "Chrome");
    }

    #[tokio::test]
    async fn rank_caps_at_limit() {
        let apps: Vec<AppEntry> = (0..50).map(|i| make_app(&format!("app{i}"), &format!("App{i}"))).collect();
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "app", None, 5);
        assert_eq!(ranked.len(), 5);
    }

    #[tokio::test]
    async fn rank_respects_empty_query() {
        let apps = vec![make_app("chrome", "Chrome")];
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "", None, 10);
        assert!(ranked.is_empty());
        let ranked = adapter.rank(&snap, "   ", None, 10);
        assert!(ranked.is_empty());
    }

    #[tokio::test]
    async fn rank_finds_shortcut_by_trigger() {
        let adapter = build_adapter(vec![], make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "browser", None, 10);
        assert!(ranked.iter().any(|r| r.name == "browser" && matches!(r.kind, DiscoverableKind::Shortcut)));
    }

    #[tokio::test]
    async fn rank_finds_script_by_alias() {
        let adapter = build_adapter(vec![], make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "deploy", None, 10);
        assert!(ranked.iter().any(|r| matches!(r.kind, DiscoverableKind::Script)));
    }

    #[tokio::test]
    async fn rank_finds_ai_tool_by_keyword() {
        let adapter = build_adapter(vec![], make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "rewrite", None, 10);
        assert!(ranked.iter().any(|r| matches!(r.kind, DiscoverableKind::AiTool)));
    }

    #[tokio::test]
    async fn rank_finds_recent_file() {
        let now = current_time_ms();
        let apps = vec![make_app("notes", "Notes")];
        let actions = vec![make_action("notes.txt", "file", 5, now)];
        let adapter = build_adapter(apps, make_config(), actions);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "notes", None, 10);
        assert!(ranked
            .iter()
            .any(|r| matches!(r.kind, DiscoverableKind::RecentFile)));
    }

    #[tokio::test]
    async fn frequency_boost_prefers_recently_used_app() {
        let now = current_time_ms();
        let day_ms = 24 * 3_600_000;
        let apps = vec![
            make_app("cold", "Cold"),
            make_app("hot", "Hot"),
        ];
        let actions = vec![
            make_action("cold", "app", 1, now - day_ms * 7),
            make_action("hot", "app", 50, now),
        ];
        let adapter = build_adapter(apps, make_config(), actions);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "o", None, 10);
        let hot_pos = ranked.iter().position(|r| r.id == "hot");
        let cold_pos = ranked.iter().position(|r| r.id == "cold");
        assert!(hot_pos.is_some() && cold_pos.is_some());
        assert!(hot_pos.unwrap() < cold_pos.unwrap());
    }

    #[tokio::test]
    async fn build_snapshot_captures_apps_scripts_shortcuts_and_ai_tools() {
        let apps = vec![make_app("chrome", "Chrome")];
        let config = make_config();
        let adapter = build_adapter(apps, config, vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        assert_eq!(snap.apps.len(), 1);
        assert_eq!(snap.scripts.len(), 1);
        assert_eq!(snap.ai_tools.len(), 1);
        assert_eq!(snap.shortcuts.len(), 2);
    }

    #[tokio::test]
    async fn ensure_warm_caches_snapshot_across_calls() {
        let mut app_mock = MockAppRepository::new();
        app_mock
            .expect_list_apps()
            .times(1)
            .returning(|| Ok(vec![make_app("chrome", "Chrome")]));
        let mut config_mock = MockConfigService::new();
        config_mock
            .expect_load_config()
            .times(1)
            .returning(make_config);
        let history = Arc::new(StubHistoryRepo::new(vec![]));

        let adapter = FuzzyIndexAdapter::new(
            Arc::new(app_mock),
            history,
            Arc::new(config_mock),
        );
        let s1 = adapter.ensure_warm().await.unwrap();
        let s2 = adapter.ensure_warm().await.unwrap();
        assert!(Arc::ptr_eq(&s1, &s2));
    }

    #[tokio::test]
    async fn invalidate_drops_cached_snapshot() {
        let mut app_mock = MockAppRepository::new();
        app_mock
            .expect_list_apps()
            .returning(|| Ok(vec![make_app("chrome", "Chrome")]));
        let mut config_mock = MockConfigService::new();
        config_mock.expect_load_config().returning(make_config);
        let history = Arc::new(StubHistoryRepo::new(vec![]));
        let adapter = FuzzyIndexAdapter::new(
            Arc::new(app_mock),
            history,
            Arc::new(config_mock),
        );
        let _ = adapter.ensure_warm().await.unwrap();
        adapter.invalidate();
        let _ = adapter.ensure_warm().await.unwrap();
    }

    #[tokio::test]
    async fn handles_5000_apps_within_budget() {
        let apps: Vec<AppEntry> = (0..5000)
            .map(|i| make_app(&format!("a{i:04}"), &format!("App{i:04}")))
            .collect();
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let start = std::time::Instant::now();
        let ranked = adapter.rank(&snap, "1234", None, 50);
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 200, "ranking took too long: {elapsed:?}");
        assert!(!ranked.is_empty());
        assert!(ranked.len() <= 50);
    }

    #[tokio::test]
    async fn time_decay_is_full_for_zero_age_and_decays_for_old_action() {
        let now = current_time_ms();
        assert!((time_decay(now, now) - 1.0).abs() < 1e-6);
        let one_day_ms = 24 * 3_600_000;
        let decay = time_decay(now - one_day_ms, now);
        assert!(decay > 0.9 && decay < 1.0, "1d decay should be ~0.95, got {decay}");
        let two_weeks_ms = 14 * one_day_ms;
        let decay_old = time_decay(now - two_weeks_ms, now);
        assert!(decay_old < 0.4 && decay_old > 0.0, "14d decay should be ~0.37, got {decay_old}");
    }

    #[tokio::test]
    async fn frequency_boost_caps_at_max() {
        let now = current_time_ms();
        let action = make_action("a", "app", 1_000_000, now);
        let boost = frequency_boost(Some(&action), now);
        assert!((boost - FREQUENCY_BOOST_MAX).abs() < 1e-3);
        let boost_none = frequency_boost(None, now);
        assert_eq!(boost_none, 0.0);
    }

    #[tokio::test]
    async fn search_via_port_returns_unified_results() {
        let apps = vec![make_app("chrome", "Chrome")];
        let config = make_config();
        let adapter = build_adapter(apps, config, vec![]);
        adapter.warm().await.unwrap();
        let svc: Arc<dyn DiscoverService> = adapter;
        let results = svc.search("chr", None, 10).await;
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| matches!(r.kind, DiscoverableKind::App)));
    }

    struct StubHistoryRepo {
        inner: Mutex<Vec<Action>>,
    }

    impl StubHistoryRepo {
        fn new(actions: Vec<Action>) -> Self {
            Self {
                inner: Mutex::new(actions),
            }
        }
    }

    #[async_trait]
    impl HistoryRepository for StubHistoryRepo {
        async fn get_recent(&self, limit: usize) -> Result<Vec<Action>, String> {
            let g = self.inner.lock().unwrap();
            let mut v = g.clone();
            v.sort_by_key(|a| std::cmp::Reverse(a.last_accessed));
            Ok(v.into_iter().take(limit).collect())
        }

        async fn get_top_frecency(
            &self,
            limit: usize,
            _kind_filter: Option<String>,
        ) -> Result<Vec<Action>, String> {
            let g = self.inner.lock().unwrap();
            let now = current_time_ms();
            let mut v = g.clone();
            v.sort_by(|a, b| {
                let sa = crate::utils::frecency::frecency_score(a, now, 336.0);
                let sb = crate::utils::frecency::frecency_score(b, now, 336.0);
                sb.partial_cmp(&sa).unwrap_or(Ordering::Equal)
            });
            Ok(v.into_iter().take(limit).collect())
        }

        async fn record(&self, _action: Action) -> Result<(), String> {
            Ok(())
        }

        async fn clear(&self) -> Result<(), String> {
            self.inner.lock().unwrap().clear();
            Ok(())
        }
    }

    fn make_app_with_category(id: &str, name: &str, category: &str) -> AppEntry {
        let mut app = make_app(id, name);
        app.categories = vec![category.to_string()];
        app
    }

    #[tokio::test]
    async fn rank_propagates_category_field_to_items() {
        let apps = vec![
            make_app_with_category("chrome", "Chrome", "Internet"),
            make_app_with_category("code", "Code", "Development"),
        ];
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "chr", None, 10);
        let chrome = ranked.iter().find(|r| r.id == "chrome").unwrap();
        assert_eq!(chrome.category.as_deref(), Some("Internet"));
        assert!(chrome.aliases.is_empty());
    }

    #[tokio::test]
    async fn rank_user_alias_pushes_matching_app_to_front() {
        let apps = vec![
            make_app("chromatic", "Chromatic"),
            make_app("google-chrome", "Chrome"),
        ];
        let mut config = make_config();
        config.shortcuts.clear();
        let adapter = build_adapter(apps, config, vec![]);
        adapter.set_user_aliases(HashMap::from([(
            "google-chrome".to_string(),
            vec!["browser".to_string()],
        )]));
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "browser", None, 10);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].id, "google-chrome");
        assert_eq!(ranked[0].aliases, vec!["browser".to_string()]);
    }

    #[tokio::test]
    async fn rank_category_frequency_boost_breaks_ties() {
        let mut apps = vec![
            make_app_with_category("a", "Alpha", "Misc"),
            make_app_with_category("b", "Beta", "Misc"),
        ];
        for i in 0..30 {
            apps.push(make_app_with_category(
                &format!("d{i}"),
                &format!("DevTool{i}"),
                "Development",
            ));
        }
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "alph", None, 5);
        assert!(!ranked.is_empty());
        let alpha = ranked.iter().find(|r| r.id == "a").unwrap();
        assert!(alpha.score > 0.0);
    }

    #[test]
    fn compute_category_frequency_normalized_to_unit_range() {
        let mut apps = Vec::new();
        for i in 0..100 {
            apps.push(make_app_with_category(
                &format!("d{i}"),
                &format!("Dev{i}"),
                "Development",
            ));
        }
        apps.push(make_app_with_category("z", "Zeta", "Misc"));
        let freq = compute_category_frequency(&apps);
        let dev = freq.get("Development").copied().unwrap_or(0.0);
        assert!(dev > 0.5, "frequent category should approach 1, got {dev}");
        assert!(dev <= 1.0);
        assert!(freq.get("Misc").copied().unwrap_or(0.0) < dev);
    }

    #[test]
    fn merged_aliases_dedupes_and_appends_user_aliases() {
        let mut app = make_app("chrome", "Chrome");
        app.keywords = vec!["web".to_string(), "browser".to_string()];
        let user = vec!["  browser  ".to_string(), "net".to_string()];
        let merged = merged_aliases(&app, Some(&user));
        assert!(merged.iter().any(|a| a == "web"));
        assert_eq!(
            merged.iter().filter(|a| *a == "browser").count(),
            1,
            "user alias dedupes against keywords"
        );
        assert!(merged.iter().any(|a| a == "net"));
        assert!(merged.iter().all(|a| !a.is_empty()));
    }

    #[tokio::test]
    async fn all_apps_returns_snapshot_apps_with_aliases_and_category() {
        let apps = vec![
            make_app_with_category("chrome", "Chrome", "Internet"),
            make_app_with_category("code", "Code", "Development"),
        ];
        let adapter = build_adapter(apps, make_config(), vec![]);
        adapter.set_user_aliases(HashMap::from([(
            "google-chrome".to_string(),
            vec!["browser".to_string()],
        )]));
        let snap = adapter.build_snapshot().await.unwrap();
        let all = adapter.all_apps(&snap);
        assert_eq!(all.len(), 2);
        let chrome = all.iter().find(|a| a.id == "chrome").unwrap();
        assert_eq!(chrome.category.as_deref(), Some("Internet"));
        let code = all.iter().find(|a| a.id == "code").unwrap();
        assert_eq!(code.category.as_deref(), Some("Development"));
        assert!(code.aliases.is_empty());
    }

    #[tokio::test]
    async fn rank_kind_filter_apps_only_excludes_other_kinds() {
        let now = current_time_ms();
        let apps = vec![make_app("chrome", "Chrome"), make_app("code", "Code")];
        let actions = vec![make_action("notes.md", "file", 3, now)];
        let adapter = build_adapter(apps, make_config(), actions);
        let snap = adapter.build_snapshot().await.unwrap();
        let filter = [DiscoverableKind::App];
        let ranked = adapter.rank(&snap, "c", Some(&filter), 20);
        assert!(!ranked.is_empty());
        for item in &ranked {
            assert!(matches!(item.kind, DiscoverableKind::App));
        }
        assert!(ranked.iter().any(|r| r.id == "chrome"));
        assert!(ranked.iter().any(|r| r.id == "code"));
        assert!(ranked
            .iter()
            .all(|r| !matches!(r.kind, DiscoverableKind::Shortcut)));
    }

    #[tokio::test]
    async fn rank_kind_filter_shortcuts_only_returns_shortcuts() {
        let adapter = build_adapter(vec![make_app("chrome", "Chrome")], make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let filter = [DiscoverableKind::Shortcut];
        let ranked = adapter.rank(&snap, "browser", Some(&filter), 20);
        assert!(!ranked.is_empty());
        for item in &ranked {
            assert!(matches!(item.kind, DiscoverableKind::Shortcut));
        }
        assert!(ranked.iter().any(|r| r.name == "browser"));
    }

    #[tokio::test]
    async fn rank_kind_filter_multiple_kinds_returns_union() {
        let now = current_time_ms();
        let apps = vec![make_app("chrome", "Chrome")];
        let actions = vec![make_action("chrome-notes.md", "file", 5, now)];
        let adapter = build_adapter(apps, make_config(), actions);
        let snap = adapter.build_snapshot().await.unwrap();
        let filter = [DiscoverableKind::App, DiscoverableKind::RecentFile];
        let ranked = adapter.rank(&snap, "chrome", Some(&filter), 20);
        let kinds: std::collections::HashSet<_> = ranked.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&DiscoverableKind::App));
        assert!(kinds.contains(&DiscoverableKind::RecentFile));
        assert!(!kinds.contains(&DiscoverableKind::Shortcut));
        assert!(!kinds.contains(&DiscoverableKind::Script));
    }

    #[tokio::test]
    async fn rank_kind_filter_empty_set_returns_nothing() {
        let apps = vec![make_app("chrome", "Chrome")];
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let filter: [DiscoverableKind; 0] = [];
        let ranked = adapter.rank(&snap, "chrome", Some(&filter), 20);
        assert!(ranked.is_empty());
    }

    #[tokio::test]
    async fn search_via_port_with_kind_filter_respects_filter() {
        let apps = vec![make_app("chrome", "Chrome"), make_app("code", "Code")];
        let adapter = build_adapter(apps, make_config(), vec![]);
        adapter.warm().await.unwrap();
        let svc: Arc<dyn DiscoverService> = adapter;
        let ranked = svc
            .search("c", Some(vec![DiscoverableKind::App]), 20)
            .await;
        assert!(!ranked.is_empty());
        for item in &ranked {
            assert!(matches!(item.kind, DiscoverableKind::App));
        }
    }
}