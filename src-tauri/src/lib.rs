pub mod adapters;
pub mod domain;
pub mod ports;

use adapters::cached_icon_resolver::CachedIconResolver;
use adapters::fs_app_repository::FsAppRepository;
use adapters::fs_config_service::FsConfigService;
use adapters::http_ai_service::HttpAiService;
use adapters::linux_window_service::LinuxWindowService;
use domain::ai::Message;
use domain::apps::AppEntry;
use domain::config::AppConfig;
use domain::windows::WindowEntry;
use futures_util::StreamExt;
use ports::ai_port::AiService;
use ports::app_port::AppRepository;
use ports::config_port::ConfigService;
use ports::icon_port::IconResolver;
use ports::window_port::WindowService;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};
use walkdir::WalkDir;

struct AppState {
    app_repository: Arc<dyn AppRepository>,
    window_service: Arc<dyn WindowService>,
    config_service: Arc<dyn ConfigService>,
    icon_resolver: Arc<dyn IconResolver>,
    ai_service: Arc<dyn AiService>,
}

// Reuse PaletteTray as is, but it might need access to icon resolver if we wanted to be strict.
// For now, it uses include_bytes! so it's fine.
struct PaletteTray {
    handle: tauri::AppHandle,
}

impl ksni::Tray for PaletteTray {
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let mut icons = Vec::new();

        let sizes = vec![
            (include_bytes!("../icons/tray-icon-22.png").to_vec(), 22),
            (include_bytes!("../icons/tray-icon-32.png").to_vec(), 32),
            (include_bytes!("../icons/tray-icon-48.png").to_vec(), 48),
        ];

        for (data, _expected_size) in sizes {
            if let Ok(image) = image::load_from_memory(&data) {
                let rgba = image.to_rgba8();
                let width = rgba.width() as i32;
                let height = rgba.height() as i32;
                let raw_rgba = rgba.into_raw();

                let mut argb = Vec::with_capacity(raw_rgba.len());
                for chunk in raw_rgba.chunks(4) {
                    if chunk.len() == 4 {
                        argb.push(chunk[3]); // A
                        argb.push(chunk[0]); // R
                        argb.push(chunk[1]); // G
                        argb.push(chunk[2]); // B
                    }
                }

                icons.push(ksni::Icon {
                    width,
                    height,
                    data: argb,
                });
            }
        }

