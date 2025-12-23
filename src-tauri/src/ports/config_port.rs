use crate::domain::config::AppConfig;
use std::path::PathBuf;

pub trait ConfigService: Send + Sync {
    fn load_config(&self) -> AppConfig;
    fn save_config(&self, config: &AppConfig) -> Result<(), String>;
    fn get_config_dir(&self) -> Option<PathBuf>;
}
