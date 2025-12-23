use tauri::{Emitter, Manager};

/// Helper to toggle the main window's visibility.
/// If visible, it hides it.
/// If hidden, it shows, raises, focuses, and emits the 'window-shown' event.
pub fn toggle_main_window(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            let _ = window.hide();
        } else {
            // Re-assert always on top to ensure it floats above others
            let _ = window.set_always_on_top(true);
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.emit("window-shown", ());
        }
    }
}
