use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
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
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AppSource {
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
        };

        assert_eq!(app.id, "firefox");
        assert_eq!(app.name, "Firefox");
        assert_eq!(app.exec, "firefox");
        assert_eq!(app.icon.as_deref(), Some("firefox.png"));
        assert_eq!(app.source, AppSource::Desktop);
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
}
