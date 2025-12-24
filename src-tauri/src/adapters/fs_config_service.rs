use crate::domain::config::AppConfig;
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
