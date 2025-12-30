use crate::domain::translation::{TranslationRequest, TranslationResult};
use crate::ports::translation_port::TranslationService;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::error::Error;

pub struct GoogleTranslationService {
    client: Client,
}

impl GoogleTranslationService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl TranslationService for GoogleTranslationService {
    async fn translate(
        &self,
        request: TranslationRequest,
    ) -> Result<TranslationResult, Box<dyn Error + Send + Sync>> {
        let source_lang = request.source_lang.unwrap_or_else(|| "auto".to_string());
        let target_lang = request.target_lang; // e.g. "en"

        let url = "https://translate.googleapis.com/translate_a/single";

        // client=gtx & sl=auto & tl=en & dt=t & q=...
        let res = self
            .client
            .get(url)
            .query(&[
                ("client", "gtx"),
                ("sl", &source_lang),
                ("tl", &target_lang),
                ("dt", "t"),
                ("q", &request.text),
            ])
            .send()
            .await?
            .json::<Value>()
            .await?;

        // Parse logic:
        // Response is roughly: [[["Target", "Source", null, null, ...]], null, "detected_lang", ...]

        let translated_text = res
            .get(0)
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                // Iterate over all segments (for multi-sentence input)
                let mut full_translation = String::new();
                for segment in arr {
                    if let Some(text) = segment.get(0).and_then(|s| s.as_str()) {
                        full_translation.push_str(text);
                    }
                }
                if full_translation.is_empty() {
                    None
                } else {
                    Some(full_translation)
                }
            })
            .ok_or("Failed to parse translation from response")?;

        let detected_source = res
            .get(2)
            .and_then(|v| v.as_str())
            .unwrap_or(&source_lang)
            .to_string();

        Ok(TranslationResult {
            original_text: request.text,
            translated_text,
            source_language: detected_source,
            target_language: target_lang,
        })
    }
}
