use crate::domain::action::Action;
use crate::domain::apps::{AppEntry, AppSource};
use crate::domain::config::{AiTool, ScriptConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableKind {
    #[default]
    App,
    Script,
    AiTool,
    File,
    Shortcut,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DiscoverableItem {
    pub id: String,
    pub name: String,
    pub kind: DiscoverableKind,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub category: Option<String>,
    pub aliases: Vec<String>,
    pub source: Option<AppSource>,
    pub keywords: Vec<String>,
}

impl DiscoverableItem {
    pub fn from_app(app: &AppEntry) -> Self {
        let category = app.categories.first().cloned();
        let mut aliases = Vec::new();
        for kw in &app.keywords {
            aliases.push(kw.clone());
        }
        Self {
            id: app.id.clone(),
            name: app.name.clone(),
            kind: DiscoverableKind::App,
            description: app.description.clone(),
            icon: app.icon.clone(),
            exec: Some(app.exec.clone()),
            category,
            aliases,
            source: Some(app.source),
            keywords: app.keywords.clone(),
        }
    }

    pub fn with_user_aliases(mut self, user_aliases: &[String]) -> Self {
        for a in user_aliases {
            let trimmed = a.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !self.aliases.iter().any(|existing| existing == trimmed) {
                self.aliases.push(trimmed.to_string());
            }
        }
        self
    }

    pub fn with_category(mut self, category: Option<String>) -> Self {
        self.category = category;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredDiscoverable {
    pub item: DiscoverableItem,
    pub score: f32,
    pub matched_field: MatchedField,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchedField {
    NamePrefix,
    NameContains,
    AliasExact,
    AliasPrefix,
    AliasContains,
    Keyword,
    Category,
    Description,
    Exec,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowseSections {
    pub apps_by_category: HashMap<String, Vec<DiscoverableItem>>,
    pub scripts: Vec<ScriptConfig>,
    pub ai_tools: Vec<AiTool>,
    pub recent_files: Vec<Action>,
    pub shortcuts: HashMap<String, String>,
}

impl BrowseSections {
    pub fn from_parts(
        apps: Vec<DiscoverableItem>,
        scripts: Vec<ScriptConfig>,
        ai_tools: Vec<AiTool>,
        recent_files: Vec<Action>,
        shortcuts: HashMap<String, String>,
    ) -> Self {
        let mut apps_by_category: HashMap<String, Vec<DiscoverableItem>> = HashMap::new();
        for app in apps {
            let bucket = app
                .category
                .clone()
                .unwrap_or_else(|| "Other".to_string());
            apps_by_category.entry(bucket).or_default().push(app);
        }
        for bucket in apps_by_category.values_mut() {
            bucket.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        Self {
            apps_by_category,
            scripts,
            ai_tools,
            recent_files,
            shortcuts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::apps::AppSource;

    fn fixture(id: &str, name: &str) -> AppEntry {
        AppEntry {
            id: id.to_string(),
            name: name.to_string(),
            generic_name: None,
            description: Some(format!("{name} description")),
            keywords: vec!["kw".to_string()],
            exec: id.to_string(),
            try_exec: None,
            icon: Some(format!("/icons/{id}.png")),
            categories: vec!["Development".to_string()],
            startup_wm_class: None,
            source: AppSource::Desktop,
            path: format!("/usr/share/applications/{id}.desktop"),
        }
    }

    #[test]
    fn from_app_populates_category_and_aliases() {
        let app = fixture("code", "Code");
        let item = DiscoverableItem::from_app(&app);
        assert_eq!(item.id, "code");
        assert_eq!(item.name, "Code");
        assert_eq!(item.kind, DiscoverableKind::App);
        assert_eq!(item.category.as_deref(), Some("Development"));
        assert!(item.aliases.contains(&"kw".to_string()));
        assert_eq!(item.exec.as_deref(), Some("code"));
        assert_eq!(item.source, Some(AppSource::Desktop));
    }

    #[test]
    fn with_user_aliases_appends_without_duplicates() {
        let app = fixture("code", "Code");
        let item = DiscoverableItem::from_app(&app).with_user_aliases(&["browser".to_string(), "kw".to_string()]);
        assert!(item.aliases.iter().any(|a| a == "browser"));
        let kw_count = item.aliases.iter().filter(|a| *a == "kw").count();
        assert_eq!(kw_count, 1);
    }

    #[test]
    fn with_user_aliases_skips_blank_entries() {
        let app = fixture("code", "Code");
        let item = DiscoverableItem::from_app(&app).with_user_aliases(&["  ".to_string(), "".to_string(), "editor".to_string()]);
        assert_eq!(item.aliases.iter().filter(|a| *a == "editor").count(), 1);
        assert!(!item.aliases.iter().any(|a| a.trim().is_empty()));
    }

    #[test]
    fn with_category_overrides_category() {
        let app = fixture("code", "Code");
        let item = DiscoverableItem::from_app(&app).with_category(Some("Editors".to_string()));
        assert_eq!(item.category.as_deref(), Some("Editors"));
    }

    #[test]
    fn browse_sections_groups_by_category() {
        let mut code = DiscoverableItem::from_app(&fixture("code", "Code"));
        code.category = Some("Development".to_string());
        let mut chrome = DiscoverableItem::from_app(&fixture("chrome", "Chrome"));
        chrome.category = Some("Internet".to_string());
        let mut misc = DiscoverableItem::from_app(&fixture("misc", "Misc"));
        misc.category = None;

        let apps = vec![code, chrome, misc];
        let sections = BrowseSections::from_parts(apps, vec![], vec![], vec![], HashMap::new());
        assert!(sections.apps_by_category.contains_key("Development"));
        assert!(sections.apps_by_category.contains_key("Internet"));
        assert!(sections.apps_by_category.contains_key("Other"));
        assert_eq!(sections.apps_by_category.get("Development").unwrap().len(), 1);
        assert_eq!(sections.apps_by_category.get("Other").unwrap().len(), 1);
    }

    #[test]
    fn browse_sections_sorts_within_bucket_alphabetically() {
        let mut a = DiscoverableItem::from_app(&fixture("a", "Zeta"));
        a.category = Some("X".to_string());
        let mut b = DiscoverableItem::from_app(&fixture("b", "Alpha"));
        b.category = Some("X".to_string());
        let sections = BrowseSections::from_parts(vec![a, b], vec![], vec![], vec![], HashMap::new());
        let bucket = sections.apps_by_category.get("X").unwrap();
        assert_eq!(bucket[0].name, "Alpha");
        assert_eq!(bucket[1].name, "Zeta");
    }
}