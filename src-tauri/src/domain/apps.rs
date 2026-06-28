use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub exec: String,
    pub try_exec: Option<String>,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub startup_wm_class: Option<String>,
    pub source: AppSource,
    pub path: String,

    #[serde(default)]
    pub launch_count: Option<u32>,
    #[serde(default)]
    pub last_launched_at: Option<u64>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub score_hint: Option<f32>,
    #[serde(default)]
    pub indexed_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppSource {
    #[default]
    Desktop,
    Flatpak,
    Snap,
    AppImage,
    Nix,
    Other,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MatchedField {
    NamePrefix,
    NameContains,
    Keyword,
    GenericName,
    Description,
    Exec,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScoredApp {
    pub app: AppEntry,
    pub score: f32,
    pub matched_field: MatchedField,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> AppEntry {
        AppEntry {
            id: name.to_lowercase(),
            name: name.to_string(),
            generic_name: None,
            description: None,
            keywords: Vec::new(),
            exec: name.to_lowercase(),
            try_exec: None,
            icon: None,
            categories: Vec::new(),
            startup_wm_class: None,
            source: AppSource::Desktop,
            path: format!("/tmp/{name}.desktop"),
            ..Default::default()
        }
    }

    #[test]
    fn test_app_entry_creation() {
        let app = AppEntry {
            id: "firefox".to_string(),
            name: "Firefox".to_string(),
            generic_name: Some("Web Browser".to_string()),
            description: Some("Browse the web".to_string()),
            keywords: vec!["browser".to_string(), "web".to_string()],
            exec: "firefox".to_string(),
            try_exec: Some("firefox".to_string()),
            icon: Some("firefox.png".to_string()),
            categories: vec!["Network".to_string(), "WebBrowser".to_string()],
            startup_wm_class: Some("Firefox".to_string()),
            source: AppSource::Desktop,
            path: "/usr/share/applications/firefox.desktop".to_string(),
            ..Default::default()
        };

        assert_eq!(app.id, "firefox");
        assert_eq!(app.name, "Firefox");
        assert_eq!(app.exec, "firefox");
        assert_eq!(app.icon.as_deref(), Some("firefox.png"));
        assert_eq!(app.source, AppSource::Desktop);
        assert_eq!(app.launch_count, None);
        assert_eq!(app.last_launched_at, None);
        assert!(app.aliases.is_empty());
        assert_eq!(app.score_hint, None);
        assert_eq!(app.indexed_at, None);
    }

    #[test]
    fn test_scored_app_creation() {
        let app = AppEntry {
            id: "firefox".to_string(),
            name: "Firefox".to_string(),
            generic_name: None,
            description: None,
            keywords: Vec::new(),
            exec: "firefox".to_string(),
            try_exec: None,
            icon: None,
            categories: Vec::new(),
            startup_wm_class: None,
            source: AppSource::Desktop,
            path: "/usr/share/applications/firefox.desktop".to_string(),
            ..Default::default()
        };

        let scored = ScoredApp {
            app,
            score: 1.25,
            matched_field: MatchedField::NamePrefix,
        };

        assert_eq!(scored.app.name, "Firefox");
        assert_eq!(scored.matched_field, MatchedField::NamePrefix);
        assert!((scored.score - 1.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_matched_field_serializes_snake_case() {
        let json = serde_json::to_string(&MatchedField::NamePrefix).unwrap();
        assert_eq!(json, "\"name_prefix\"");
        let json = serde_json::to_string(&MatchedField::GenericName).unwrap();
        assert_eq!(json, "\"generic_name\"");
    }

    #[test]
    fn test_app_entry_serde_defaults_when_fields_missing() {
        let minimal_json = r#"{
            "id": "code",
            "name": "Code",
            "keywords": [],
            "exec": "code",
            "source": "desktop",
            "path": "/usr/share/applications/code.desktop",
            "categories": []
        }"#;
        let parsed: AppEntry = serde_json::from_str(minimal_json).unwrap();
        assert_eq!(parsed.id, "code");
        assert_eq!(parsed.launch_count, None);
        assert_eq!(parsed.last_launched_at, None);
        assert!(parsed.aliases.is_empty());
        assert_eq!(parsed.score_hint, None);
        assert_eq!(parsed.indexed_at, None);

        let reserialized = serde_json::to_string(&parsed).unwrap();
        let roundtrip: AppEntry = serde_json::from_str(&reserialized).unwrap();
        assert_eq!(roundtrip, parsed);
    }

    #[test]
    fn test_app_entry_launch_count_round_trip() {
        let mut app = fixture("chrome");
        app.launch_count = Some(42);
        app.last_launched_at = Some(1_700_000_000);
        app.aliases = vec!["browser".to_string(), "g".to_string()];
        app.score_hint = Some(0.75);
        app.indexed_at = Some(1_700_000_999);

        let json = serde_json::to_string(&app).unwrap();
        let parsed: AppEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.launch_count, Some(42));
        assert_eq!(parsed.last_launched_at, Some(1_700_000_000));
        assert_eq!(parsed.aliases, vec!["browser".to_string(), "g".to_string()]);
        assert_eq!(parsed.score_hint, Some(0.75));
        assert_eq!(parsed.indexed_at, Some(1_700_000_999));
        assert_eq!(parsed, app);
    }

    #[test]
    fn test_app_entry_score_hint_round_trip() {
        let mut app = fixture("code");
        app.score_hint = Some(1.5);
        let json = serde_json::to_string(&app).unwrap();
        let parsed: AppEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.score_hint, Some(1.5));
        assert_eq!(parsed, app);
    }

    #[test]
    fn test_app_entry_indexed_at_round_trip() {
        let mut app = fixture("firefox");
        app.indexed_at = Some(1_234_567_890);
        let json = serde_json::to_string(&app).unwrap();
        let parsed: AppEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.indexed_at, Some(1_234_567_890));
        assert_eq!(parsed, app);
    }

    #[test]
    fn test_app_entry_aliases_round_trip() {
        let mut app = fixture("chrome");
        app.aliases = vec!["browser".to_string(), "google".to_string()];
        let json = serde_json::to_string(&app).unwrap();
        let parsed: AppEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.aliases, vec!["browser".to_string(), "google".to_string()]);
        assert_eq!(parsed, app);
    }
}
