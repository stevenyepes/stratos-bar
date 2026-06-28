use crate::domain::config::{AppConfig, QuickActions};
use crate::ports::config_port::ConfigService;
use std::fs;
use std::path::PathBuf;

pub struct FsConfigService {
    custom_root: Option<PathBuf>,
}

impl FsConfigService {
    pub fn new() -> Self {
        Self { custom_root: None }
    }

    #[cfg(test)]
    pub fn new_with_root(root: PathBuf) -> Self {
        Self {
            custom_root: Some(root),
        }
    }
}

impl ConfigService for FsConfigService {
    fn get_config_dir(&self) -> Option<PathBuf> {
        if let Some(ref root) = self.custom_root {
            return Some(root.clone());
        }
        dirs::config_dir().map(|p| p.join("stratos-bar"))
    }

    fn load_config(&self) -> AppConfig {
        let mut config = if let Some(config_dir) = self.get_config_dir() {
            let config_path = config_dir.join("config.json");
            if config_path.exists() {
                if let Ok(content) = fs::read_to_string(config_path) {
                    serde_json::from_str(&content).unwrap_or_default()
                } else {
                    AppConfig::default()
                }
            } else {
                AppConfig::default()
            }
        } else {
            AppConfig::default()
        };

        // Set Defaults if empty
        config.apply_defaults();

        config
    }

    fn save_config(&self, config: &AppConfig) -> Result<(), String> {
        if let Some(config_dir) = self.get_config_dir() {
            if !config_dir.exists() {
                fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
            }
            let config_path = config_dir.join("config.json");
            let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
            fs::write(config_path, content).map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Could not find config directory".to_string())
        }
    }

    fn set_quick_actions(&self, actions: QuickActions) -> Result<(), String> {
        let mut config = self.load_config();
        config.quick_actions = actions;
        self.save_config(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_load_config() {
        let dir = tempdir().unwrap();
        let service = FsConfigService::new_with_root(dir.path().to_path_buf());

        let mut config = AppConfig::default();
        config.preferred_model = "test_model".to_string();

        // Save
        service.save_config(&config).expect("Failed to save config");

        // Load
        let loaded_config = service.load_config();
        assert_eq!(loaded_config.preferred_model, "test_model");
    }

    #[test]
    fn test_load_defaults_if_missing() {
        let dir = tempdir().unwrap();
        let service = FsConfigService::new_with_root(dir.path().to_path_buf());

        let config = service.load_config();
        // Should have defaults applied
        assert_eq!(config.preferred_model, "local");
    }

    #[test]
    fn test_quick_actions_persists_through_disk_round_trip() {
        let dir = tempdir().unwrap();
        let service = FsConfigService::new_with_root(dir.path().to_path_buf());

        let mut quick = QuickActions::default();
        let mut app_aliases = HashMap::new();
        app_aliases.insert(
            "google-chrome".to_string(),
            vec!["browser".to_string()],
        );
        quick.app_aliases = app_aliases;
        let mut skill_aliases = HashMap::new();
        skill_aliases.insert("rephrase".to_string(), vec!["rewrite".to_string()]);
        quick.skill_aliases = skill_aliases;
        let mut keyword_shortcuts = HashMap::new();
        keyword_shortcuts.insert("slack".to_string(), "slack-desktop".to_string());
        keyword_shortcuts.insert(
            "g cal".to_string(),
            "https://calendar.google.com".to_string(),
        );
        quick.keyword_shortcuts = keyword_shortcuts;
        quick.pinned_favorites = vec!["google-chrome".to_string(), "code".to_string()];

        service
            .set_quick_actions(quick.clone())
            .expect("set_quick_actions should succeed");

        // Simulate a fresh process: build a new service backed by the same dir.
        let reloaded = FsConfigService::new_with_root(dir.path().to_path_buf());
        let reloaded_actions = reloaded.load_config().quick_actions;

        assert_eq!(
            reloaded_actions.app_aliases.get("google-chrome"),
            Some(&vec!["browser".to_string()])
        );
        assert_eq!(
            reloaded_actions.skill_aliases.get("rephrase"),
            Some(&vec!["rewrite".to_string()])
        );
        assert_eq!(
            reloaded_actions.keyword_shortcuts.get("slack"),
            Some(&"slack-desktop".to_string())
        );
        assert_eq!(
            reloaded_actions.keyword_shortcuts.get("g cal"),
            Some(&"https://calendar.google.com".to_string())
        );
        assert_eq!(
            reloaded_actions.pinned_favorites,
            vec!["google-chrome".to_string(), "code".to_string()]
        );
        assert!(reloaded_actions.seed_initialized);
    }

    #[test]
    fn test_load_config_seeds_first_run_top_apps() {
        let dir = tempdir().unwrap();
        let service = FsConfigService::new_with_root(dir.path().to_path_buf());

        let config = service.load_config();
        assert!(config.quick_actions.seed_initialized);
        assert!(!config.quick_actions.top_used_seed.is_empty());
        assert!(config
            .quick_actions
            .top_used_seed
            .contains(&"google-chrome".to_string()));
    }

    #[test]
    fn test_load_config_is_additive_for_legacy_files() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        // Pre-existing config without quick_actions field at all
        let legacy = r#"{
            "preferred_model": "cloud",
            "ai_tools": [],
            "shortcuts": {},
            "scripts": [],
            "theme": null,
            "file_search": { "include_hidden": false },
            "custom_app_dirs": []
        }"#;
        std::fs::write(&config_path, legacy).unwrap();

        let service = FsConfigService::new_with_root(dir.path().to_path_buf());
        let config = service.load_config();
        assert_eq!(config.preferred_model, "cloud");
        assert!(config.quick_actions.seed_initialized);
        assert!(!config.quick_actions.top_used_seed.is_empty());
    }
}