        if !icons.is_empty() {
            icons
        } else {
            if let Some(img) = self.handle.default_window_icon() {
                vec![ksni::Icon {
                    width: img.width() as i32,
                    height: img.height() as i32,
                    data: img.rgba().to_vec(),
                }]
            } else {
                vec![]
            }
        }
    }

    fn id(&self) -> String {
        "stratos-bar".to_string()
    }

    fn title(&self) -> String {
        "stratos-bar".to_string()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        if let Some(window) = self.handle.get_webview_window("main") {
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            } else {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Show".into(),
                activate: Box::new(|this: &mut Self| {
                    if let Some(window) = this.handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    this.handle.exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

// Commands

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config_service.load_config())
}

#[tauri::command]
async fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    state.config_service.save_config(&config)
}

#[tauri::command]
async fn list_apps(state: State<'_, AppState>) -> Result<Vec<AppEntry>, String> {
    state.app_repository.list_apps()
}

#[tauri::command]
async fn list_windows(state: State<'_, AppState>) -> Result<Vec<WindowEntry>, String> {
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
async fn focus_window(state: State<'_, AppState>, address: String) -> Result<(), String> {
    state.window_service.focus_window(&address)
}

#[tauri::command]
async fn launch_app(exec_cmd: String) -> Result<(), String> {
    // Helper to clean exec command (inline or moved to util if shared)
    // We can keep the helper function here or move it to a util module.
    // For now inline helper logic from previous lib.rs to avoid losing it.

    fn parse_exec_command(exec_cmd: &str) -> Option<(String, Vec<String>)> {
        let cleaned = exec_cmd
            .replace("%f", "")
            .replace("%F", "")
            .replace("%u", "")
            .replace("%U", "")
            .replace("%i", "")
            .replace("%c", "")
            .replace("%k", "");

        let parts = shell_words::split(&cleaned).ok()?;
        if parts.is_empty() {
            return None;
        }

        let cmd = parts[0].clone();
        let args = parts[1..].to_vec();
        Some((cmd, args))
    }

    let (cmd, args) = parse_exec_command(&exec_cmd).ok_or_else(|| "Empty command".to_string())?;

    std::process::Command::new(cmd)
        .args(args)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn open_entity(path: String) -> Result<(), String> {
    open::that(path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn ask_ai(
    window: tauri::Window,
    state: State<'_, AppState>,
    messages: Vec<Message>,
) -> Result<(), String> {
    let config = state.config_service.load_config();

    window
        .emit("ai-response-start", ())
        .map_err(|e| e.to_string())?;

    match state.ai_service.stream_completion(&config, messages).await {
        Ok(mut stream) => {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(content) => {
                        window
                            .emit("ai-response-chunk", content)
                            .map_err(|e| e.to_string())?;
                    }
                    Err(e) => {
                        window
                            .emit("ai-response-error", e.clone())
                            .map_err(|e1| e1.to_string())?;
                        return Err(e);
                    }
                }
            }
        }
        Err(e) => {
            window
                .emit("ai-response-error", e.clone())
                .map_err(|e1| e1.to_string())?;
            return Err(e);
        }
    }

    window
        .emit("ai-response-done", ())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn check_ai_connection(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config_service.load_config();
    state.ai_service.check_connection(&config).await
}

#[tauri::command]
async fn list_ollama_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let config = state.config_service.load_config();
    match state.ai_service.list_models(&config).await {
        Ok(models) => Ok(models),
        Err(_) => Ok(vec![]), // Graceful fallback
    }
}

#[tauri::command]
async fn list_scripts(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let mut scripts = Vec::new();
    if let Some(config_dir) = state.config_service.get_config_dir() {
        let scripts_dir = config_dir.join("scripts");
        if scripts_dir.exists() {
            for entry in WalkDir::new(scripts_dir)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.is_file() {
                    // check executable bit? or just extension?
                    // allow .sh, .py, .js or no extension
                    #[cfg(unix)]
                    use std::os::unix::fs::PermissionsExt;

                    let is_executable = if let Ok(metadata) = p.metadata() {
                        metadata.permissions().mode() & 0o111 != 0
                    } else {
                        false
                    };

                    if is_executable || p.extension().map_or(false, |e| e == "sh" || e == "py") {
                        scripts.push(p.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    Ok(scripts)
}

#[tauri::command]
async fn search_files(query: String, path: String) -> Result<Vec<String>, String> {
    let mut results = Vec::new();
    // Default to home if path is empty? Or enforce path.
    // For now, simple walk
    for entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
        let path_str = entry.path().to_string_lossy();
        if path_str.to_lowercase().contains(&query.to_lowercase()) {
            results.push(path_str.to_string());
            if results.len() >= 50 {
                break;
            }
        }
    }
    Ok(results)
}

#[tauri::command]
async fn get_selection_context() -> Result<String, String> {
    use arboard::Clipboard;

    if let Ok(mut clipboard) = Clipboard::new() {
        if let Ok(text) = clipboard.get_text() {
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    Err("No text found in selection or clipboard".to_string())
}

#[tauri::command]
async fn copy_to_clipboard(text: String) -> Result<(), String> {
    println!("DEBUG: copy_to_clipboard called with text: '{}'", text);
    use arboard::Clipboard;

    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Instantiate Adapters
            let config_service = Arc::new(FsConfigService::new());
            let icon_resolver = Arc::new(CachedIconResolver::new());
            // FsAppRepository needs icon resolver
            let app_repository = Arc::new(FsAppRepository::new(icon_resolver.clone()));
            let window_service = Arc::new(LinuxWindowService::new());
            let ai_service = Arc::new(HttpAiService::new());

            // Manage State
            app.manage(AppState {
                app_repository,
                window_service,
                config_service: config_service.clone(),
                icon_resolver: icon_resolver.clone(),
                ai_service,
            });

            // Initialize KSNI Tray Service
            let handle = app.handle().clone();
            let tray = PaletteTray { handle };
            let service = ksni::TrayService::new(tray);
            let _handle = service.handle();

            service.spawn();

            Ok(())
        })
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.emit("open_request", ());
            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcut("Super+Space")
                .expect("Failed to register shortcut")
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if shortcut.matches(Modifiers::SUPER, Code::Space) {
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            greet,
            search_files,
            list_apps,
            launch_app,
            open_entity,
            get_config,
            save_config,
            ask_ai,
            list_scripts,
            list_ollama_models,
            get_selection_context,
            copy_to_clipboard,
            list_windows,
            focus_window,
            check_ai_connection
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        let result = greet("World");
        assert_eq!(result, "Hello, World! You've been greeted from Rust!");
    }
}
