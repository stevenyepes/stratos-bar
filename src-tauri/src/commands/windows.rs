use crate::domain::windows::WindowEntry;
use crate::state::AppState;
use tauri::{AppHandle, LogicalSize, Manager, State};

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

#[tauri::command]
pub async fn resize_window(app_handle: AppHandle, width: u32, height: u32) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        // Ensure not maximized and is resizable
        let _ = window.unmaximize();
        let _ = window.set_fullscreen(false);
        let _ = window.set_resizable(true);

        window
            .set_size(LogicalSize::new(width, height))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
