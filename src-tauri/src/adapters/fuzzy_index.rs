use crate::domain::apps::AppEntry;
use crate::domain::discover::{DiscoverableItem, MatchedField, ScoredDiscoverable};
use crate::ports::discover_port::DiscoverService;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const ALIAS_EXACT_BASE: f32 = 1.40;
const ALIAS_PREFIX_BASE: f32 = 1.20;
const ALIAS_CONTAINS_BASE: f32 = 0.95;
const NAME_PREFIX_BASE: f32 = 1.00;
const NAME_CONTAINS_BASE: f32 = 0.85;
const KEYWORD_BASE: f32 = 0.70;
const CATEGORY_BASE: f32 = 0.55;
const DESCRIPTION_BASE: f32 = 0.40;
const EXEC_BASE: f32 = 0.25;

const CATEGORY_FREQUENCY_CAP: f32 = 16.0;
const CATEGORY_FREQUENCY_WEIGHT: f32 = 0.20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldStrength {
    Exact,
    Prefix,
    Contains,
    Subsequence,
}

fn is_subsequence(query: &str, target: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut chars = query.chars();
    let mut needle = chars.next();
    for c in target.chars() {
        if Some(c) == needle {
            needle = chars.next();
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
    if q == t {
        return Some(FieldStrength::Exact);
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

pub struct FuzzyIndexAdapter {
    items: Mutex<Vec<DiscoverableItem>>,
    user_aliases: Mutex<HashMap<String, Vec<String>>>,
}

impl FuzzyIndexAdapter {
    pub fn new(initial_apps: Vec<AppEntry>) -> Self {
        let items = initial_apps
            .iter()
            .map(DiscoverableItem::from_app)
            .collect();
        Self {
            items: Mutex::new(items),
            user_aliases: Mutex::new(HashMap::new()),
        }
    }

    pub fn replace_apps(&self, apps: Vec<AppEntry>) {
        let aliases = self.user_aliases.lock().expect("aliases poisoned").clone();
        let mut items: Vec<DiscoverableItem> = apps
            .iter()
            .map(|app| {
                let user = aliases.get(&app.id).cloned().unwrap_or_default();
                DiscoverableItem::from_app(app).with_user_aliases(&user)
            })
            .collect();
        items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        *self.items.lock().expect("items poisoned") = items;
    }

    pub fn set_user_aliases(&self, aliases: HashMap<String, Vec<String>>) {
        *self.user_aliases.lock().expect("aliases poisoned") = aliases.clone();
        let mut items = self.items.lock().expect("items poisoned");
        for item in items.iter_mut() {
            let user = aliases.get(&item.id).cloned().unwrap_or_default();
            item.aliases = item
                .keywords
                .iter()
                .chain(user.iter())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }

    pub fn category_frequency(&self) -> HashMap<String, f32> {
        let items = self.items.lock().expect("items poisoned");
        let mut counts: HashMap<String, usize> = HashMap::new();
        for item in items.iter() {
            if let Some(cat) = &item.category {
                *counts.entry(cat.clone()).or_insert(0) += 1;
            }
        }
        counts
            .into_iter()
            .map(|(k, v)| {
                let norm = (v as f32).ln_1p() / CATEGORY_FREQUENCY_CAP.ln_1p();
                (k, norm.min(1.0))
            })
            .collect()
    }

    pub fn rank(&self, query: &str, limit: usize) -> Vec<ScoredDiscoverable> {
        self.rank_with_category_freq(query, &self.category_frequency(), limit)
    }

    pub fn rank_with_category_freq(
        &self,
        query: &str,
        category_freq: &HashMap<String, f32>,
        limit: usize,
    ) -> Vec<ScoredDiscoverable> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }
        let items = self.items.lock().expect("items poisoned");
        let mut scored: Vec<ScoredDiscoverable> = items
            .iter()
            .filter_map(|item| score_item(item, trimmed, category_freq))
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.item.name.to_lowercase().cmp(&b.item.name.to_lowercase()))
        });
        if limit > 0 && scored.len() > limit {
            scored.truncate(limit);
        }
        scored
    }
}

