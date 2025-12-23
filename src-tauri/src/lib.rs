use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};
use walkdir::WalkDir;

static ICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

// Public for testing, or just use within module

// Helper to clean exec command
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

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq, Hash)]
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
    use freedesktop_desktop_entry::Iter;
    use std::collections::HashSet;
    use std::path::PathBuf;

    let mut apps = Vec::new();
    let mut seen_ids = HashSet::new();

    // Construct search paths
    let mut search_paths = Vec::new();
    if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in dirs.split(':') {
            search_paths.push(PathBuf::from(dir).join("applications"));
        }
    } else {
        search_paths.push(PathBuf::from("/usr/share/applications"));
        search_paths.push(PathBuf::from("/usr/local/share/applications"));
    }
    if let Some(home) = dirs::data_local_dir() {
        search_paths.push(home.join("applications"));
    }

    // Helper to parse desktop file manually
    fn parse_desktop_file(path: &std::path::Path) -> Option<AppEntry> {
        let content = std::fs::read_to_string(path).ok()?;
        let mut in_desktop_entry = false;
        let mut name = None;
        let mut exec = None;
        let mut icon = None;
        let mut no_display = false;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                in_desktop_entry = line == "[Desktop Entry]";
                continue;
            }

            if in_desktop_entry {
                if let Some(val) = line.strip_prefix("Name=") {
                    if name.is_none() {
                        name = Some(val.to_string());
                    }
                } else if let Some(val) = line.strip_prefix("Exec=") {
                    if exec.is_none() {
                        // Cleanup exec command (remove field codes)
                        let clean_exec = val
                            .replace("%f", "")
                            .replace("%F", "")
                            .replace("%u", "")
                            .replace("%U", "")
                            .replace("%i", "")
                            .replace("%c", "")
                            .replace("%k", "")
                            .trim()
                            .to_string();
                        exec = Some(clean_exec);
                    }
                } else if let Some(val) = line.strip_prefix("Icon=") {
                    if icon.is_none() {
                        icon = Some(val.to_string());
                    }
                } else if let Some(val) = line.strip_prefix("NoDisplay=") {
                    if val.to_lowercase() == "true" {
                        no_display = true;
                    }
                }
            }
        }

        if !no_display {
            if let (Some(name), Some(exec)) = (name, exec) {
                // Check if icon resolves
                let icon_path = icon.and_then(|i| resolve_icon(&i));
                return Some(AppEntry {
                    name,
                    exec,
                    icon: icon_path.filter(|i| !i.is_empty()),
                });
            }
        }
        None
    }

    // Iterate over paths using the crate's iterator
    for path in Iter::new(search_paths.clone().into_iter()) {
        if let Some(app) = parse_desktop_file(&path) {
            let id = app.name.clone();
            if !seen_ids.contains(&id) {
                apps.push(app);
                seen_ids.insert(id);
            }
        }
    }

    // 2. Scan Flatpak Exports explicitly if available
    let flatpak_paths = vec![
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        dirs::data_local_dir()
            .map(|d| d.join("flatpak/exports/share/applications"))
            .unwrap_or_default(),
    ];

    // Scan flatpaks using same Iter
    for path in Iter::new(flatpak_paths.into_iter()) {
        if let Some(app) = parse_desktop_file(&path) {
            let id = app.name.clone();
            if !seen_ids.contains(&id) {
                apps.push(app);
                seen_ids.insert(id);
            }
        }
    }

    // 3. Scan AppImages
    if let Some(home) = dirs::home_dir() {
        let applications_dir = home.join("Applications");
        if applications_dir.exists() {
            let glob_pattern = applications_dir.join("*.AppImage");
            if let Ok(glob_paths) = glob::glob(&glob_pattern.to_string_lossy()) {
                for entry in glob_paths.filter_map(|e| e.ok()) {
                    let name = entry
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if name.is_empty() {
                        continue;
                    }
                    if seen_ids.contains(&name) {
                        continue;
                    }

                    let icon =
                        resolve_icon(&name).or_else(|| resolve_icon("application-x-executable"));

                    apps.push(AppEntry {
                        name: name.clone(),
                        exec: entry.to_string_lossy().to_string(),
                        icon,
                    });
                    seen_ids.insert(name);
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

    // 1. Direct path check
    let path = std::path::Path::new(icon_name);
    if path.is_absolute() && path.exists() {
        // Canonicalize to resolve symlinks
        let resolved_path = match std::fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => icon_name.to_string(), // Fallback to original if canonicalize fails
        };
        let result = Some(resolved_path);
        if let Ok(mut guard) = cache.lock() {
            guard.insert(icon_name.to_string(), result.clone());
        }
        return result;
    }

    // 2. Use linicon
    use linicon::lookup_icon;

    // Iterate over all results instead of just first, prioritize scalable/png
    // Actually linicon should return best match first.
    if let Some(icon_path) = lookup_icon(icon_name).next() {
        if let Ok(path_str) = icon_path {
            // Canonicalize to resolve symlinks
            let resolved_path = match std::fs::canonicalize(&path_str.path) {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => path_str.path.to_string_lossy().to_string(),
            };
            let result = Some(resolved_path);
            if let Ok(mut guard) = cache.lock() {
                guard.insert(icon_name.to_string(), result.clone());
            }
            return result;
        }
    }

    // 3. Fallback: Manual search in standard paths
    // Linicon might fail if theme config is weird or specific sizes not found?
    // Let's do a robust manual search for common cases (steam, hicolor default)
    if icon_name.contains("steam") || icon_name.contains("org.gnome") {
        println!("DEBUG: Manual fallback for {}", icon_name);
    }

    let mut search_paths = Vec::new();

    // Standard XDG paths
    if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in dirs.split(':') {
            let p = std::path::Path::new(dir);
            // Check hicolor fallback which is almost always used by Steam/Apps
            search_paths.push(p.join("icons/hicolor/48x48/apps"));
            search_paths.push(p.join("icons/hicolor/32x32/apps"));
            search_paths.push(p.join("icons/hicolor/128x128/apps"));
            search_paths.push(p.join("icons/hicolor/scalable/apps"));
            search_paths.push(p.join("pixmaps"));
            search_paths.push(p.join("icons"));
        }
    } else {
        search_paths.push(std::path::PathBuf::from(
            "/usr/share/icons/hicolor/48x48/apps",
        ));
        search_paths.push(std::path::PathBuf::from(
            "/usr/share/icons/hicolor/32x32/apps",
        ));
        search_paths.push(std::path::PathBuf::from(
            "/usr/share/icons/hicolor/scalable/apps",
        ));
        search_paths.push(std::path::PathBuf::from("/usr/share/pixmaps"));
        search_paths.push(std::path::PathBuf::from("/usr/share/icons"));
    }

    // User local paths
    if let Some(home) = dirs::data_local_dir() {
        search_paths.push(home.join("icons/hicolor/48x48/apps"));
        search_paths.push(home.join("icons/hicolor/32x32/apps"));
        search_paths.push(home.join("icons/hicolor/128x128/apps"));
        search_paths.push(home.join("icons/hicolor/scalable/apps"));
        search_paths.push(home.join("icons"));
    }

    // Steam specific paths
    if let Some(home) = dirs::home_dir() {
        search_paths.push(home.join(".steam/root/appcache/librarycache"));
        search_paths.push(home.join(".local/share/icons/hicolor/48x48/apps"));
    }

    let extensions = vec!["png", "svg", "xpm", "ico", "jpg"];

    for base in &search_paths {
        if !base.exists() {
            continue;
        }

        // Steam specific cache check (icon_name usually steam_icon_APPID)
        // librarycache has files like APPID_icon.jpg or just matches the name
        // The icons in librarycache seem to be mostly jpg or png, named by appid.
        // But our icon_name from desktop file is usually "steam_icon_APPID".
        if base.ends_with("librarycache") && icon_name.starts_with("steam_icon_") {
            let app_id = icon_name.trim_start_matches("steam_icon_");
            // try app_id_icon.jpg
            let p = base.join(format!("{}_icon.jpg", app_id));
            if p.exists() {
                let result = Some(p.to_string_lossy().to_string());
                if let Ok(mut guard) = cache.lock() {
                    guard.insert(icon_name.to_string(), result.clone());
                }
                return result;
            }
        }

        for ext in &extensions {
            let p = base.join(format!("{}.{}", icon_name, ext));
            if p.exists() {
                if icon_name.contains("steam") || icon_name.contains("org.gnome") {
                    println!("DEBUG: Found at {:?}", p);
                }
                // Canonicalize to resolve symlinks
                let resolved_path = match std::fs::canonicalize(&p) {
                    Ok(canonical) => canonical.to_string_lossy().to_string(),
                    Err(_) => p.to_string_lossy().to_string(),
                };
                let result = Some(resolved_path);
                if let Ok(mut guard) = cache.lock() {
                    guard.insert(icon_name.to_string(), result.clone());
                }
                return result;
            }
        }
    }

    if icon_name.contains("steam") {
        println!("DEBUG: Failed to find {} in {:?}", icon_name, search_paths);
    }

    // Try stripping extension if present in name but not a path
    // e.g. "foo.png" -> "foo"
    if let Some(stem) = std::path::Path::new(icon_name).file_stem() {
        if stem != icon_name {
            let stem_str = stem.to_string_lossy();
            // Recurse once
            // Avoid infinite recursion by checking strictly different
            // Actually just call manual lookup again or linicon again?
            // Calling linicon again is safe.
            if let Some(icon_path) = lookup_icon(&stem_str).next() {
                if let Ok(path_str) = icon_path {
                    let result = Some(path_str.path.to_string_lossy().to_string());
                    if let Ok(mut guard) = cache.lock() {
                        guard.insert(icon_name.to_string(), result.clone());
                    }
                    return result;
                }
            }
        }
    }

    // Fallback? Linicon is pretty good.
    // If it is a generic name like "firefox", linicon handles it.
    // If it fails, maybe return None.

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

            // DEBUG: Force list_apps to run
            tauri::async_runtime::spawn(async {
                println!("DEBUG: Triggering list_apps manually");
                match list_apps().await {
                    Ok(apps) => println!("DEBUG: list_apps finished, found {} apps", apps.len()),
                    Err(e) => println!("DEBUG: list_apps failed: {}", e),
                }
            });

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
