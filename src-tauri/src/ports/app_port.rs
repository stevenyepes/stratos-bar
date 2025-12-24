use crate::domain::apps::AppEntry;

#[cfg_attr(test, mockall::automock)]
pub trait AppRepository: Send + Sync {
    fn list_apps(&self) -> Result<Vec<AppEntry>, String>;
}
