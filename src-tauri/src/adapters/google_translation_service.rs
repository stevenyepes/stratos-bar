use crate::domain::translation::{TranslationRequest, TranslationResult};
use crate::ports::translation_port::TranslationService;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::error::Error;

pub struct GoogleTranslationService {
    client: Client,
    base_url: String,
}

impl GoogleTranslationService {
    pub fn new(base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| {
                "https://translate.googleapis.com/translate_a/single".to_string()
            }),
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

        // client=gtx & sl=auto & tl=en & dt=t & q=...
        let res = self
            .client
            .get(&self.base_url)
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
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_translate_success() {
        let mock_server = MockServer::start().await;

        // Simplified Google Translate API response
        // [[["Hola","Hello",null,null,10]],null,"en",...]
        let response_body = serde_json::json!([[["Hola", "Hello", null, null, 10]], null, "en"]);

        Mock::given(method("GET"))
            .and(path("/translate_a/single"))
            .and(query_param("sl", "auto"))
            .and(query_param("tl", "es"))
            .and(query_param("q", "Hello"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        // Use the mock server URI as base_url
        // Note: mock_server.uri() returns "http://127.0.0.1:xxx".
        // Our service appends "?..." to it.
        // However, the service code does: let url = format!("{}/translate_a/single", self.base_url); NO
        // The service code does: client.get(&self.base_url)
        // So we need to pass the FULL path in the base_url or change the service logic to append path.
        // Currently service logic: client.get(&self.base_url)
        // And default base_url is "https://translate.googleapis.com/translate_a/single"
        // So for the mock, we should pass "http://127.0.0.1:xxx/translate_a/single"

        let mut base_url = mock_server.uri();
        base_url.push_str("/translate_a/single");

        let service = GoogleTranslationService::new(Some(base_url));

        let result = service
            .translate(TranslationRequest {
                text: "Hello".to_string(),
                source_lang: None,
                target_lang: "es".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(result.translated_text, "Hola");
        assert_eq!(result.source_language, "en");
        assert_eq!(result.target_language, "es");
    }
}
