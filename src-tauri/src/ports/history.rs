use crate::domain::action::Action;
use async_trait::async_trait;

#[async_trait]
pub trait HistoryRepository: Send + Sync {
    async fn get_recent(&self, limit: usize) -> Result<Vec<Action>, String>;
    async fn record(&self, action: Action) -> Result<(), String>;
    async fn clear(&self) -> Result<(), String>;
}
