use crate::domain::action::Action;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_recent_actions(
    state: State<'_, AppState>,
    limit: usize,
) -> Result<Vec<Action>, String> {
    state.history_repository.get_recent(limit).await
}

#[tauri::command]
pub async fn record_action(state: State<'_, AppState>, action: Action) -> Result<(), String> {
    state.history_repository.record(action).await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.history_repository.clear().await
}
