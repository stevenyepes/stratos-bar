use crate::domain::apps::{AppEntry, ScoredApp};
use std::path::PathBuf;

#[cfg_attr(test, mockall::automock)]
pub trait AppRepository: Send + Sync {
    fn list_apps(&self) -> Result<Vec<AppEntry>, String>;

    fn rescan(&self) -> Result<Vec<AppEntry>, String> {
        self.list_apps()
    }

    fn set_custom_paths(&self, _paths: Vec<PathBuf>) {}

    fn search_apps(&self, _query: &str, _limit: usize) -> Vec<ScoredApp> {
        Vec::new()
    }
}
