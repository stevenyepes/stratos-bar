pub mod adapters;
pub mod commands;
pub mod diagnose;
pub mod domain;
pub mod ports;
pub mod state;
pub mod tray;
pub mod utils;

use adapters::cached_icon_resolver::{CachedIconResolver, IconAccessError};
use adapters::file_history::FileHistoryAdapter;
use adapters::flag_assets;
use adapters::fs_app_repository::FsAppRepository;
use adapters::fs_config_service::FsConfigService;
use adapters::google_translation_service::GoogleTranslationService;
use adapters::http_ai_service::HttpAiService;
use adapters::linux_window_service::LinuxWindowService;
use adapters::preview_grants::{read_preview_range, PreviewAccessError, PreviewGrants};
use adapters::process_environment::ProcessEnvironment;
use adapters::systemd_launcher::SystemdScopeLauncher;
use ports::app_port::AppRepository;
use ports::history::HistoryRepository;
use state::AppState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{http, Emitter, Manager};
use tray::PaletteTray;
use utils::toggle_main_window;

/// Decodes the percent-encoded absolute path segment produced by the frontend's
/// `convertFileSrc(path, 'stratos-icon')` call. Hand-rolled rather than pulling in
/// `percent-encoding` as a new direct dependency for this one call site.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    out.push(value);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Extension-based `Content-Type` guess for icon bytes. Deliberately not the
/// `mime_guess` crate (not already a dependency) -- this only needs to cover the
/// handful of formats `freedesktop-icons`/desktop entries actually use.
fn guess_icon_content_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("xpm") => "image/x-xpixmap",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/vnd.microsoft.icon",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

