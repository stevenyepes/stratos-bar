use crate::domain::ai::Message;
use crate::domain::config::AppConfig;
use crate::ports::ai_port::AiService;
use futures_util::stream::BoxStream;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;

pub struct HttpAiService {
    client: Client,
}

impl HttpAiService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
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
                let res = self
                    .client
                    .post("https://api.openai.com/v1/chat/completions")
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
                let res = self
                    .client
                    .get("https://api.openai.com/v1/models")
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
