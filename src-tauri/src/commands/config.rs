use crate::domain::config::AppConfig;
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

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    get_config_logic(&*state.config_service)
}

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    save_config_logic(&*state.config_service, &config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::config_port::MockConfigService;

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
        // Simple check, exact equality might require partialeq on AppConfig but it derives it
    }

    #[test]
    fn test_save_config() {
        let mut mock = MockConfigService::new();
        mock.expect_save_config().times(1).returning(|_| Ok(()));

        let config = AppConfig::default();
        let result = save_config_logic(&mock, &config);
        assert!(result.is_ok());
    }
}
