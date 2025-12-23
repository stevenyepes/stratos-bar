use crate::domain::apps::AppEntry;

pub trait AppRepository: Send + Sync {
    fn list_apps(&self) -> Result<Vec<AppEntry>, String>;
}