/// `Content-Type` for a previewed file. Extends `guess_icon_content_type` with the
/// video formats `useFilePreview.js` recognises; icons and preview images overlap
/// entirely, so the image cases are not restated here.
fn guess_preview_content_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4") => "video/mp4",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("tiff") | Some("tif") => "image/tiff",
        Some("pdf") => "application/pdf",
        _ => guess_icon_content_type(path),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(env_snapshot: HashMap<String, String>) {
    // Constructed above the builder (rather than inside `setup()`, as before) so the
    // same instance can be shared with the `stratos-icon` protocol handler below,
    // which is registered before `setup()` runs and has no `app.handle()` yet.
    let icon_resolver = Arc::new(CachedIconResolver::new());
    let icon_resolver_for_protocol = icon_resolver.clone();

    // Shared the same way, and for the same reason: the `asset` handler is
    // registered before `setup()` runs, so it cannot reach `AppState`.
    let preview_grants = Arc::new(PreviewGrants::new());
    let preview_grants_for_protocol = preview_grants.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .register_uri_scheme_protocol("stratos-icon", move |_ctx, request| {
            let raw_path = request.uri().path();
            let encoded = raw_path.strip_prefix('/').unwrap_or(raw_path);
            let decoded_path = percent_decode(encoded);

            // Vendored currency flags ship inside the binary, so they answer before the
            // disk-backed resolver ever sees the path; every other request falls through
            // to the icon-theme lookup below unchanged.
            if let Some(code) = flag_assets::flag_code_from_path(&decoded_path) {
                let mut response = http::Response::new(flag_assets::lookup(code).to_vec());
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static("image/svg+xml"),
                );
                return response;
            }

            match icon_resolver_for_protocol.serve_icon_bytes(&decoded_path) {
                Ok(bytes) => {
                    let mut response = http::Response::new(bytes.as_ref().clone());
                    response.headers_mut().insert(
                        http::header::CONTENT_TYPE,
                        http::HeaderValue::from_static(guess_icon_content_type(&decoded_path)),
                    );
                    response
                }
                Err(err) => {
                    eprintln!(
                        "[stratos-bar] stratos-icon request for {decoded_path} failed: {err:?}"
                    );
                    let mut response = http::Response::new(Vec::new());
                    *response.status_mut() = match err {
                        IconAccessError::NotFound => http::StatusCode::NOT_FOUND,
                        IconAccessError::OutsideAllowedRoots => http::StatusCode::FORBIDDEN,
                        IconAccessError::ReadFailed(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    response
                }
            }
        })
        // Tauri only registers its own built-in `asset` handler when the app has
        // not claimed the scheme itself (manager/webview.rs:307-318), and that
        // built-in is gated behind the `protocol-asset` Cargo feature -- which in
        // turn requires an `assetProtocol` config block, the whole-home read grant
        // this feature removed. Serving the scheme ourselves keeps file preview
        // working with no config-declared scope at all: authority comes from
        // `PreviewGrants`, one path at a time.
        .register_uri_scheme_protocol("asset", move |_ctx, request| {
            let raw_path = request.uri().path();
            let encoded = raw_path.strip_prefix('/').unwrap_or(raw_path);
            let decoded_path = percent_decode(encoded);

            let range_header = request
                .headers()
                .get(http::header::RANGE)
                .and_then(|value| value.to_str().ok());

            match read_preview_range(&preview_grants_for_protocol, &decoded_path, range_header) {
                Ok(served) => {
                    let mut response = http::Response::new(served.bytes);
                    let headers = response.headers_mut();
                    headers.insert(
                        http::header::CONTENT_TYPE,
                        http::HeaderValue::from_static(guess_preview_content_type(&decoded_path)),
                    );
                    // Without this a media element will not seek, and WebKit falls
                    // back to downloading the whole file before it plays anything.
                    headers.insert(
                        http::header::ACCEPT_RANGES,
                        http::HeaderValue::from_static("bytes"),
                    );
                    if let Some((start, end)) = served.range {
                        let content_range = format!("bytes {start}-{end}/{}", served.total_len);
                        if let Ok(value) = http::HeaderValue::from_str(&content_range) {
                            headers.insert(http::header::CONTENT_RANGE, value);
                        }
                        *response.status_mut() = http::StatusCode::PARTIAL_CONTENT;
                    }
                    response
                }
                Err(err) => {
                    eprintln!("[stratos-bar] asset request for {decoded_path} failed: {err:?}");
                    let mut response = http::Response::new(Vec::new());
                    if let PreviewAccessError::RangeNotSatisfiable(total_len) = &err {
                        if let Ok(value) =
                            http::HeaderValue::from_str(&format!("bytes */{total_len}"))
                        {
                            response
                                .headers_mut()
                                .insert(http::header::CONTENT_RANGE, value);
                        }
                    }
                    *response.status_mut() = match err {
                        PreviewAccessError::NotGranted => http::StatusCode::FORBIDDEN,
                        PreviewAccessError::NotFound => http::StatusCode::NOT_FOUND,
                        PreviewAccessError::RangeNotSatisfiable(_) => {
                            http::StatusCode::RANGE_NOT_SATISFIABLE
                        }
                        PreviewAccessError::ReadFailed(_) => {
                            http::StatusCode::INTERNAL_SERVER_ERROR
                        }
                    };
                    response
                }
            }
        })
        .setup(move |app| {
            // Instantiate Adapters
            let config_service = Arc::new(FsConfigService::new());
            // FsAppRepository needs icon resolver
            let app_repository = Arc::new(FsAppRepository::new_with_handle(
                icon_resolver.clone(),
                app.handle().clone(),
            ));
            let command_executor = Arc::new(adapters::linux_window_service::StdCommandExecutor);
            let window_service = Arc::new(LinuxWindowService::new(command_executor));
            let ai_service = Arc::new(HttpAiService::new());
            let app_launcher = Arc::new(SystemdScopeLauncher::new());

            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let history_repository = Arc::new(FileHistoryAdapter::new(app_data_dir));
            let translation_service = Arc::new(GoogleTranslationService::new(None));
            let env_port = Arc::new(ProcessEnvironment::new(env_snapshot));

            // Manage State
            app.manage(AppState {
                app_repository: app_repository.clone(),
                app_launcher,
                window_service,
                config_service: config_service.clone(),
                icon_resolver: icon_resolver.clone(),
                ai_service,
                history_repository: history_repository.clone(),
                translation_service,
                env_port,
                preview_grants,
            });

            // Rekey history entries from exec-derived ids to desktop-file ids.
            // Fire-and-forget: must not block setup() or delay start_background_tasks().
            let history_repository_for_rekey = history_repository;
            let app_repository_for_rekey = app_repository.clone();
            tauri::async_runtime::spawn(async move {
                let list_apps_result = tauri::async_runtime::spawn_blocking(move || {
                    app_repository_for_rekey.list_apps()
                })
                .await;
                let mapping: std::collections::HashMap<String, String> = match list_apps_result {
                    Ok(Ok(apps)) => apps.into_iter().map(|app| (app.exec, app.id)).collect(),
                    Ok(Err(e)) => {
                        eprintln!("[stratos-bar] failed to list apps for history rekey: {e}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("[stratos-bar] list apps task for history rekey panicked: {e}");
                        return;
                    }
                };
                if let Err(e) = history_repository_for_rekey.rekey_app_ids(&mapping).await {
                    eprintln!("[stratos-bar] failed to rekey history app ids: {e}");
                }
            });

            // Background scan + filesystem watcher must start *after* manage(),
            // because both look the AppState up through the AppHandle.
            app_repository.start_background_tasks();

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
            commands::apps::rescan_apps,
            commands::apps::set_custom_app_dirs,
            commands::apps::set_icon_scale,
            commands::apps::launch_app,
            commands::apps::run_command,
            commands::config::get_config,
            commands::config::save_config,
            commands::ai::ask_ai,
            commands::ai::check_ai_connection,
            commands::ai::list_ollama_models,
            commands::scripts::list_scripts,
            commands::scripts::execute_script,
            commands::windows::list_windows,
            commands::windows::focus_window,
            commands::windows::focus_window,
            commands::windows::resize_window,
            commands::system::generate_video_thumbnail,
            commands::history::get_recent_actions,
            commands::history::record_action,
            commands::history::clear_history,
            commands::translation::translate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
