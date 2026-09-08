use crate::domain::apps::AppEntry;
use std::path::PathBuf;

#[cfg_attr(test, mockall::automock)]
pub trait AppRepository: Send + Sync {
    fn list_apps(&self) -> Result<Vec<AppEntry>, String>;

    fn rescan(&self) -> Result<Vec<AppEntry>, String> {
        self.list_apps()
    }

    fn set_custom_paths(&self, _paths: Vec<PathBuf>) {}

    fn set_icon_scale(&self, _scale: u16) {}
}