fn score_item(
    item: &DiscoverableItem,
    query: &str,
    category_freq: &HashMap<String, f32>,
) -> Option<ScoredDiscoverable> {
    let mut best: Option<(f32, MatchedField)> = None;
    let mut consider = |raw_score: f32, field: MatchedField| {
        let boosted = apply_category_boost(item, raw_score, category_freq);
        match best {
            None => best = Some((boosted, field)),
            Some((current, _)) if boosted > current => best = Some((boosted, field)),
            _ => {}
        }
    };

    for alias in &item.aliases {
        match classify(query, alias) {
            Some(FieldStrength::Exact) => {
                let raw = ALIAS_EXACT_BASE + strength_score(FieldStrength::Exact, query, alias);
                consider(raw, MatchedField::AliasExact);
            }
            Some(FieldStrength::Prefix) => {
                let raw =
                    ALIAS_PREFIX_BASE + 0.4 + strength_score(FieldStrength::Prefix, query, alias);
                consider(raw, MatchedField::AliasPrefix);
            }
            Some(FieldStrength::Contains) => {
                let raw = ALIAS_CONTAINS_BASE + strength_score(FieldStrength::Contains, query, alias);
                consider(raw, MatchedField::AliasContains);
            }
            Some(FieldStrength::Subsequence) => {
                let raw = ALIAS_CONTAINS_BASE - 0.1
                    + strength_score(FieldStrength::Subsequence, query, alias);
                consider(raw, MatchedField::AliasContains);
            }
            None => {}
        }
    }

    if let Some(strength) = classify(query, &item.name) {
        let raw = base_score_for_name(strength, query, &item.name);
        let field = match strength {
            FieldStrength::Prefix | FieldStrength::Exact => MatchedField::NamePrefix,
            FieldStrength::Contains | FieldStrength::Subsequence => MatchedField::NameContains,
        };
        consider(raw, field);
    }

    for kw in &item.keywords {
        if let Some(strength) = classify(query, kw) {
            let raw = KEYWORD_BASE + strength_score(strength, query, kw) - KEYWORD_BASE;
            consider(raw, MatchedField::Keyword);
        }
    }

    if let Some(cat) = &item.category {
        if let Some(strength) = classify(query, cat) {
            let raw = CATEGORY_BASE + strength_score(strength, query, cat) - CATEGORY_BASE;
            consider(raw, MatchedField::Category);
        }
    }

    if let Some(desc) = &item.description {
        if let Some(strength) = classify(query, desc) {
            let raw = DESCRIPTION_BASE + strength_score(strength, query, desc) - DESCRIPTION_BASE;
            consider(raw, MatchedField::Description);
        }
    }

    if let Some(exec) = &item.exec {
        if let Some(strength) = classify(query, exec) {
            let raw = EXEC_BASE + strength_score(strength, query, exec) - EXEC_BASE;
            consider(raw, MatchedField::Exec);
        }
    }

    let (score, matched_field) = best?;
    Some(ScoredDiscoverable {
        item: item.clone(),
        score,
        matched_field,
    })
}

fn base_score_for_name(strength: FieldStrength, query: &str, target: &str) -> f32 {
    match strength {
        FieldStrength::Exact => NAME_PREFIX_BASE + 1.0,
        FieldStrength::Prefix => NAME_PREFIX_BASE + strength_score(strength, query, target) - 1.0,
        FieldStrength::Contains => {
            NAME_CONTAINS_BASE + strength_score(strength, query, target) - NAME_CONTAINS_BASE
        }
        FieldStrength::Subsequence => {
            NAME_CONTAINS_BASE + strength_score(strength, query, target) - NAME_CONTAINS_BASE
        }
    }
}

