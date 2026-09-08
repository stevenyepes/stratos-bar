use crate::domain::action::Action;
use async_trait::async_trait;
use std::collections::HashMap;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait HistoryRepository: Send + Sync {
    async fn get_recent(&self, limit: usize) -> Result<Vec<Action>, String>;
    async fn record(&self, action: Action) -> Result<(), String>;
    async fn clear(&self) -> Result<(), String>;
    async fn rekey_app_ids(&self, mapping: &HashMap<String, String>) -> Result<(), String>;
}
