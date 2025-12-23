use crate::domain::config::AppConfig;
use crate::ports::config_port::ConfigService;
use std::fs;
use std::path::PathBuf;

pub struct FsConfigService;

impl FsConfigService {
    pub fn new() -> Self {
        Self
    }
}

impl ConfigService for FsConfigService {
    fn get_config_dir(&self) -> Option<PathBuf> {
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
