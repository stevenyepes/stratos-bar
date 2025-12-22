use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};
use walkdir::WalkDir;

static ICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

// Public for testing, or just use within module
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

struct PaletteTray {
    handle: tauri::AppHandle,
}

impl ksni::Tray for PaletteTray {
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
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

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(serde::Serialize)]
struct AppEntry {
    name: String,
    exec: String,
    icon: Option<String>,
}

mod window_manager;
use window_manager::WindowManager;

mod config;

use config::{AppConfig, ConfigManager};
use serde_json::json;

#[tauri::command]
async fn get_config() -> Result<AppConfig, String> {
    Ok(ConfigManager::load_config())
}

#[tauri::command]
async fn save_config(config: AppConfig) -> Result<(), String> {
    ConfigManager::save_config(&config)
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Message {
    role: String,
    content: String,
}

#[tauri::command]
async fn ask_ai(window: tauri::Window, messages: Vec<Message>) -> Result<(), String> {
    let config = ConfigManager::load_config();
    let client = reqwest::Client::new();

    window
        .emit("ai-response-start", ())
        .map_err(|e| e.to_string())?;

    if config.preferred_model == "local" {
        // Ollama streaming
        let url = format!(
            "{}/api/chat",
            config
                .local_model_url
                .unwrap_or("http://127.0.0.1:11434".to_string())
        );

        let res = client
            .post(url)
            .json(&json!({
                "model": config.ollama_model.unwrap_or("llama3".to_string()),
                "messages": messages,
                "stream": true
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            let chunk = item.map_err(|e| e.to_string())?;
            let chunk_str = String::from_utf8_lossy(&chunk);

            for line in chunk_str.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(content) = val["message"]["content"].as_str() {
                        window
                            .emit("ai-response-chunk", content)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    } else {
        // Cloud (OpenAI)
        if let Some(key) = config.openai_api_key {
            let res = client
                .post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", key))
                .json(&json!({
                    "model": "gpt-4o",
                    "messages": messages,
                    "stream": true
                }))
                .send()
                .await
                .map_err(|e| e.to_string())?;

            let mut stream = res.bytes_stream();
            while let Some(item) = stream.next().await {
                let chunk = item.map_err(|e| e.to_string())?;
                let chunk_str = String::from_utf8_lossy(&chunk);

                for line in chunk_str.lines() {
                    let line = line.trim();
                    if line.starts_with("data: ") {
                        let data = &line[6..];
                        if data == "[DONE]" {
                            break;
                        }
                        // Handle potential partial JSONs or errors by ignoring malformed lines
                        // In production, we'd want a proper buffer
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(content) = val["choices"][0]["delta"]["content"].as_str() {
                                window
                                    .emit("ai-response-chunk", content)
                                    .map_err(|e| e.to_string())?;
                            }
                        }
                    }
                }
            }
        } else {
            let err = "No OpenAI API Key configured";
            window
                .emit("ai-response-error", err)
                .map_err(|e| e.to_string())?;
            return Err(err.to_string());
        }
    }

    window
        .emit("ai-response-done", ())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn check_ai_connection() -> Result<String, String> {
    let config = ConfigManager::load_config();
    let client = reqwest::Client::new();

    let start = std::time::Instant::now();

    if config.preferred_model == "local" {
        // Test Ollama connection
        let url = format!(
            "{}/api/tags",
            config
                .local_model_url
                .unwrap_or("http://127.0.0.1:11434".to_string())
        );

        let res = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await;

        match res {
            Ok(response) => {
                if response.status().is_success() {
                    let duration = start.elapsed();
                    Ok(format!("Connected to Ollama ({:?})", duration))
                } else {
                    Err(format!("Ollama returned error: {}", response.status()))
                }
            }
            Err(e) => Err(format!("Failed to connect to Ollama: {}", e)),
        }
    } else {
        // Test OpenAI connection
        if let Some(key) = config.openai_api_key {
            let res = client
                .get("https://api.openai.com/v1/models")
                .header("Authorization", format!("Bearer {}", key))
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;

            match res {
                Ok(response) => {
                    if response.status().is_success() {
                        let duration = start.elapsed();
                        Ok(format!("Connected to OpenAI ({:?})", duration))
                    } else {
                        Err(format!("OpenAI returned error: {}", response.status()))
                    }
                }
                Err(e) => Err(format!("Failed to connect to OpenAI: {}", e)),
            }
        } else {
            Err("No OpenAI API Key configured".to_string())
        }
    }
}

#[tauri::command]
async fn list_ollama_models() -> Result<Vec<String>, String> {
    let config = ConfigManager::load_config();
    let client = reqwest::Client::new();
    let url = format!(
        "{}/api/tags",
        config
            .local_model_url
            .unwrap_or("http://127.0.0.1:11434".to_string())
    );

    let res = client.get(url).send().await.map_err(|e| e.to_string())?;
    let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

    if let Some(models) = body["models"].as_array() {
        let names = models
            .iter()
            .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
            .collect();
        Ok(names)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
async fn list_scripts() -> Result<Vec<String>, String> {
    let mut scripts = Vec::new();
    if let Some(config_dir) = ConfigManager::get_config_dir() {
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
async fn launch_app(exec_cmd: String) -> Result<(), String> {
    let (cmd, args) = parse_exec_command(&exec_cmd).ok_or_else(|| "Empty command".to_string())?;

    std::process::Command::new(cmd)
        .args(args)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn open_entity(path: String) -> Result<(), String> {
    // use open crate or Command
    open::that(path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_apps() -> Result<Vec<AppEntry>, String> {
    let mut apps = Vec::new();

    let mut paths = Vec::new();
    if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in dirs.split(':') {
            paths.push(std::path::PathBuf::from(dir).join("applications"));
        }
    } else {
        // Fallback if XDG_DATA_DIRS is not set
        paths.push(std::path::PathBuf::from("/usr/share/applications"));
        paths.push(std::path::PathBuf::from("/usr/local/share/applications"));
    }

    if let Some(home) = dirs::data_local_dir() {
        paths.push(home.join("applications"));
    }

    for path in paths {
        if !path.exists() {
            continue;
        }

        for entry in WalkDir::new(path)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.extension().map_or(false, |ext| ext == "desktop") {
                if let Ok(content) = std::fs::read_to_string(p) {
                    let mut name = None;
                    let mut exec = None;
                    let mut icon = None;
                    let mut is_hidden = false;

                    for line in content.lines() {
                        if line.starts_with("Name=") && name.is_none() {
                            name = Some(line.trim_start_matches("Name=").to_string());
                        } else if line.starts_with("Exec=") && exec.is_none() {
                            exec = Some(line.trim_start_matches("Exec=").to_string());
                        } else if line.starts_with("Icon=") && icon.is_none() {
                            icon = Some(line.trim_start_matches("Icon=").to_string());
                        } else if line.starts_with("NoDisplay=true") {
                            is_hidden = true;
                        }
                    }

                    if !is_hidden {
                        if let (Some(name), Some(exec)) = (name, exec) {
                            let icon_path = if let Some(icon_name) = icon {
                                resolve_icon(&icon_name)
                            } else {
                                None
                            };
                            apps.push(AppEntry {
                                name,
                                exec,
                                icon: icon_path,
                            });
                        }
                    }
                }
            }
        }
    }

    println!("Found {} apps", apps.len());
    Ok(apps)
}

#[tauri::command]
fn list_windows() -> Result<Vec<window_manager::WindowEntry>, String> {
    let mut windows = WindowManager::list_windows()?;

    // Post-process to resolve icons
    for window in &mut windows {
        if window.icon.is_none() {
            window.icon = resolve_icon(&window.class.to_lowercase());
        }
    }

    Ok(windows)
}

#[tauri::command]
fn focus_window(address: String) -> Result<(), String> {
    WindowManager::focus_window(&address)
}

fn resolve_icon(icon_name: &str) -> Option<String> {
    // 0. Check Cache
    let cache = ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(cached_result) = guard.get(icon_name) {
            return cached_result.clone();
        }
    }

    let path = std::path::Path::new(icon_name);
    if path.is_absolute() && path.exists() {
        let result = Some(icon_name.to_string());
        if let Ok(mut guard) = cache.lock() {
            guard.insert(icon_name.to_string(), result.clone());
        }
        return result;
    }

    let mut search_paths = Vec::new();

    // Standard XDG paths
    if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in dirs.split(':') {
            let p = std::path::Path::new(dir);
            search_paths.push(p.join("pixmaps").to_string_lossy().to_string());
            search_paths.push(p.join("icons").to_string_lossy().to_string());
            // Add common resolutions
            search_paths.push(
                p.join("icons/hicolor/48x48/apps")
                    .to_string_lossy()
                    .to_string(),
            );
            search_paths.push(
                p.join("icons/hicolor/128x128/apps")
                    .to_string_lossy()
                    .to_string(),
            );
            search_paths.push(
                p.join("icons/hicolor/scalable/apps")
                    .to_string_lossy()
                    .to_string(),
            );
        }
    } else {
        search_paths.push("/usr/share/pixmaps".to_string());
        search_paths.push("/usr/share/icons".to_string());
        search_paths.push("/usr/share/icons/hicolor/48x48/apps".to_string());
        search_paths.push("/usr/share/icons/hicolor/128x128/apps".to_string());
        search_paths.push("/usr/share/icons/hicolor/scalable/apps".to_string());
    }

    // User local paths
    if let Some(home) = dirs::data_local_dir() {
        search_paths.push(home.join("icons").to_string_lossy().to_string());
        search_paths.push(
            home.join("icons/hicolor/48x48/apps")
                .to_string_lossy()
                .to_string(),
        );
        search_paths.push(
            home.join("icons/hicolor/128x128/apps")
                .to_string_lossy()
                .to_string(),
        );
        search_paths.push(
            home.join("icons/hicolor/scalable/apps")
                .to_string_lossy()
                .to_string(),
        );
    }

    let extensions = vec!["png", "svg", "xpm"];

    // First, quick check in specific paths (non-recursive) for speed
    for base in &search_paths {
        for ext in &extensions {
            let p = std::path::Path::new(base).join(format!("{}.{}", icon_name, ext));
            if p.exists() {
                let result = Some(p.to_string_lossy().to_string());
                if let Ok(mut guard) = cache.lock() {
                    guard.insert(icon_name.to_string(), result.clone());
                }
                return result;
            }
        }
    }

    println!("Failed to resolve icon: {}", icon_name);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(icon_name.to_string(), None);
    }
    None
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
            // Initialize KSNI Tray Service
            let handle = app.handle().clone();
            let tray = PaletteTray { handle };
            let service = ksni::TrayService::new(tray);
            let _handle = service.handle();

            // Spawn the service (ksni uses blocking run, so usually needs a thread or async task)
            // But ksni::TrayService::spawn returns a handle and runs in background if runtime available?
            // Actually ksni service.spawn() spawns it. simple.
            service.spawn();

            Ok(())
        })
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.emit("open_request", ()); // Optional: emit event to frontend
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

    #[test]
    fn test_parse_exec_command() {
        // Basic command
        let (cmd, args) = parse_exec_command("firefox").unwrap();
        assert_eq!(cmd, "firefox");
        assert!(args.is_empty());

        // Command with args
        let (cmd, args) = parse_exec_command("code .").unwrap();
        assert_eq!(cmd, "code");
        assert_eq!(args, vec!["."]);

        // Placeholders (should be removed)
        let (cmd, args) = parse_exec_command("mpv %f").unwrap();
        assert_eq!(cmd, "mpv");
        assert!(args.is_empty());

        // Complex placeholders
        let (cmd, args) = parse_exec_command("app --url %u --file %f").unwrap();
        assert_eq!(cmd, "app");
        assert_eq!(args, vec!["--url", "--file"]);

        // Empty command
        assert!(parse_exec_command("").is_none());
        assert!(parse_exec_command("   ").is_none());
    }
}
