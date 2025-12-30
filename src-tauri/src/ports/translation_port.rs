use crate::domain::translation::{TranslationRequest, TranslationResult};
use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait TranslationService: Send + Sync {
    async fn translate(
        &self,
        request: TranslationRequest,
    ) -> Result<TranslationResult, Box<dyn Error + Send + Sync>>;
}
