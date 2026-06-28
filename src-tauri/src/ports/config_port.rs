use crate::domain::config::{AppConfig, QuickActions};
use std::path::PathBuf;

#[cfg_attr(test, mockall::automock)]
pub trait ConfigService: Send + Sync {
    fn load_config(&self) -> AppConfig;
    fn save_config(&self, config: &AppConfig) -> Result<(), String>;
    fn get_config_dir(&self) -> Option<PathBuf>;

    fn set_quick_actions(&self, _actions: QuickActions) -> Result<(), String> {
        Ok(())
    }
}
