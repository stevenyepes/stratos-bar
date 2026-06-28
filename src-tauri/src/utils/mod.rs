use tauri::{Emitter, Manager};

pub mod fuzzy;

pub fn toggle_main_window(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            let _ = window.hide();
        } else {
            let _ = window.set_always_on_top(true);
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.emit("window-shown", ());
        }
    }
}