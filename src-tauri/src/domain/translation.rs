use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationResult {
    pub original_text: String,
    pub translated_text: String,
    pub source_language: String,
    pub target_language: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationRequest {
    pub text: String,
    pub source_lang: Option<String>,
    pub target_lang: String,
}
