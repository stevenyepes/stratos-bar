use std::io::Write;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};
use walkdir::WalkDir;

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
async fn ask_ai(messages: Vec<Message>) -> Result<String, String> {
    let config = ConfigManager::load_config();
    let client = reqwest::Client::new();

    if config.preferred_model == "local" {
        // Ollama - use /api/chat
        let url = format!(
            "{}/api/chat",
            config
                .local_model_url
                .unwrap_or("http://localhost:11434".to_string())
        );

        let res = client
            .post(url)
            .json(&json!({
                "model": config.ollama_model.unwrap_or("llama3".to_string()),
                "messages": messages,
                "stream": false
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        // Ollama /api/chat returns message.content
        Ok(body["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    } else {
        // Cloud (OpenAI)
        if let Some(key) = config.openai_api_key {
            let res = client
                .post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", key))
                .json(&json!({
                    "model": "gpt-4o",
                    "messages": messages
                }))
                .send()
                .await
                .map_err(|e| e.to_string())?;

            let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            // OpenAI returns choices[0].message.content
            Ok(body["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string())
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
            .unwrap_or("http://localhost:11434".to_string())
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
    // Basic Exec parsing: remove %f, %F, %u, %U placeholders
    // Split by space, handle quotes
    let cleaned = exec_cmd
        .replace("%f", "")
        .replace("%F", "")
        .replace("%u", "")
        .replace("%U", "")
        .replace("%i", "")
        .replace("%c", "")
        .replace("%k", "");

    let parts: Vec<&str> = cleaned.trim().split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".to_string());
    }

    let cmd = parts[0];
    let args = &parts[1..];

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
    let mut paths = vec![
        std::path::PathBuf::from("/usr/share/applications"),
        std::path::PathBuf::from("/usr/local/share/applications"),
    ];

    if let Some(home) = dirs::data_local_dir() {
        paths.push(home.join("applications"));
    }

    for path in paths {
        if !path.exists() {
            println!("App path does not exist: {:?}", path);
            continue;
        }
        println!("Scanning app path: {:?}", path);
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
                    } else {
                        // println!("Skipping hidden app: {:?}", p);
                    }
                }
            }
        }
    }
    println!("Found {} apps", apps.len());
    Ok(apps)
}

fn resolve_icon(icon_name: &str) -> Option<String> {
    let path = std::path::Path::new(icon_name);
    if path.is_absolute() && path.exists() {
        return Some(icon_name.to_string());
    }

    let mut search_paths = vec![
        "/usr/share/pixmaps".to_string(),
        "/usr/share/icons".to_string(),
    ];

    if let Some(home) = dirs::data_local_dir() {
        search_paths.push(home.join("icons").to_string_lossy().to_string());
    }

    let extensions = vec!["png", "svg", "xpm"];

    // First, quick check in specific paths (non-recursive) for speed
    for base in &search_paths {
        for ext in &extensions {
            let p = std::path::Path::new(base).join(format!("{}.{}", icon_name, ext));
            if p.exists() {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }

    // Deep search (more expensive but finds themed icons)
    // Limit depth to avoid scanning too much
    for base in &search_paths {
        for entry in WalkDir::new(base)
            .max_depth(5)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() {
                if let Some(stem) = p.file_stem() {
                    if stem == icon_name {
                        if let Some(ext) = p.extension() {
                            let ext_str = ext.to_string_lossy();
                            if extensions.contains(&ext_str.as_ref()) {
                                println!("Resolved icon {} to {:?}", icon_name, p);
                                return Some(p.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    println!("Failed to resolve icon: {}", icon_name);
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
    // Helper to run a command and get output
    fn run_output(cmd: &str, args: &[&str]) -> Option<String> {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout).ok()
                } else {
                    None
                }
            })
    }

    // 1. Try Wayland Primary
    if let Some(text) = run_output("wl-paste", &["--primary"]) {
        if !text.trim().is_empty() {
            return Ok(text);
        }
    }

    // 2. Try X11 Primary
    if let Some(text) = run_output("xclip", &["-o", "-selection", "primary"]) {
        if !text.trim().is_empty() {
            return Ok(text);
        }
    }

    // 3. Try Wayland Clipboard
    if let Some(text) = run_output("wl-paste", &[]) {
        if !text.trim().is_empty() {
            return Ok(text);
        }
    }

    // 4. Try X11 Clipboard
    if let Some(text) = run_output("xclip", &["-o", "-selection", "clipboard"]) {
        if !text.trim().is_empty() {
            return Ok(text);
        }
    }

    Err("No text found in selection or clipboard".to_string())
}

#[tauri::command]
async fn copy_to_clipboard(text: String) -> Result<(), String> {
    println!("DEBUG: copy_to_clipboard called with text: '{}'", text);

    // Helper to pipe input to command
    fn run_input(cmd: &str, args: &[&str], input: &str) -> Result<(), String> {
        let mut child = std::process::Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .map_err(|e| e.to_string())?;
        }

        let status = child.wait().map_err(|e| e.to_string())?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Command {} failed with status {:?}", cmd, status))
        }
    }

    // 1. Try wl-copy (Wayland)
    if run_input("wl-copy", &["--type", "text/plain"], &text).is_ok() {
        return Ok(());
    }

    // 2. Try xclip (X11)
    // -selection clipboard
    if run_input("xclip", &["-selection", "clipboard"], &text).is_ok() {
        return Ok(());
    }

    // 3. Fallback to arboard via a new instance? Or just error.
    Err("Failed to copy to clipboard (wl-copy and xclip failed)".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
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
            copy_to_clipboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
