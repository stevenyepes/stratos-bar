pub mod adapters;
pub mod commands;
pub mod domain;
pub mod ports;
pub mod state;
pub mod tray;
pub mod utils;

use adapters::cached_icon_resolver::CachedIconResolver;
use adapters::fs_app_repository::FsAppRepository;
use adapters::fs_config_service::FsConfigService;
use adapters::http_ai_service::HttpAiService;
use adapters::linux_window_service::LinuxWindowService;
use state::AppState;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tray::PaletteTray;
use utils::toggle_main_window;

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
            toggle_main_window(app);
        }))
        .invoke_handler(tauri::generate_handler![
            commands::system::greet,
            commands::system::search_files,
            commands::system::open_entity,
            commands::system::read_file_preview,
            commands::system::get_file_metadata,
            commands::system::get_selection_context,
            commands::system::copy_to_clipboard,
            commands::system::check_is_executable,
            commands::system::make_file_executable,
            commands::apps::list_apps,
            commands::apps::launch_app,
            commands::config::get_config,
            commands::config::save_config,
            commands::ai::ask_ai,
            commands::ai::check_ai_connection,
            commands::ai::list_ollama_models,
            commands::scripts::list_scripts,
            commands::scripts::execute_script,
            commands::windows::list_windows,
            commands::windows::focus_window,
            commands::system::generate_video_thumbnail,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
