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

impl DiscoverableKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiscoverableKind::App => "app",
            DiscoverableKind::Script => "script",
            DiscoverableKind::AiTool => "ai_tool",
            DiscoverableKind::RecentFile => "recent_file",
            DiscoverableKind::Shortcut => "shortcut",
        }
    }

    pub fn parse(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "app" | "apps" => Some(DiscoverableKind::App),
            "script" | "scripts" => Some(DiscoverableKind::Script),
            "ai" | "ai_tool" | "ai-tool" | "ai_tools" | "ai-tools" | "aitool" => {
                Some(DiscoverableKind::AiTool)
            }
            "recent" | "recent_file" | "recent-file" | "recent_files" | "recent-files" => {
                Some(DiscoverableKind::RecentFile)
            }
            "shortcut" | "shortcuts" => Some(DiscoverableKind::Shortcut),
            _ => None,
        }
    }
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

    pub fn snippet(&self, max_len: usize) -> String {
        let raw = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.name);
        if raw.chars().count() <= max_len {
            return raw.to_string();
        }
        let mut out: String = raw.chars().take(max_len.saturating_sub(1)).collect();
        out.push('\u{2026}');
        out
    }

    pub fn source_label(&self) -> &str {
        let s = self.source.as_str();
        if let Some(idx) = s.find(':') {
            &s[idx + 1..]
        } else {
            s
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
    #[serde(default)]
    pub category_counts: HashMap<String, usize>,
    #[serde(default)]
    pub source_counts: HashMap<String, usize>,
    #[serde(default)]
    pub kind_counts: HashMap<String, usize>,
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
        let mut category_counts: HashMap<String, usize> = HashMap::new();
        let mut source_counts: HashMap<String, usize> = HashMap::new();
        let mut kind_counts: HashMap<String, usize> = HashMap::new();

        for app in &apps {
            let bucket = app
                .category
                .clone()
                .unwrap_or_else(|| "Other".to_string());
            *category_counts.entry(bucket.clone()).or_insert(0) += 1;
            *source_counts.entry(app.source.clone()).or_insert(0) += 1;
            *kind_counts
                .entry(app.kind.as_str().to_string())
                .or_insert(0) += 1;
            apps_by_category.entry(bucket).or_default().push(app.clone());
        }
        for bucket in apps_by_category.values_mut() {
            bucket.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }

        if !scripts.is_empty() {
            *source_counts
                .entry("config:script".to_string())
                .or_insert(0) += scripts.len();
            *kind_counts
                .entry(DiscoverableKind::Script.as_str().to_string())
                .or_insert(0) += scripts.len();
        }
        if !ai_tools.is_empty() {
            *source_counts
                .entry("config:ai_tool".to_string())
                .or_insert(0) += ai_tools.len();
            *kind_counts
                .entry(DiscoverableKind::AiTool.as_str().to_string())
                .or_insert(0) += ai_tools.len();
        }
        if !recent_files.is_empty() {
            *source_counts
                .entry("history:file".to_string())
                .or_insert(0) += recent_files.len();
            *kind_counts
                .entry(DiscoverableKind::RecentFile.as_str().to_string())
                .or_insert(0) += recent_files.len();
        }
        if !shortcuts.is_empty() {
            *source_counts
                .entry("config:shortcut".to_string())
                .or_insert(0) += shortcuts.len();
            *kind_counts
                .entry(DiscoverableKind::Shortcut.as_str().to_string())
                .or_insert(0) += shortcuts.len();
        }

        Self {
            apps_by_category,
            scripts,
            ai_tools,
            recent_files,
            shortcuts,
            category_counts,
            source_counts,
            kind_counts,
        }
    }

    pub fn total_apps(&self) -> usize {
        self.apps_by_category.values().map(|v| v.len()).sum()
    }

    pub fn sources(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.source_counts.keys().cloned().collect();
        keys.sort();
        keys
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

    fn make_app_item(id: &str, name: &str, category: Option<&str>, source: &str) -> DiscoverableItem {
        DiscoverableItem {
            kind: DiscoverableKind::App,
            id: id.to_string(),
            name: name.to_string(),
            description: Some(format!("{name} description")),
            icon: None,
            keywords: Vec::new(),
            score: 1.0,
            source: source.to_string(),
            launch: DiscoverableLaunch::App {
                exec: id.to_string(),
            },
            category: category.map(|s| s.to_string()),
            aliases: Vec::new(),
        }
    }

    #[test]
    fn kind_as_str_matches_serde_representation() {
        assert_eq!(DiscoverableKind::App.as_str(), "app");
        assert_eq!(DiscoverableKind::Script.as_str(), "script");
        assert_eq!(DiscoverableKind::AiTool.as_str(), "ai_tool");
        assert_eq!(DiscoverableKind::RecentFile.as_str(), "recent_file");
        assert_eq!(DiscoverableKind::Shortcut.as_str(), "shortcut");
    }

    #[test]
    fn kind_parse_accepts_plural_and_dash_variants() {
        assert_eq!(DiscoverableKind::parse("app"), Some(DiscoverableKind::App));
        assert_eq!(DiscoverableKind::parse("apps"), Some(DiscoverableKind::App));
        assert_eq!(
            DiscoverableKind::parse("ai-tool"),
            Some(DiscoverableKind::AiTool)
        );
        assert_eq!(
            DiscoverableKind::parse("recent_files"),
            Some(DiscoverableKind::RecentFile)
        );
        assert_eq!(
            DiscoverableKind::parse("recent-file"),
            Some(DiscoverableKind::RecentFile)
        );
        assert_eq!(
            DiscoverableKind::parse("shortcuts"),
            Some(DiscoverableKind::Shortcut)
        );
        assert_eq!(DiscoverableKind::parse("bogus"), None);
    }

    #[test]
    fn snippet_falls_back_to_name_and_truncates_long_text() {
        let mut item = make_blank_item(DiscoverableKind::App);
        item.name = "Chrome".to_string();
        item.description = None;
        assert_eq!(item.snippet(80), "Chrome");

        item.description = Some("short".to_string());
        assert_eq!(item.snippet(80), "short");

        item.description = Some("a".repeat(40));
        let s = item.snippet(10);
        assert!(s.chars().count() <= 10);
        assert!(s.ends_with('\u{2026}'));
    }

    #[test]
    fn source_label_strips_kind_prefix() {
        let mut item = make_blank_item(DiscoverableKind::App);
        item.source = "app:desktop".to_string();
        assert_eq!(item.source_label(), "desktop");

        item.source = "config:script".to_string();
        assert_eq!(item.source_label(), "script");

        item.source = "history:file".to_string();
        assert_eq!(item.source_label(), "file");
    }

    #[test]
    fn browse_sections_records_category_source_and_kind_counts() {
        use crate::domain::action::Action;
        use crate::domain::config::{AiTool, ScriptConfig};
        let apps = vec![
            make_app_item("chrome", "Chrome", Some("Internet"), "app:desktop"),
            make_app_item("firefox", "Firefox", Some("Internet"), "app:flatpak"),
            make_app_item("code", "Code", Some("Development"), "app:desktop"),
            make_app_item("misc", "Misc", None, "app:desktop"),
        ];
        let scripts = vec![ScriptConfig {
            id: "deploy".to_string(),
            alias: "deploy".to_string(),
            path: "/srv/scripts/deploy.sh".to_string(),
            args: None,
        }];
        let ai_tools = vec![AiTool {
            id: "rephrase".to_string(),
            name: "Rephrase".to_string(),
            description: String::new(),
            prompt_template: String::new(),
            keywords: Vec::new(),
            icon: String::new(),
        }];
        let recent_files = vec![Action {
            id: "f".to_string(),
            kind: "file".to_string(),
            content: "/tmp/note.md".to_string(),
            name: "note.md".to_string(),
            icon: None,
            last_accessed: 0,
            frequency: 0,
        }];
        let mut shortcuts = HashMap::new();
        shortcuts.insert("browser".to_string(), "chrome".to_string());

        let sections = BrowseSections::from_snapshot(apps, scripts, ai_tools, recent_files, shortcuts);

        assert_eq!(sections.category_counts.get("Internet").copied(), Some(2));
        assert_eq!(sections.category_counts.get("Development").copied(), Some(1));
        assert_eq!(sections.category_counts.get("Other").copied(), Some(1));
        assert_eq!(sections.category_counts.values().sum::<usize>(), 4);

        assert_eq!(sections.source_counts.get("app:desktop").copied(), Some(3));
        assert_eq!(sections.source_counts.get("app:flatpak").copied(), Some(1));
        assert_eq!(sections.source_counts.get("config:script").copied(), Some(1));
        assert_eq!(sections.source_counts.get("config:ai_tool").copied(), Some(1));
        assert_eq!(sections.source_counts.get("history:file").copied(), Some(1));
        assert_eq!(sections.source_counts.get("config:shortcut").copied(), Some(1));

        assert_eq!(sections.kind_counts.get("app").copied(), Some(4));
        assert_eq!(sections.kind_counts.get("script").copied(), Some(1));
        assert_eq!(sections.kind_counts.get("ai_tool").copied(), Some(1));
        assert_eq!(sections.kind_counts.get("recent_file").copied(), Some(1));
        assert_eq!(sections.kind_counts.get("shortcut").copied(), Some(1));

        assert_eq!(sections.total_apps(), 4);
        let mut sources = sections.sources();
        sources.sort();
        assert_eq!(
            sources,
            vec![
                "app:desktop".to_string(),
                "app:flatpak".to_string(),
                "config:ai_tool".to_string(),
                "config:script".to_string(),
                "config:shortcut".to_string(),
                "history:file".to_string(),
            ]
        );
    }

    #[test]
    fn browse_sections_counts_zero_when_empty() {
        let sections = BrowseSections::from_snapshot(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
        );
        assert!(sections.category_counts.is_empty());
        assert!(sections.source_counts.is_empty());
        assert!(sections.kind_counts.is_empty());
        assert_eq!(sections.total_apps(), 0);
        assert!(sections.sources().is_empty());
    }

    #[test]
    fn browse_sections_serializes_new_count_fields() {
        let sections = BrowseSections {
            apps_by_category: HashMap::new(),
            scripts: Vec::new(),
            ai_tools: Vec::new(),
            recent_files: Vec::new(),
            shortcuts: HashMap::new(),
            category_counts: HashMap::from([("Internet".to_string(), 2)]),
            source_counts: HashMap::from([("app:desktop".to_string(), 2)]),
            kind_counts: HashMap::from([("app".to_string(), 2)]),
        };
        let json = serde_json::to_string(&sections).unwrap();
        assert!(json.contains("\"category_counts\""));
        assert!(json.contains("\"Internet\":2"));
        assert!(json.contains("\"source_counts\""));
        assert!(json.contains("\"app:desktop\":2"));
        assert!(json.contains("\"kind_counts\""));
        assert!(json.contains("\"app\":2"));
    }

    #[test]
    fn browse_sections_round_trips_legacy_payload_without_count_fields() {
        let legacy_json = r#"{
            "apps_by_category": {},
            "scripts": [],
            "ai_tools": [],
            "recent_files": [],
            "shortcuts": {}
        }"#;
        let parsed: BrowseSections = serde_json::from_str(legacy_json).unwrap();
        assert!(parsed.category_counts.is_empty());
        assert!(parsed.source_counts.is_empty());
        assert!(parsed.kind_counts.is_empty());
    }
}