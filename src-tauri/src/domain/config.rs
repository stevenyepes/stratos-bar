use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
pub struct ScriptConfig {
    pub id: String,
    pub alias: String,
    pub path: String,
    pub args: Option<String>,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct QuickActions {
    #[serde(default)]
    pub pinned_favorites: Vec<String>,
    #[serde(default)]
    pub app_aliases: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub skill_aliases: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub keyword_shortcuts: HashMap<String, String>,
    #[serde(default)]
    pub top_used_seed: Vec<String>,
    #[serde(default)]
    pub seed_initialized: bool,
}

impl QuickActions {
    pub const ALIAS_MAX_LEN: usize = 32;

    pub fn is_valid_alias(alias: &str) -> bool {
        if alias.is_empty() || alias.chars().count() > Self::ALIAS_MAX_LEN {
            return false;
        }
        alias
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ' ' || c == '.' || c == '-')
    }

    pub fn apply_defaults(&mut self) {
        if !self.seed_initialized {
            if self.top_used_seed.is_empty() {
                self.top_used_seed = default_top_used_apps();
            }
            self.seed_initialized = true;
        }
    }

    pub fn set_app_aliases(
        &mut self,
        app_id: &str,
        aliases: Vec<String>,
    ) -> Result<(), String> {
        for alias in &aliases {
            if !Self::is_valid_alias(alias) {
                return Err(format!("invalid alias: {alias:?}"));
            }
        }
        if aliases.is_empty() {
            self.app_aliases.remove(app_id);
        } else {
            self.app_aliases.insert(app_id.to_string(), aliases);
        }
        Ok(())
    }

    pub fn set_skill_aliases(
        &mut self,
        skill_id: &str,
        aliases: Vec<String>,
    ) -> Result<(), String> {
        for alias in &aliases {
            if !Self::is_valid_alias(alias) {
                return Err(format!("invalid alias: {alias:?}"));
            }
        }
        if aliases.is_empty() {
            self.skill_aliases.remove(skill_id);
        } else {
            self.skill_aliases.insert(skill_id.to_string(), aliases);
        }
        Ok(())
    }

    pub fn set_keyword_shortcut(&mut self, keyword: &str, target: &str) -> Result<(), String> {
        if !Self::is_valid_alias(keyword) {
            return Err(format!("invalid keyword: {keyword:?}"));
        }
        if target.is_empty() {
            self.keyword_shortcuts.remove(keyword);
        } else {
            self.keyword_shortcuts
                .insert(keyword.to_string(), target.to_string());
        }
        Ok(())
    }

    pub fn pin_favorite(&mut self, app_id: &str) {
        if !self.pinned_favorites.iter().any(|p| p == app_id) {
            self.pinned_favorites.push(app_id.to_string());
        }
    }

    pub fn unpin_favorite(&mut self, app_id: &str) {
        self.pinned_favorites.retain(|p| p != app_id);
    }
}

fn default_top_used_apps() -> Vec<String> {
    vec![
        "google-chrome".to_string(),
        "firefox".to_string(),
        "code".to_string(),
        "cursor".to_string(),
        "kitty".to_string(),
        "alacritty".to_string(),
        "thunderbird".to_string(),
        "spotify".to_string(),
    ]
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

    #[serde(default)]
    pub scripts: Vec<ScriptConfig>,

    #[serde(default)]
    pub window_scale: Option<f32>,

    pub theme: Option<ThemeConfig>,

    #[serde(default)]
    pub file_search: FileSearchConfig,

    #[serde(default)]
    pub custom_app_dirs: Vec<PathBuf>,

    #[serde(default)]
    pub quick_actions: QuickActions,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct FileSearchConfig {
    #[serde(default)]
    pub include_hidden: bool,
}

impl AppConfig {
    pub fn apply_defaults(&mut self) {
        if self.preferred_model.is_empty() {
            self.preferred_model = "local".to_string();
            self.local_model_url = Some("http://localhost:11434".to_string());
            self.ollama_model = Some("llama3".to_string());
            self.ollama_model = Some("llama3".to_string());
        }

        if self.window_scale.is_none() {
            self.window_scale = Some(0.2); // Default to 20% of screen width
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

        self.quick_actions.apply_defaults();
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

    #[test]
    fn test_apply_defaults_seeds_quick_actions_on_first_run() {
        let mut config = AppConfig::default();
        assert!(!config.quick_actions.seed_initialized);
        assert!(config.quick_actions.top_used_seed.is_empty());

        config.apply_defaults();

        assert!(config.quick_actions.seed_initialized);
        assert!(!config.quick_actions.top_used_seed.is_empty());
        assert!(config
            .quick_actions
            .top_used_seed
            .contains(&"google-chrome".to_string()));
    }

    #[test]
    fn test_apply_defaults_preserves_quick_actions_seed() {
        let mut config = AppConfig::default();
        config.quick_actions.seed_initialized = true;
        config.quick_actions.top_used_seed = vec!["custom-app".to_string()];

        config.apply_defaults();

        assert_eq!(
            config.quick_actions.top_used_seed,
            vec!["custom-app".to_string()]
        );
        assert!(config.quick_actions.seed_initialized);
    }

    #[test]
    fn test_is_valid_alias_accepts_alnum_underscore_dot_dash_space() {
        assert!(QuickActions::is_valid_alias("browser"));
        assert!(QuickActions::is_valid_alias("g cal"));
        assert!(QuickActions::is_valid_alias("g_cal"));
        assert!(QuickActions::is_valid_alias("g-cal"));
        assert!(QuickActions::is_valid_alias("g.cal"));
        assert!(QuickActions::is_valid_alias("Slack123"));
    }

    #[test]
    fn test_is_valid_alias_rejects_empty_too_long_and_disallowed_chars() {
        assert!(!QuickActions::is_valid_alias(""));
        assert!(!QuickActions::is_valid_alias(&"a".repeat(33)));
        assert!(!QuickActions::is_valid_alias("g;cal"));
        assert!(!QuickActions::is_valid_alias("rm -rf /"));
        assert!(!QuickActions::is_valid_alias("hello/world"));
        assert!(!QuickActions::is_valid_alias("héllo"));
    }

    #[test]
    fn test_quick_actions_serde_round_trip() {
        let mut quick = QuickActions::default();
        quick
            .set_app_aliases("google-chrome", vec!["browser".to_string()])
            .unwrap();
        quick
            .set_skill_aliases("rephrase", vec!["rewrite".to_string()])
            .unwrap();
        quick
            .set_keyword_shortcut("slack", "slack-desktop")
            .unwrap();
        quick
            .set_keyword_shortcut("g cal", "https://calendar.google.com")
            .unwrap();
        quick.pin_favorite("code");
        quick.pin_favorite("cursor");

        let json = serde_json::to_string(&quick).unwrap();
        let parsed: QuickActions = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, quick);
    }

    #[test]
    fn test_quick_actions_set_app_aliases_rejects_invalid() {
        let mut quick = QuickActions::default();
        let result = quick.set_app_aliases("google-chrome", vec!["rm -rf /".to_string()]);
        assert!(result.is_err());
        assert!(quick.app_aliases.is_empty());
    }

    #[test]
    fn test_quick_actions_unpin_favorite_removes_entry() {
        let mut quick = QuickActions::default();
        quick.pin_favorite("code");
        quick.pin_favorite("cursor");
        quick.unpin_favorite("code");
        assert_eq!(quick.pinned_favorites, vec!["cursor".to_string()]);
    }
}
