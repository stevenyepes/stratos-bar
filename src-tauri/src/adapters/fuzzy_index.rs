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

fn score_app(app: &AppEntry, query: &str) -> Option<ScoredEntry> {
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
    app_repository: Arc<dyn AppRepository>,
    history_repository: Arc<dyn HistoryRepository>,
    config_service: Arc<dyn ConfigService>,
    snapshot: RwLock<Option<Arc<IndexSnapshot>>>,
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
        }
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

    pub fn current_snapshot(&self) -> Option<Arc<IndexSnapshot>> {
        self.snapshot.read().ok().and_then(|g| g.clone())
    }

    pub fn rank(&self, snapshot: &IndexSnapshot, query: &str, limit: usize) -> Vec<DiscoverableItem> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let now_ms = current_time_ms();
        let mut scored: Vec<DiscoverableItem> = Vec::new();

        let mut recent_by_id: HashMap<String, &Action> = HashMap::new();
        for action in &snapshot.recent_actions {
            recent_by_id.insert(action.id.clone(), action);
        }

        for app in &snapshot.apps {
            if let Some(entry) = score_app(app, query) {
                let base = entry.raw();
                let recent = recent_by_id.get(&app.id).copied();
                let boost = frequency_boost(recent, now_ms);
                scored.push(DiscoverableItem {
                    kind: DiscoverableKind::App,
                    id: app.id.clone(),
                    name: app.name.clone(),
                    description: app.description.clone(),
                    icon: app.icon.clone(),
                    keywords: app.keywords.clone(),
                    score: (base + boost) * WEIGHT_APP,
                    source: format!("app:{:?}", app.source).to_lowercase(),
                    launch: DiscoverableLaunch::App {
                        exec: app.exec.clone(),
                    },
                });
            }
        }

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
                });
            }
        }

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
                });
            }
        }

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
                });
            }
        }

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
                });
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

    async fn search(&self, query: &str, limit: usize) -> Vec<DiscoverableItem> {
        let snap = match self.ensure_warm().await {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        self.rank(&snap, query, limit)
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
        assert!(score_app(&app, "").is_none());
        assert!(score_app(&app, "   ").is_none());
    }

    #[test]
    fn score_app_returns_none_for_no_match() {
        let app = make_app("chrome", "Chrome");
        assert!(score_app(&app, "xyzpdq").is_none());
    }

    #[test]
    fn name_prefix_outranks_name_contains() {
        let prefix = score_app(&make_app("code", "Code"), "code").unwrap();
        let contains = score_app(&make_app("vsc", "VisualStudioCode"), "code").unwrap();
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
        let ranked = adapter.rank(&snap, "chr", 10);
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
        let ranked = adapter.rank(&snap, "chr", 10);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].name, "Chrome");
    }

    #[tokio::test]
    async fn rank_caps_at_limit() {
        let apps: Vec<AppEntry> = (0..50).map(|i| make_app(&format!("app{i}"), &format!("App{i}"))).collect();
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "app", 5);
        assert_eq!(ranked.len(), 5);
    }

    #[tokio::test]
    async fn rank_respects_empty_query() {
        let apps = vec![make_app("chrome", "Chrome")];
        let adapter = build_adapter(apps, make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "", 10);
        assert!(ranked.is_empty());
        let ranked = adapter.rank(&snap, "   ", 10);
        assert!(ranked.is_empty());
    }

    #[tokio::test]
    async fn rank_finds_shortcut_by_trigger() {
        let adapter = build_adapter(vec![], make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "browser", 10);
        assert!(ranked.iter().any(|r| r.name == "browser" && matches!(r.kind, DiscoverableKind::Shortcut)));
    }

    #[tokio::test]
    async fn rank_finds_script_by_alias() {
        let adapter = build_adapter(vec![], make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "deploy", 10);
        assert!(ranked.iter().any(|r| matches!(r.kind, DiscoverableKind::Script)));
    }

    #[tokio::test]
    async fn rank_finds_ai_tool_by_keyword() {
        let adapter = build_adapter(vec![], make_config(), vec![]);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "rewrite", 10);
        assert!(ranked.iter().any(|r| matches!(r.kind, DiscoverableKind::AiTool)));
    }

    #[tokio::test]
    async fn rank_finds_recent_file() {
        let now = current_time_ms();
        let apps = vec![make_app("notes", "Notes")];
        let actions = vec![make_action("notes.txt", "file", 5, now)];
        let adapter = build_adapter(apps, make_config(), actions);
        let snap = adapter.build_snapshot().await.unwrap();
        let ranked = adapter.rank(&snap, "notes", 10);
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
        let ranked = adapter.rank(&snap, "o", 10);
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
        let ranked = adapter.rank(&snap, "1234", 50);
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
        let results = svc.search("chr", 10).await;
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
}