use crate::domain::config::{AppConfig, QuickActions};
use crate::ports::config_port::ConfigService;
use crate::state::AppState;
use tauri::State;

// Handler logic separated from Tauri state injection for easier testing
pub fn get_config_logic(service: &dyn ConfigService) -> Result<AppConfig, String> {
    Ok(service.load_config())
}

pub fn save_config_logic(service: &dyn ConfigService, config: &AppConfig) -> Result<(), String> {
    service.save_config(config)
}

pub fn get_quick_actions_logic(service: &dyn ConfigService) -> Result<QuickActions, String> {
    Ok(service.load_config().quick_actions)
}

pub fn set_quick_actions_logic(
    service: &dyn ConfigService,
    quick_actions: QuickActions,
) -> Result<(), String> {
    for aliases in quick_actions.app_aliases.values() {
        for alias in aliases {
            if !QuickActions::is_valid_alias(alias) {
                return Err(format!("invalid app alias: {alias:?}"));
            }
        }
    }
    for aliases in quick_actions.skill_aliases.values() {
        for alias in aliases {
            if !QuickActions::is_valid_alias(alias) {
                return Err(format!("invalid skill alias: {alias:?}"));
            }
        }
    }
    for keyword in quick_actions.keyword_shortcuts.keys() {
        if !QuickActions::is_valid_alias(keyword) {
            return Err(format!("invalid keyword: {keyword:?}"));
        }
    }

    let mut config = service.load_config();
    config.quick_actions = quick_actions;
    service.save_config(&config)
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    get_config_logic(&*state.config_service)
}

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    save_config_logic(&*state.config_service, &config)
}

#[tauri::command]
pub async fn get_quick_actions(state: State<'_, AppState>) -> Result<QuickActions, String> {
    get_quick_actions_logic(&*state.config_service)
}

#[tauri::command]
pub async fn set_quick_actions(
    state: State<'_, AppState>,
    quick_actions: QuickActions,
) -> Result<(), String> {
    set_quick_actions_logic(&*state.config_service, quick_actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::config_port::MockConfigService;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[test]
    fn test_get_config() {
        let mut mock = MockConfigService::new();
        let expected_config = AppConfig::default();
        let ret_config = expected_config.clone();

        mock.expect_load_config()
            .times(1)
            .returning(move || ret_config.clone());

        let result = get_config_logic(&mock);
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_config() {
        let mut mock = MockConfigService::new();
        mock.expect_save_config().times(1).returning(|_| Ok(()));

        let config = AppConfig::default();
        let result = save_config_logic(&mock, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_quick_actions_returns_seeded_defaults() {
        let mut mock = MockConfigService::new();
        let mut config = AppConfig::default();
        config.apply_defaults();
        let expected = config.quick_actions.clone();
        let ret_config = config;

        mock.expect_load_config()
            .times(1)
            .returning(move || ret_config.clone());

        let result = get_quick_actions_logic(&mock).expect("expected ok");
        assert_eq!(result.seed_initialized, true);
        assert!(!result.top_used_seed.is_empty());
        assert_eq!(result, expected);
    }

    #[test]
    fn test_set_quick_actions_validates_aliases() {
        let mut quick = QuickActions::default();
        let mut aliases = HashMap::new();
        aliases.insert(
            "google-chrome".to_string(),
            vec!["rm -rf /".to_string()],
        );
        quick.app_aliases = aliases;

        let mut mock = MockConfigService::new();
        mock.expect_load_config().times(0);
        mock.expect_save_config().times(0);

        let result = set_quick_actions_logic(&mock, quick);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid app alias"));
    }

    #[test]
    fn test_set_quick_actions_validates_keywords() {
        let mut quick = QuickActions::default();
        let mut keywords = HashMap::new();
        keywords.insert("g;cal".to_string(), "https://calendar.google.com".to_string());
        quick.keyword_shortcuts = keywords;

        let mut mock = MockConfigService::new();
        mock.expect_load_config().times(0);
        mock.expect_save_config().times(0);

        let result = set_quick_actions_logic(&mock, quick);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid keyword"));
    }

    #[test]
    fn test_set_quick_actions_persists_through_save() {
        let store: Arc<Mutex<AppConfig>> = Arc::new(Mutex::new(AppConfig::default()));
        let store_for_load = store.clone();
        let store_for_save = store.clone();

        let mut mock = MockConfigService::new();
        mock.expect_load_config()
            .times(1)
            .returning(move || store_for_load.lock().unwrap().clone());
        mock.expect_save_config()
            .times(1)
            .returning(move |cfg: &AppConfig| {
                *store_for_save.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let mut quick = QuickActions::default();
        let mut app_aliases = HashMap::new();
        app_aliases.insert(
            "google-chrome".to_string(),
            vec!["browser".to_string(), "browse".to_string()],
        );
        quick.app_aliases = app_aliases;
        let mut skill_aliases = HashMap::new();
        skill_aliases.insert("rephrase".to_string(), vec!["reword".to_string()]);
        quick.skill_aliases = skill_aliases;
        let mut keyword_shortcuts = HashMap::new();
        keyword_shortcuts.insert("slack".to_string(), "slack-desktop".to_string());
        keyword_shortcuts.insert(
            "g cal".to_string(),
            "https://calendar.google.com".to_string(),
        );
        quick.keyword_shortcuts = keyword_shortcuts;
        quick.pinned_favorites = vec!["google-chrome".to_string(), "code".to_string()];

        set_quick_actions_logic(&mock, quick.clone()).expect("expected ok");

        let stored = store.lock().unwrap().clone();
        assert_eq!(
            stored.quick_actions.app_aliases.get("google-chrome"),
            Some(&vec!["browser".to_string(), "browse".to_string()])
        );
        assert_eq!(
            stored.quick_actions.keyword_shortcuts.get("slack"),
            Some(&"slack-desktop".to_string())
        );
        assert_eq!(stored.quick_actions.pinned_favorites, quick.pinned_favorites);
    }

    #[test]
    fn aliases_round_trip() {
        let store: Arc<Mutex<AppConfig>> = Arc::new(Mutex::new(AppConfig::default()));
        let store_for_load = store.clone();
        let store_for_save = store.clone();

        let mut mock = MockConfigService::new();
        mock.expect_load_config()
            .returning(move || store_for_load.lock().unwrap().clone());
        mock.expect_save_config()
            .returning(move |cfg: &AppConfig| {
                *store_for_save.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let mut quick = QuickActions::default();
        quick
            .set_app_aliases("google-chrome", vec!["browser".to_string()])
            .unwrap();

        set_quick_actions_logic(&mock, quick).expect("expected ok");

        let reloaded = get_quick_actions_logic(&mock).expect("expected ok");
        assert_eq!(
            reloaded.app_aliases.get("google-chrome"),
            Some(&vec!["browser".to_string()])
        );
    }
}