fn strength_score(strength: FieldStrength, query: &str, target: &str) -> f32 {
    let qlen = query.chars().count() as f32;
    let tlen = target.chars().count().max(1) as f32;
    let compactness = qlen / tlen;
    match strength {
        FieldStrength::Exact => 2.0,
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

fn apply_category_boost(
    item: &DiscoverableItem,
    raw_score: f32,
    category_freq: &HashMap<String, f32>,
) -> f32 {
    let cat = match &item.category {
        Some(c) => c,
        None => return raw_score,
    };
    let freq = category_freq.get(cat).copied().unwrap_or(0.0);
    raw_score + freq * CATEGORY_FREQUENCY_WEIGHT
}

impl DiscoverService for FuzzyIndexAdapter {
    fn search(&self, query: &str, limit: usize) -> Vec<ScoredDiscoverable> {
        self.rank(query, limit)
    }

    fn browse(
        &self,
        scripts: Vec<crate::domain::config::ScriptConfig>,
        ai_tools: Vec<crate::domain::config::AiTool>,
        recent_files: Vec<crate::domain::action::Action>,
        shortcuts: HashMap<String, String>,
    ) -> crate::domain::discover::BrowseSections {
        let items = self.items.lock().expect("items poisoned").clone();
        crate::domain::discover::BrowseSections::from_parts(
            items,
            scripts,
            ai_tools,
            recent_files,
            shortcuts,
        )
    }
}

pub type SharedFuzzyIndex = Arc<FuzzyIndexAdapter>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::apps::AppSource;

    fn fixture(id: &str, name: &str, categories: Vec<String>) -> AppEntry {
        AppEntry {
            id: id.to_string(),
            name: name.to_string(),
            generic_name: None,
            description: Some(format!("{name} description")),
            keywords: Vec::new(),
            exec: id.to_string(),
            try_exec: None,
            icon: None,
            categories,
            startup_wm_class: None,
            source: AppSource::Desktop,
            path: format!("/usr/share/applications/{id}.desktop"),
        }
    }

    #[test]
    fn rank_orders_name_prefix_matches_first() {
        let idx = FuzzyIndexAdapter::new(vec![
            fixture("chrome", "Chrome", vec!["Internet".to_string()]),
            fixture("chromium", "Chromium", vec!["Internet".to_string()]),
            fixture("figma", "Figma", vec!["Design".to_string()]),
        ]);
        let ranked = idx.rank("chr", 10);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].item.name, "Chrome");
        assert_eq!(ranked[1].item.name, "Chromium");
        assert_eq!(ranked[0].matched_field, MatchedField::NamePrefix);
    }

    #[test]
    fn rank_matches_against_user_alias() {
        let idx = FuzzyIndexAdapter::new(vec![
            fixture("google-chrome", "Chrome", vec!["Internet".to_string()]),
            fixture("firefox", "Firefox", vec!["Internet".to_string()]),
        ]);
        idx.set_user_aliases(HashMap::from([(
            "google-chrome".to_string(),
            vec!["browser".to_string()],
        )]));

        let ranked = idx.rank("browser", 10);
        assert_eq!(ranked.len(), 1, "only Chrome has the alias");
        assert_eq!(ranked[0].item.id, "google-chrome");
        assert_eq!(ranked[0].matched_field, MatchedField::AliasExact);
    }

    #[test]
    fn rank_alias_prefix_beats_unrelated_name_match() {
        let idx = FuzzyIndexAdapter::new(vec![
            fixture("google-chrome", "Chrome", vec!["Internet".to_string()]),
            fixture("chromatic", "Chromatic", vec!["Design".to_string()]),
        ]);
        idx.set_user_aliases(HashMap::from([(
            "google-chrome".to_string(),
            vec!["chrome-alias".to_string()],
        )]));

        let ranked = idx.rank("chrome", 10);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].item.id, "google-chrome");
        assert!(
            matches!(
                ranked[0].matched_field,
                MatchedField::AliasPrefix | MatchedField::AliasContains | MatchedField::AliasExact
            ),
            "expected alias match, got {:?}",
            ranked[0].matched_field
        );
    }

    #[test]
    fn rank_boosts_common_category() {
        let mut apps = vec![
            fixture("a", "Aurora", vec!["Internet".to_string()]),
            fixture("b", "Brave", vec!["Internet".to_string()]),
            fixture("c", "Chromium", vec!["Internet".to_string()]),
        ];
        for i in 0..30 {
            apps.push(fixture(
                &format!("dev{i}"),
                &format!("DevTool{i}"),
                vec!["Development".to_string()],
            ));
        }
        let idx = FuzzyIndexAdapter::new(apps);
        let freq = idx.category_frequency();
        assert!(freq.get("Development").copied().unwrap_or(0.0)
            > freq.get("Internet").copied().unwrap_or(0.0));
        let ranked = idx.rank("aurora", 10);
        assert!(!ranked.is_empty());
        assert!(ranked[0].score > 0.0);
    }

    #[test]
    fn rank_handles_empty_query() {
        let idx = FuzzyIndexAdapter::new(vec![fixture("c", "Chrome", vec!["Internet".to_string()])]);
        assert!(idx.rank("", 10).is_empty());
        assert!(idx.rank("   ", 10).is_empty());
    }

    #[test]
    fn rank_excludes_non_matches() {
        let idx = FuzzyIndexAdapter::new(vec![
            fixture("c", "Chrome", vec!["Internet".to_string()]),
            fixture("f", "Figma", vec!["Design".to_string()]),
        ]);
        let ranked = idx.rank("xyzpdq", 10);
        assert!(ranked.is_empty());
    }

    #[test]
    fn rank_respects_limit() {
        let apps: Vec<AppEntry> = (0..10)
            .map(|i| fixture(&format!("a{i}"), &format!("App{i}"), vec!["Misc".to_string()]))
            .collect();
        let idx = FuzzyIndexAdapter::new(apps);
        let ranked = idx.rank("app", 3);
        assert_eq!(ranked.len(), 3);
    }

    #[test]
    fn rank_handles_5000_items_under_budget() {
        let apps: Vec<AppEntry> = (0..5000)
            .map(|i| fixture(&format!("a{i}"), &format!("App{i:04}"), vec!["Misc".to_string()]))
            .collect();
        let idx = FuzzyIndexAdapter::new(apps);
        let start = std::time::Instant::now();
        let ranked = idx.rank("1234", 50);
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 250, "ranking took too long: {elapsed:?}");
        assert!(ranked.len() <= 50);
    }

    #[test]
    fn set_user_aliases_refreshes_items() {
        let idx = FuzzyIndexAdapter::new(vec![fixture(
            "google-chrome",
            "Chrome",
            vec!["Internet".to_string()],
        )]);
        let initial = idx.rank("browser", 10);
        assert!(initial.is_empty());

        idx.set_user_aliases(HashMap::from([(
            "google-chrome".to_string(),
            vec!["browser".to_string()],
        )]));

        let after = idx.rank("browser", 10);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].item.id, "google-chrome");
    }

    #[test]
    fn category_frequency_normalized_to_unit_range() {
        let mut apps = Vec::new();
        for i in 0..100 {
            apps.push(fixture(
                &format!("a{i}"),
                &format!("App{i}"),
                vec!["Development".to_string()],
            ));
        }
        apps.push(fixture("z", "Zeta", vec!["Misc".to_string()]));
        let idx = FuzzyIndexAdapter::new(apps);
        let freq = idx.category_frequency();
        let dev = freq.get("Development").copied().unwrap_or(0.0);
        assert!(dev > 0.5, "frequent category should approach 1, got {dev}");
        assert!(dev <= 1.0);
    }

    #[test]
    fn replace_apps_retains_user_aliases() {
        let idx = FuzzyIndexAdapter::new(vec![fixture(
            "google-chrome",
            "Chrome",
            vec!["Internet".to_string()],
        )]);
        idx.set_user_aliases(HashMap::from([(
            "google-chrome".to_string(),
            vec!["browser".to_string()],
        )]));

        idx.replace_apps(vec![fixture(
            "google-chrome",
            "Chrome",
            vec!["Internet".to_string()],
        )]);

        let ranked = idx.rank("browser", 10);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].item.id, "google-chrome");
    }
}