use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableKind {
    App,
    Script,
    AiTool,
    RecentFile,
    Shortcut,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiscoverableLaunch {
    App { exec: String },
    Script { path: String, args: Option<String> },
    AiTool { tool_id: String },
    RecentFile { path: String },
    Shortcut { target: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscoverableItem {
    pub kind: DiscoverableKind,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub keywords: Vec<String>,
    pub score: f32,
    pub source: String,
    pub launch: DiscoverableLaunch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

impl DiscoverableItem {
    pub fn from_app(app: &crate::domain::apps::AppEntry) -> Self {
        let category = app.categories.first().cloned();
        let mut aliases: Vec<String> = app
            .keywords
            .iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        aliases.sort();
        aliases.dedup();
        let source = format!("app:{:?}", app.source).to_lowercase();
        Self {
            kind: DiscoverableKind::App,
            id: app.id.clone(),
            name: app.name.clone(),
            description: app.description.clone(),
            icon: app.icon.clone(),
            keywords: app.keywords.clone(),
            score: 0.0,
            source,
            launch: DiscoverableLaunch::App {
                exec: app.exec.clone(),
            },
            category,
            aliases,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowseSections {
    pub apps_by_category: HashMap<String, Vec<DiscoverableItem>>,
    pub scripts: Vec<crate::domain::config::ScriptConfig>,
    pub ai_tools: Vec<crate::domain::config::AiTool>,
    pub recent_files: Vec<crate::domain::action::Action>,
    pub shortcuts: HashMap<String, String>,
}

impl BrowseSections {
    pub fn from_snapshot(
        apps: Vec<DiscoverableItem>,
        scripts: Vec<crate::domain::config::ScriptConfig>,
        ai_tools: Vec<crate::domain::config::AiTool>,
        recent_files: Vec<crate::domain::action::Action>,
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

    #[test]
    fn kind_serializes_as_snake_case() {
        let json = serde_json::to_string(&DiscoverableKind::RecentFile).unwrap();
        assert_eq!(json, "\"recent_file\"");
        let json = serde_json::to_string(&DiscoverableKind::AiTool).unwrap();
        assert_eq!(json, "\"ai_tool\"");
    }

    #[test]
    fn launch_serializes_with_tag_and_snake_case_fields() {
        let launch = DiscoverableLaunch::App {
            exec: "firefox".to_string(),
        };
        let json = serde_json::to_string(&launch).unwrap();
        assert!(json.contains("\"type\":\"app\""));
        assert!(json.contains("\"exec\":\"firefox\""));

        let launch = DiscoverableLaunch::RecentFile {
            path: "/home/me/note.txt".to_string(),
        };
        let json = serde_json::to_string(&launch).unwrap();
        assert!(json.contains("\"type\":\"recent_file\""));
        assert!(json.contains("\"path\":\"/home/me/note.txt\""));
    }

    #[test]
    fn discoverable_item_round_trips_via_json() {
        let item = DiscoverableItem {
            kind: DiscoverableKind::Script,
            id: "deploy".to_string(),
            name: "deploy".to_string(),
            description: Some("deploy to staging".to_string()),
            icon: None,
            keywords: vec!["deploy".to_string()],
            score: 1.23,
            source: "config.script".to_string(),
            launch: DiscoverableLaunch::Script {
                path: "/srv/scripts/deploy.sh".to_string(),
                args: Some("--staging".to_string()),
            },
            category: None,
            aliases: Vec::new(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: DiscoverableItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.kind, DiscoverableKind::Script);
        assert_eq!(parsed.id, "deploy");
        match parsed.launch {
            DiscoverableLaunch::Script { path, args } => {
                assert_eq!(path, "/srv/scripts/deploy.sh");
                assert_eq!(args.as_deref(), Some("--staging"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn discoverable_item_accepts_additive_category_and_aliases() {
        let legacy_json = r#"{
            "kind": "app",
            "id": "chrome",
            "name": "Chrome",
            "description": null,
            "icon": null,
            "keywords": [],
            "score": 1.0,
            "source": "app:desktop",
            "launch": {"type": "app", "exec": "chrome"}
        }"#;
        let parsed: DiscoverableItem = serde_json::from_str(legacy_json).unwrap();
        assert!(parsed.category.is_none());
        assert!(parsed.aliases.is_empty());

        let with_extras_json = r#"{
            "kind": "app",
            "id": "chrome",
            "name": "Chrome",
            "description": null,
            "icon": null,
            "keywords": [],
            "score": 1.0,
            "source": "app:desktop",
            "launch": {"type": "app", "exec": "chrome"},
            "category": "Internet",
            "aliases": ["browser", "web"]
        }"#;
        let parsed: DiscoverableItem = serde_json::from_str(with_extras_json).unwrap();
        assert_eq!(parsed.category.as_deref(), Some("Internet"));
        assert_eq!(parsed.aliases, vec!["browser".to_string(), "web".to_string()]);
    }

    #[test]
    fn browse_sections_groups_by_category() {
        use crate::domain::action::Action;
        let apps = vec![
            DiscoverableItem {
                kind: DiscoverableKind::App,
                id: "code".to_string(),
                name: "Code".to_string(),
                category: Some("Development".to_string()),
                aliases: vec!["editor".to_string()],
                launch: DiscoverableLaunch::App {
                    exec: "code".to_string(),
                },
                ..make_blank_item(DiscoverableKind::App)
            },
            DiscoverableItem {
                kind: DiscoverableKind::App,
                id: "chrome".to_string(),
                name: "Chrome".to_string(),
                category: Some("Internet".to_string()),
                launch: DiscoverableLaunch::App {
                    exec: "chrome".to_string(),
                },
                ..make_blank_item(DiscoverableKind::App)
            },
            DiscoverableItem {
                kind: DiscoverableKind::App,
                id: "misc".to_string(),
                name: "Misc".to_string(),
                category: None,
                launch: DiscoverableLaunch::App {
                    exec: "misc".to_string(),
                },
                ..make_blank_item(DiscoverableKind::App)
            },
        ];
        let sections = BrowseSections::from_snapshot(
            apps,
            Vec::new(),
            Vec::new(),
            vec![Action {
                id: "f".to_string(),
                kind: "file".to_string(),
                content: "/tmp/f".to_string(),
                name: "f".to_string(),
                icon: None,
                last_accessed: 0,
                frequency: 0,
            }],
            HashMap::new(),
        );
        assert_eq!(sections.apps_by_category.get("Development").unwrap().len(), 1);
        assert_eq!(sections.apps_by_category.get("Internet").unwrap().len(), 1);
        assert_eq!(sections.apps_by_category.get("Other").unwrap().len(), 1);
        assert_eq!(sections.recent_files.len(), 1);
    }

    fn make_blank_item(kind: DiscoverableKind) -> DiscoverableItem {
        let launch = match kind {
            DiscoverableKind::App => DiscoverableLaunch::App {
                exec: String::new(),
            },
            DiscoverableKind::Script => DiscoverableLaunch::Script {
                path: String::new(),
                args: None,
            },
            DiscoverableKind::AiTool => DiscoverableLaunch::AiTool {
                tool_id: String::new(),
            },
            DiscoverableKind::RecentFile => DiscoverableLaunch::RecentFile {
                path: String::new(),
            },
            DiscoverableKind::Shortcut => DiscoverableLaunch::Shortcut {
                target: String::new(),
            },
        };
        DiscoverableItem {
            kind,
            id: String::new(),
            name: String::new(),
            description: None,
            icon: None,
            keywords: Vec::new(),
            score: 0.0,
            source: String::new(),
            launch,
            category: None,
            aliases: Vec::new(),
        }
    }
}