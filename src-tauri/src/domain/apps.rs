use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_entry_creation() {
        let app = AppEntry {
            name: "Firefox".to_string(),
            exec: "firefox".to_string(),
            icon: Some("firefox.png".to_string()),
        };

        assert_eq!(app.name, "Firefox");
        assert_eq!(app.exec, "firefox");
        assert_eq!(app.icon, Some("firefox.png".to_string()));
    }
}
