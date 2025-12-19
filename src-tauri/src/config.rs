use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct ThemeConfig {
    pub name: String,
    pub primary: String,
    pub secondary: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    #[serde(default)]
    pub is_custom: bool,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct AiTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt_template: String,
    pub keywords: Vec<String>,
    pub icon: String,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct AppConfig {
    pub openai_api_key: Option<String>,
    pub local_model_url: Option<String>, // e.g. http://localhost:11434
    pub preferred_model: String,         // "local" or "cloud"
    pub ollama_model: Option<String>,    // Specific model name e.g. "llama3"

    #[serde(default)]
    pub ai_tools: Vec<AiTool>,

    #[serde(default)]
    pub shortcuts: HashMap<String, String>, // trigger -> tool_id or app_name

    pub theme: Option<ThemeConfig>,
}

impl AppConfig {
    pub fn apply_defaults(&mut self) {
        if self.preferred_model.is_empty() {
            self.preferred_model = "local".to_string();
            self.local_model_url = Some("http://localhost:11434".to_string());
            self.ollama_model = Some("llama3".to_string());
        }

        if self.theme.is_none() {
            self.theme = Some(ThemeConfig {
                name: "Tokyo Night".to_string(),
                primary: "#7aa2f7".to_string(),
                secondary: "#bb9af7".to_string(),
                background: "#1a1b26".to_string(),
                surface: "#24283b".to_string(),
                text: "#c0caf5".to_string(),
                is_custom: false,
            });
        }

        if self.ai_tools.is_empty() {
            self.ai_tools.push(AiTool {
                 id: "rephrase".to_string(),
                 name: "Rephrase Selection".to_string(),
                 description: "Improve clarity and grammar".to_string(),
                 prompt_template: "Identity the language of the following text and rephrase it to improve clarity and grammar. Return ONLY the improved text wrapped in a markdown code block (using ```text or the appropriate language). Do not add any conversational text.\n\nText:\n{{selection}}".to_string(),
                 keywords: vec!["rephrase".to_string(), "rewrite".to_string(), "fix".to_string(), "improve".to_string()],
                 icon: "✏️".to_string()
             });
        }
    }
}

pub struct ConfigManager;

impl ConfigManager {
    pub fn get_config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("stratos-bar"))
    }

    pub fn load_config() -> AppConfig {
        let mut config = if let Some(config_dir) = Self::get_config_dir() {
            let config_path = config_dir.join("config.json");
            if config_path.exists() {
                if let Ok(content) = fs::read_to_string(config_path) {
                    serde_json::from_str(&content).unwrap_or_default()
                } else {
                    AppConfig::default()
                }
            } else {
                AppConfig::default()
            }
        } else {
            AppConfig::default()
        };

        // Set Defaults if empty
        config.apply_defaults();

        config
    }

    pub fn save_config(config: &AppConfig) -> Result<(), String> {
        if let Some(config_dir) = Self::get_config_dir() {
            if !config_dir.exists() {
                fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
            }
            let config_path = config_dir.join("config.json");
            let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
            fs::write(config_path, content).map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Could not find config directory".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_defaults() {
        let mut config = AppConfig::default();

        // Initially empty/defaults
        assert!(config.preferred_model.is_empty());
        assert!(config.theme.is_none());
        assert!(config.ai_tools.is_empty());

        // Apply defaults
        config.apply_defaults();

        // Check defaults
        assert_eq!(config.preferred_model, "local");
        assert_eq!(
            config.local_model_url.as_deref(),
            Some("http://localhost:11434")
        );
        assert_eq!(config.ollama_model.as_deref(), Some("llama3"));

        // Theme
        assert!(config.theme.is_some());
        let theme = config.theme.as_ref().unwrap();
        assert_eq!(theme.name, "Tokyo Night");

        // AI Tools
        assert!(!config.ai_tools.is_empty());
        assert_eq!(config.ai_tools[0].id, "rephrase");
    }

    #[test]
    fn test_apply_defaults_preserves_existing() {
        let mut config = AppConfig::default();
        config.preferred_model = "cloud".to_string();
        config.theme = Some(ThemeConfig {
            name: "Custom".to_string(),
            ..Default::default()
        });

        config.apply_defaults();

        // Should preserve user settings
        assert_eq!(config.preferred_model, "cloud");
        assert_eq!(config.theme.as_ref().unwrap().name, "Custom");

        // But should still fill in missing ones (ai_tools)
        assert!(!config.ai_tools.is_empty());
    }
}
