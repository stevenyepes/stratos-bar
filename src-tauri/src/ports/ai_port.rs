use crate::domain::ai::Message;
use crate::domain::config::AppConfig;
use futures_util::stream::BoxStream;

#[async_trait::async_trait]
pub trait AiService: Send + Sync {
    async fn stream_completion<'a>(
        &'a self,
        config: &'a AppConfig,
        messages: Vec<Message>,
    ) -> Result<BoxStream<'a, Result<String, String>>, String>;

    async fn check_connection(&self, config: &AppConfig) -> Result<String, String>;

    async fn list_models(&self, config: &AppConfig) -> Result<Vec<String>, String>;
}
