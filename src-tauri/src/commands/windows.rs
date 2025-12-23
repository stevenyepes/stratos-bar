use crate::domain::windows::WindowEntry;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_windows(state: State<'_, AppState>) -> Result<Vec<WindowEntry>, String> {
    let mut windows = state.window_service.list_windows()?;

    // Enrich with icons
    for window in &mut windows {
        if window.icon.is_none() {
            window.icon = state
                .icon_resolver
                .resolve_icon(&window.class.to_lowercase());
        }
    }

    Ok(windows)
}

#[tauri::command]
pub async fn focus_window(state: State<'_, AppState>, address: String) -> Result<(), String> {
    state.window_service.focus_window(&address)
}
