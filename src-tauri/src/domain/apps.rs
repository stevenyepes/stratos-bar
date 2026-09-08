use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppEntry {
    /// The XDG desktop-file-id: the `.desktop` file's path relative to its
    /// `applications/` root, with path separators replaced by `-` (e.g.
    /// `kde4-foo` for `applications/kde4/foo.desktop`). Not a bare filename —
    /// entries nested in subdirectories are not equivalent to their basename.
    /// AppImage entries instead use the `appimage:<name>` namespace.
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
    pub working_dir: Option<String>,
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
            working_dir: None,
        };

        assert_eq!(app.id, "firefox");
        assert_eq!(app.name, "Firefox");
        assert_eq!(app.exec, "firefox");
        assert_eq!(app.icon.as_deref(), Some("firefox.png"));
        assert_eq!(app.source, AppSource::Desktop);
    }
}
