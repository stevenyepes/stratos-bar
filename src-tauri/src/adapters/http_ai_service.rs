use crate::domain::ai::Message;
use crate::domain::config::AppConfig;
use crate::ports::ai_port::AiService;
use futures_util::stream::BoxStream;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;

pub struct HttpAiService {
    client: Client,
    openai_base_url: String,
}

impl HttpAiService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            openai_base_url: "https://api.openai.com/v1".to_string(),
        }
    }

    #[cfg(test)]
    pub fn new_with_url(openai_url: String) -> Self {
        Self {
            client: Client::new(),
            openai_base_url: openai_url,
        }
    }
}

#[async_trait::async_trait]
impl AiService for HttpAiService {
    async fn stream_completion<'a>(
        &'a self,
        config: &'a AppConfig,
        messages: Vec<Message>,
    ) -> Result<BoxStream<'a, Result<String, String>>, String> {
        if config.preferred_model == "local" {
            // Ollama
            let url = format!(
                "{}/api/chat",
                config
                    .local_model_url
                    .as_deref()
                    .unwrap_or("http://127.0.0.1:11434")
            );

            let res = self
                .client
                .post(url)
                .json(&json!({
                    "model": config.ollama_model.as_deref().unwrap_or("llama3"),
                    "messages": messages,
                    "stream": true
                }))
                .send()
                .await
                .map_err(|e| e.to_string())?;

            let stream = res.bytes_stream().map(|item| match item {
                Ok(chunk) => {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    let mut content = String::new();
                    for line in chunk_str.lines() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                            if let Some(c) = val["message"]["content"].as_str() {
                                content.push_str(c);
                            }
                        }
                    }
                    Ok(content)
                }
                Err(e) => Err(e.to_string()),
            });

            // Filter empty contents? Actually the map above aggregates per chunk, but might return empty strings if chunk had no content.
            // Using Box::pin to create BoxStream
            Ok(Box::pin(stream))
        } else {
            // OpenAI
            if let Some(key) = &config.openai_api_key {
                let url = format!("{}/chat/completions", self.openai_base_url);
                let res = self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .json(&json!({
                        "model": "gpt-4o",
                        "messages": messages,
                        "stream": true
                    }))
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;

                let stream = res.bytes_stream().map(|item| match item {
                    Ok(chunk) => {
                        let chunk_str = String::from_utf8_lossy(&chunk);
                        let mut content = String::new();
                        for line in chunk_str.lines() {
                            let line = line.trim();
                            if line.starts_with("data: ") {
                                let data = &line[6..];
                                if data == "[DONE]" {
                                    break;
                                }
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(c) = val["choices"][0]["delta"]["content"].as_str()
                                    {
                                        content.push_str(c);
                                    }
                                }
                            }
                        }
                        Ok(content)
                    }
                    Err(e) => Err(e.to_string()),
                });

                Ok(Box::pin(stream))
            } else {
                Err("No OpenAI API Key configured".to_string())
            }
        }
    }

    async fn check_connection(&self, config: &AppConfig) -> Result<String, String> {
        let start = std::time::Instant::now();

        if config.preferred_model == "local" {
            let url = format!(
                "{}/api/tags",
                config
                    .local_model_url
                    .as_deref()
                    .unwrap_or("http://127.0.0.1:11434")
            );

            let res = self
                .client
                .get(&url)
                .timeout(std::time::Duration::from_secs(2))
                .send()
                .await;

            match res {
                Ok(response) => {
                    if response.status().is_success() {
                        let duration = start.elapsed();
                        Ok(format!("Connected to Ollama ({:?})", duration))
                    } else {
                        Err(format!("Ollama returned error: {}", response.status()))
                    }
                }
                Err(e) => Err(format!("Failed to connect to Ollama: {}", e)),
            }
        } else {
            if let Some(key) = &config.openai_api_key {
                let url = format!("{}/models", self.openai_base_url);
                let res = self
                    .client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .timeout(std::time::Duration::from_secs(5))
                    .send()
                    .await;

                match res {
                    Ok(response) => {
                        if response.status().is_success() {
                            let duration = start.elapsed();
                            Ok(format!("Connected to OpenAI ({:?})", duration))
                        } else {
                            Err(format!("OpenAI returned error: {}", response.status()))
                        }
                    }
                    Err(e) => Err(format!("Failed to connect to OpenAI: {}", e)),
                }
            } else {
                Err("No OpenAI API Key configured".to_string())
            }
        }
    }

    async fn list_models(&self, config: &AppConfig) -> Result<Vec<String>, String> {
        let url = format!(
            "{}/api/tags",
            config
                .local_model_url
                .as_deref()
                .unwrap_or("http://127.0.0.1:11434")
        );

        let res = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

        if let Some(models) = body["models"].as_array() {
            let names = models
                .iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect();
            Ok(names)
        } else {
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_local_ollama_completion() {
        let mock_server = MockServer::start().await;
        let service = HttpAiService::new();

        let response_body = json!({
            "message": {
                "content": "Hello world"
            },
            "done": false
        });

        // Mock the ollama chat endpoint
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let mut config = AppConfig::default();
        config.preferred_model = "local".to_string();
        config.local_model_url = Some(mock_server.uri());

        let stream = service.stream_completion(&config, vec![]).await.unwrap();
        let result: Vec<String> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert!(!result.is_empty());
        assert_eq!(result.concat(), "Hello world");
    }

    #[tokio::test]
    async fn test_openai_completion() {
        let mock_server = MockServer::start().await;
        let service = HttpAiService::new_with_url(mock_server.uri());

        // Mock OpenAI style stream response
        let chunk = json!({
            "choices": [{
                "delta": {
                    "content": "Hello OpenAI"
                }
            }]
        });
        let body = format!(
            "data: {}\n\ndata: [DONE]",
            serde_json::to_string(&chunk).unwrap()
        );

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&mock_server)
            .await;

        let mut config = AppConfig::default();
        config.preferred_model = "cloud".to_string();
        config.openai_api_key = Some("test-key".to_string());

        let stream = service.stream_completion(&config, vec![]).await.unwrap();
        let result: Vec<String> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(result.concat(), "Hello OpenAI");
    }
}
