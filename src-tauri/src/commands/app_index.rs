use crate::domain::app_index::{AliasEntry, AppIndexStatus, LaunchError};
use crate::domain::apps::AppSource;
use crate::domain::config::QuickActions;
use crate::ports::app_port::AppRepository;
use crate::ports::config_port::ConfigService;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

pub const ALIASES_UPDATED_EVENT: &str = "aliases-updated";
pub const APP_INDEX_STATUS_UPDATED_EVENT: &str = "app-index-status-updated";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AliasesUpdatedPayload {
    pub app_id: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppIndexStatusUpdatedPayload {
    pub status: AppIndexStatus,
}

pub trait AppEventSink: Send + Sync {
    fn emit_json(&self, event: &str, payload: serde_json::Value);
}

impl AppEventSink for AppHandle {
    fn emit_json(&self, event: &str, payload: serde_json::Value) {
        let _ = Emitter::emit(self, event, payload);
    }
}

#[derive(Debug, Clone, Default)]
pub struct RecordingEventSink {
    pub events: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl RecordingEventSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn recorded(&self) -> Vec<(String, serde_json::Value)> {
        self.events.lock().unwrap().clone()
    }
}

impl AppEventSink for RecordingEventSink {
    fn emit_json(&self, event: &str, payload: serde_json::Value) {
        self.events
            .lock()
            .unwrap()
            .push((event.to_string(), payload));
    }
}

pub fn parse_source_label(label: &str) -> Result<AppSource, String> {
    match label {
        "desktop" => Ok(AppSource::Desktop),
        "flatpak" => Ok(AppSource::Flatpak),
        "snap" => Ok(AppSource::Snap),
        "appimage" => Ok(AppSource::AppImage),
        "nix" => Ok(AppSource::Nix),
        "other" => Ok(AppSource::Other),
        other => Err(format!("unknown app source: {other}")),
    }
}

pub fn source_to_label(source: AppSource) -> &'static str {
    match source {
        AppSource::Desktop => "desktop",
        AppSource::Flatpak => "flatpak",
        AppSource::Snap => "snap",
        AppSource::AppImage => "appimage",
        AppSource::Nix => "nix",
        AppSource::Other => "other",
    }
}

pub fn validate_aliases(aliases: &[String]) -> Result<(), String> {
    for alias in aliases {
        if !QuickActions::is_valid_alias(alias) {
            return Err(format!("invalid alias: {alias:?}"));
        }
    }
    Ok(())
}

pub fn parse_disabled_sources(sources: &[String]) -> Result<Vec<AppSource>, String> {
    sources.iter().map(|s| parse_source_label(s)).collect()
}

pub fn disabled_sources_from_config(service: &dyn ConfigService) -> Vec<AppSource> {
    let config = service.load_config();
    config
        .disabled_sources
        .iter()
        .filter_map(|s| parse_source_label(s).ok())
        .collect()
}

pub fn set_disabled_sources_would_empty_index(
    sources: &[String],
    custom_app_dirs_empty: bool,
) -> bool {
    if !custom_app_dirs_empty {
        return false;
    }
    let known_labels = ["desktop", "flatpak", "snap", "appimage", "nix", "other"];
    known_labels
        .iter()
        .all(|label| sources.iter().any(|s| s == label))
}

fn build_app_index_status(
    repo: &dyn AppRepository,
    disabled_sources: &[AppSource],
) -> AppIndexStatus {
    let apps = repo.list_apps().unwrap_or_default();
    let total = apps.len();
    let mut counts: HashMap<AppSource, usize> = HashMap::new();
    for app in &apps {
        *counts.entry(app.source).or_insert(0) += 1;
    }
    let mut sources_with_counts: Vec<(AppSource, usize)> = counts.into_iter().collect();
    sources_with_counts.sort_by(|a, b| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)));

    let indexed_at = apps.iter().filter_map(|a| a.indexed_at).max();

    AppIndexStatus {
        total_apps: total,
        indexed_at,
        truncated: false,
        disabled_sources: disabled_sources.to_vec(),
        sources_with_counts,
    }
}

pub fn set_app_aliases_with_sink(
    service: &dyn ConfigService,
    sink: &dyn AppEventSink,
    app_id: String,
    aliases: Vec<String>,
) -> Result<AliasesUpdatedPayload, String> {
    let payload = set_app_aliases_logic(service, app_id, aliases)?;
    let value = serde_json::to_value(&payload).map_err(|e| e.to_string())?;
    sink.emit_json(ALIASES_UPDATED_EVENT, value);
    Ok(payload)
}

pub fn bulk_set_aliases_with_sink(
    service: &dyn ConfigService,
    sink: &dyn AppEventSink,
    entries: Vec<AliasEntry>,
) -> Result<Vec<AliasesUpdatedPayload>, String> {
    let payloads = bulk_set_aliases_logic(service, entries)?;
    for payload in &payloads {
        let value = serde_json::to_value(payload).map_err(|e| e.to_string())?;
        sink.emit_json(ALIASES_UPDATED_EVENT, value);
    }
    Ok(payloads)
}

pub fn set_disabled_sources_with_sink(
    service: &dyn ConfigService,
    repo: &dyn AppRepository,
    sink: &dyn AppEventSink,
    sources: Vec<String>,
) -> Result<AppIndexStatusUpdatedPayload, String> {
    let payload = set_disabled_sources_logic(service, repo, sources)?;
    let value = serde_json::to_value(&payload).map_err(|e| e.to_string())?;
    sink.emit_json(APP_INDEX_STATUS_UPDATED_EVENT, value);
    Ok(payload)
}

pub fn set_app_aliases_logic(
    service: &dyn ConfigService,
    app_id: String,
    aliases: Vec<String>,
) -> Result<AliasesUpdatedPayload, String> {
    validate_aliases(&aliases)?;

    let mut config = service.load_config();
    config
        .quick_actions
        .set_app_aliases(&app_id, aliases.clone())
        .map_err(|e| e.to_string())?;
    service.save_config(&config)?;

    Ok(AliasesUpdatedPayload { app_id, aliases })
}

pub fn bulk_set_aliases_logic(
    service: &dyn ConfigService,
    entries: Vec<AliasEntry>,
) -> Result<Vec<AliasesUpdatedPayload>, String> {
    for entry in &entries {
        validate_aliases(&entry.aliases)?;
    }

    let mut config = service.load_config();
    for entry in &entries {
        config
            .quick_actions
            .set_app_aliases(&entry.app_id, entry.aliases.clone())
            .map_err(|e| e.to_string())?;
    }
    service.save_config(&config)?;

    Ok(entries
        .into_iter()
        .map(|e| AliasesUpdatedPayload {
            app_id: e.app_id,
            aliases: e.aliases,
        })
        .collect())
}

pub fn set_disabled_sources_logic(
    service: &dyn ConfigService,
    repo: &dyn AppRepository,
    sources: Vec<String>,
) -> Result<AppIndexStatusUpdatedPayload, String> {
    let parsed = parse_disabled_sources(&sources)?;

    let config = service.load_config();
    if set_disabled_sources_would_empty_index(&sources, config.custom_app_dirs.is_empty()) {
        return Err(LaunchError::UnvalidatedExec.to_string());
    }

    let mut updated = config;
    updated.disabled_sources = sources;
    service.save_config(&updated)?;

    let status = build_app_index_status(repo, &parsed);
    Ok(AppIndexStatusUpdatedPayload { status })
}

pub fn get_app_index_status_logic(
    repo: &dyn AppRepository,
    service: &dyn ConfigService,
) -> Result<AppIndexStatus, String> {
    let disabled = disabled_sources_from_config(service);
    Ok(build_app_index_status(repo, &disabled))
}

#[tauri::command]
pub async fn set_app_aliases(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    app_id: String,
    aliases: Vec<String>,
) -> Result<(), String> {
    let _ = set_app_aliases_with_sink(
        &*state.config_service,
        &app_handle,
        app_id,
        aliases,
    )?;
    Ok(())
}

#[tauri::command]
pub async fn bulk_set_aliases(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    entries: Vec<AliasEntry>,
) -> Result<(), String> {
    let _ = bulk_set_aliases_with_sink(
        &*state.config_service,
        &app_handle,
        entries,
    )?;
    Ok(())
}

#[tauri::command]
pub async fn set_disabled_sources(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    sources: Vec<String>,
) -> Result<AppIndexStatus, String> {
    let payload = set_disabled_sources_with_sink(
        &*state.config_service,
        &*state.app_repository,
        &app_handle,
        sources,
    )?;
    Ok(payload.status)
}

#[tauri::command]
pub async fn get_app_index_status(
    state: State<'_, AppState>,
) -> Result<AppIndexStatus, String> {
    get_app_index_status_logic(&*state.app_repository, &*state.config_service)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::apps::{AppEntry, AppSource};
    use crate::ports::app_port::MockAppRepository;
    use crate::ports::config_port::MockConfigService;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    fn fixture(name: &str, source: AppSource) -> AppEntry {
        AppEntry {
            id: name.to_lowercase(),
            name: name.to_string(),
            generic_name: None,
            description: None,
            keywords: Vec::new(),
            exec: name.to_lowercase(),
            try_exec: None,
            icon: None,
            categories: Vec::new(),
            startup_wm_class: None,
            source,
            path: format!("/tmp/{name}.desktop"),
            ..Default::default()
        }
    }

    #[test]
    fn parse_source_label_round_trips_known_labels() {
        assert_eq!(parse_source_label("desktop").unwrap(), AppSource::Desktop);
        assert_eq!(parse_source_label("flatpak").unwrap(), AppSource::Flatpak);
        assert_eq!(parse_source_label("snap").unwrap(), AppSource::Snap);
        assert_eq!(parse_source_label("appimage").unwrap(), AppSource::AppImage);
        assert_eq!(parse_source_label("nix").unwrap(), AppSource::Nix);
        assert_eq!(parse_source_label("other").unwrap(), AppSource::Other);

        assert_eq!(source_to_label(AppSource::Desktop), "desktop");
        assert_eq!(source_to_label(AppSource::Flatpak), "flatpak");
        assert_eq!(source_to_label(AppSource::Snap), "snap");
        assert_eq!(source_to_label(AppSource::AppImage), "appimage");
        assert_eq!(source_to_label(AppSource::Nix), "nix");
        assert_eq!(source_to_label(AppSource::Other), "other");
    }

    #[test]
    fn parse_source_label_rejects_unknown_label() {
        let err = parse_source_label("garbage").unwrap_err();
        assert!(err.contains("garbage"));
    }

    #[test]
    fn validate_aliases_accepts_well_formed_and_rejects_others() {
        assert!(validate_aliases(&["browser".to_string(), "g cal".to_string()]).is_ok());
        assert!(validate_aliases(&["rm -rf /".to_string()]).is_err());
        assert!(validate_aliases(&["".to_string()]).is_err());
    }

    #[test]
    fn set_app_aliases_logic_round_trips_through_config_service() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> =
            Arc::new(Mutex::new(crate::domain::config::AppConfig::default()));
        let load_store = store.clone();
        let save_store = store.clone();

        let mut mock = MockConfigService::new();
        mock.expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        mock.expect_save_config()
            .returning(move |cfg: &crate::domain::config::AppConfig| {
                *save_store.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let payload = set_app_aliases_logic(
            &mock,
            "google-chrome".to_string(),
            vec!["browser".to_string(), "browse".to_string()],
        )
        .expect("expected ok");

        assert_eq!(payload.app_id, "google-chrome");
        assert_eq!(
            payload.aliases,
            vec!["browser".to_string(), "browse".to_string()]
        );

        let stored = store.lock().unwrap().clone();
        assert_eq!(
            stored.quick_actions.app_aliases.get("google-chrome"),
            Some(&vec!["browser".to_string(), "browse".to_string()])
        );
    }

    #[test]
    fn set_app_aliases_with_sink_emits_aliases_updated_event() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> =
            Arc::new(Mutex::new(crate::domain::config::AppConfig::default()));
        let load_store = store.clone();
        let save_store = store.clone();

        let mut mock = MockConfigService::new();
        mock.expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        mock.expect_save_config()
            .returning(move |cfg: &crate::domain::config::AppConfig| {
                *save_store.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let sink = RecordingEventSink::new();
        let payload = set_app_aliases_with_sink(
            &mock,
            &sink,
            "google-chrome".to_string(),
            vec!["browser".to_string()],
        )
        .expect("expected ok");

        assert_eq!(payload.app_id, "google-chrome");

        let recorded = sink.recorded();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, ALIASES_UPDATED_EVENT);
        assert_eq!(recorded[0].1["app_id"], "google-chrome");
        assert_eq!(recorded[0].1["aliases"][0], "browser");
    }

    #[test]
    fn set_app_aliases_logic_rejects_invalid_alias_without_persisting() {
        let mut mock = MockConfigService::new();
        mock.expect_load_config().times(0);
        mock.expect_save_config().times(0);

        let result = set_app_aliases_logic(
            &mock,
            "google-chrome".to_string(),
            vec!["rm -rf /".to_string()],
        );
        assert!(result.is_err());
    }

    #[test]
    fn set_app_aliases_logic_propagates_save_error() {
        let mut mock = MockConfigService::new();
        mock.expect_load_config()
            .returning(crate::domain::config::AppConfig::default);
        mock.expect_save_config()
            .returning(|_| Err("disk full".to_string()));

        let result = set_app_aliases_logic(
            &mock,
            "google-chrome".to_string(),
            vec!["browser".to_string()],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("disk full"));
    }

    #[test]
    fn set_app_aliases_logic_empty_aliases_removes_entry() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> = Arc::new(Mutex::new(
            crate::domain::config::AppConfig::default(),
        ));
        {
            let mut cfg = store.lock().unwrap();
            cfg.quick_actions
                .set_app_aliases("google-chrome", vec!["browser".to_string()])
                .unwrap();
        }
        let load_store = store.clone();
        let save_store = store.clone();

        let mut mock = MockConfigService::new();
        mock.expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        mock.expect_save_config()
            .returning(move |cfg: &crate::domain::config::AppConfig| {
                *save_store.lock().unwrap() = cfg.clone();
                Ok(())
            });

        set_app_aliases_logic(&mock, "google-chrome".to_string(), vec![]).expect("expected ok");

        let stored = store.lock().unwrap().clone();
        assert!(!stored.quick_actions.app_aliases.contains_key("google-chrome"));
    }

    #[test]
    fn aliases_updated_payload_serializes_with_snake_case_keys() {
        let payload = AliasesUpdatedPayload {
            app_id: "google-chrome".to_string(),
            aliases: vec!["browser".to_string()],
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["app_id"], "google-chrome");
        assert_eq!(json["aliases"][0], "browser");
    }

    #[test]
    fn bulk_set_aliases_logic_merges_partial_updates() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> = Arc::new(Mutex::new(
            crate::domain::config::AppConfig::default(),
        ));
        {
            let mut cfg = store.lock().unwrap();
            cfg.quick_actions
                .set_app_aliases("firefox", vec!["ff".to_string()])
                .unwrap();
        }
        let load_store = store.clone();
        let save_store = store.clone();

        let mut mock = MockConfigService::new();
        mock.expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        mock.expect_save_config()
            .returning(move |cfg: &crate::domain::config::AppConfig| {
                *save_store.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let entries = vec![
            AliasEntry {
                app_id: "google-chrome".to_string(),
                aliases: vec!["browser".to_string()],
            },
            AliasEntry {
                app_id: "code".to_string(),
                aliases: vec!["editor".to_string(), "ide".to_string()],
            },
        ];

        let payloads = bulk_set_aliases_logic(&mock, entries).expect("expected ok");
        assert_eq!(payloads.len(), 2);
        assert_eq!(payloads[0].app_id, "google-chrome");
        assert_eq!(payloads[1].app_id, "code");

        let stored = store.lock().unwrap().clone();
        assert_eq!(
            stored.quick_actions.app_aliases.get("google-chrome"),
            Some(&vec!["browser".to_string()])
        );
        assert_eq!(
            stored.quick_actions.app_aliases.get("code"),
            Some(&vec!["editor".to_string(), "ide".to_string()])
        );
        assert_eq!(
            stored.quick_actions.app_aliases.get("firefox"),
            Some(&vec!["ff".to_string()])
        );
    }

    #[test]
    fn bulk_set_aliases_with_sink_emits_one_event_per_entry() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> = Arc::new(Mutex::new(
            crate::domain::config::AppConfig::default(),
        ));
        let load_store = store.clone();
        let save_store = store.clone();

        let mut mock = MockConfigService::new();
        mock.expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        mock.expect_save_config()
            .returning(move |cfg: &crate::domain::config::AppConfig| {
                *save_store.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let sink = RecordingEventSink::new();
        let entries = vec![
            AliasEntry {
                app_id: "google-chrome".to_string(),
                aliases: vec!["browser".to_string()],
            },
            AliasEntry {
                app_id: "code".to_string(),
                aliases: vec!["editor".to_string()],
            },
        ];

        bulk_set_aliases_with_sink(&mock, &sink, entries).expect("expected ok");

        let recorded = sink.recorded();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].0, ALIASES_UPDATED_EVENT);
        assert_eq!(recorded[0].1["app_id"], "google-chrome");
        assert_eq!(recorded[0].1["aliases"][0], "browser");
        assert_eq!(recorded[1].0, ALIASES_UPDATED_EVENT);
        assert_eq!(recorded[1].1["app_id"], "code");
        assert_eq!(recorded[1].1["aliases"][0], "editor");
    }

    #[test]
    fn bulk_set_aliases_logic_rejects_when_any_entry_invalid() {
        let mut mock = MockConfigService::new();
        mock.expect_load_config().times(0);
        mock.expect_save_config().times(0);

        let entries = vec![
            AliasEntry {
                app_id: "google-chrome".to_string(),
                aliases: vec!["browser".to_string()],
            },
            AliasEntry {
                app_id: "code".to_string(),
                aliases: vec!["rm -rf /".to_string()],
            },
        ];

        let result = bulk_set_aliases_logic(&mock, entries);
        assert!(result.is_err());
    }

    #[test]
    fn bulk_set_aliases_logic_handles_empty_entries() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> = Arc::new(Mutex::new(
            crate::domain::config::AppConfig::default(),
        ));
        let load_store = store.clone();
        let save_store = store.clone();

        let mut mock = MockConfigService::new();
        mock.expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        mock.expect_save_config()
            .returning(move |cfg: &crate::domain::config::AppConfig| {
                *save_store.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let payloads = bulk_set_aliases_logic(&mock, vec![]).expect("expected ok");
        assert!(payloads.is_empty());

        let stored = store.lock().unwrap().clone();
        assert!(stored.quick_actions.app_aliases.is_empty());
    }

    #[test]
    fn set_disabled_sources_logic_persists_and_emits_status() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> = Arc::new(Mutex::new(
            crate::domain::config::AppConfig::default(),
        ));
        let load_store = store.clone();
        let save_store = store.clone();

        let mut config_mock = MockConfigService::new();
        config_mock
            .expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        config_mock
            .expect_save_config()
            .returning(move |cfg: &crate::domain::config::AppConfig| {
                *save_store.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let mut repo_mock = MockAppRepository::new();
        repo_mock.expect_list_apps().returning(|| {
            Ok(vec![
                fixture("Chrome", AppSource::Desktop),
                fixture("Firefox", AppSource::Desktop),
                fixture("Spotify", AppSource::Flatpak),
            ])
        });

        let payload = set_disabled_sources_logic(
            &config_mock,
            &repo_mock,
            vec!["flatpak".to_string()],
        )
        .expect("expected ok");

        assert_eq!(payload.status.disabled_sources, vec![AppSource::Flatpak]);
        assert_eq!(payload.status.total_apps, 3);

        let stored = store.lock().unwrap().clone();
        assert_eq!(stored.disabled_sources, vec!["flatpak".to_string()]);
    }

    #[test]
    fn set_disabled_sources_with_sink_emits_app_index_status_updated() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> = Arc::new(Mutex::new(
            crate::domain::config::AppConfig::default(),
        ));
        let load_store = store.clone();
        let save_store = store.clone();

        let mut config_mock = MockConfigService::new();
        config_mock
            .expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        config_mock
            .expect_save_config()
            .returning(move |cfg: &crate::domain::config::AppConfig| {
                *save_store.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let mut repo_mock = MockAppRepository::new();
        repo_mock.expect_list_apps().returning(|| {
            Ok(vec![
                fixture("Chrome", AppSource::Desktop),
                fixture("Firefox", AppSource::Desktop),
            ])
        });

        let sink = RecordingEventSink::new();
        let payload = set_disabled_sources_with_sink(
            &config_mock,
            &repo_mock,
            &sink,
            vec!["desktop".to_string()],
        )
        .expect("expected ok");

        assert_eq!(payload.status.disabled_sources, vec![AppSource::Desktop]);
        assert_eq!(payload.status.total_apps, 2);

        let recorded = sink.recorded();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, APP_INDEX_STATUS_UPDATED_EVENT);
        assert_eq!(recorded[0].1["status"]["disabled_sources"][0], "desktop");
        assert_eq!(recorded[0].1["status"]["total_apps"], 2);
    }

    #[test]
    fn set_disabled_sources_logic_rejects_unknown_label() {
        let mut config_mock = MockConfigService::new();
        config_mock.expect_load_config().times(0);
        config_mock.expect_save_config().times(0);

        let mut repo_mock = MockAppRepository::new();
        repo_mock.expect_list_apps().times(0);

        let result = set_disabled_sources_logic(
            &config_mock,
            &repo_mock,
            vec!["unsupported".to_string()],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported"));
    }

    #[test]
    fn set_disabled_sources_logic_rejects_full_disable_without_custom_dirs() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> = Arc::new(Mutex::new(
            crate::domain::config::AppConfig::default(),
        ));
        let load_store = store.clone();

        let mut config_mock = MockConfigService::new();
        config_mock
            .expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        config_mock.expect_save_config().times(0);

        let mut repo_mock = MockAppRepository::new();
        repo_mock.expect_list_apps().times(0);

        let all_sources = vec![
            "desktop".to_string(),
            "flatpak".to_string(),
            "snap".to_string(),
            "appimage".to_string(),
            "nix".to_string(),
            "other".to_string(),
        ];

        let result = set_disabled_sources_logic(&config_mock, &repo_mock, all_sources);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_lowercase()
            .contains("validated"));
    }

    #[test]
    fn get_app_index_status_logic_returns_counts_and_disabled() {
        let mut config_mock = MockConfigService::new();
        let mut cfg = crate::domain::config::AppConfig::default();
        cfg.apply_defaults();
        cfg.disabled_sources = vec!["flatpak".to_string()];
        let stored_cfg = cfg.clone();
        config_mock
            .expect_load_config()
            .returning(move || stored_cfg.clone());

        let mut repo_mock = MockAppRepository::new();
        repo_mock.expect_list_apps().returning(|| {
            Ok(vec![
                fixture("Chrome", AppSource::Desktop),
                fixture("Firefox", AppSource::Desktop),
                fixture("Spotify", AppSource::Flatpak),
                fixture("Discord", AppSource::Snap),
            ])
        });

        let status = get_app_index_status_logic(&repo_mock, &config_mock).expect("expected ok");
        assert_eq!(status.total_apps, 4);
        assert_eq!(status.disabled_sources, vec![AppSource::Flatpak]);
        let counts: HashMap<_, _> = status.sources_with_counts.iter().cloned().collect();
        assert_eq!(counts.get(&AppSource::Desktop), Some(&2));
        assert_eq!(counts.get(&AppSource::Flatpak), Some(&1));
        assert_eq!(counts.get(&AppSource::Snap), Some(&1));
    }

    #[test]
    fn get_app_index_status_logic_handles_repo_failure_with_zero_counts() {
        let mut config_mock = MockConfigService::new();
        let mut cfg = crate::domain::config::AppConfig::default();
        cfg.apply_defaults();
        let stored_cfg = cfg.clone();
        config_mock
            .expect_load_config()
            .returning(move || stored_cfg.clone());

        let mut repo_mock = MockAppRepository::new();
        repo_mock
            .expect_list_apps()
            .returning(|| Err("io error".to_string()));

        let status = get_app_index_status_logic(&repo_mock, &config_mock).expect("expected ok");
        assert_eq!(status.total_apps, 0);
        assert!(status.sources_with_counts.is_empty());
    }

    #[test]
    fn set_disabled_sources_logic_with_custom_dirs_allows_full_disable() {
        let store: Arc<Mutex<crate::domain::config::AppConfig>> = Arc::new(Mutex::new(
            crate::domain::config::AppConfig::default(),
        ));
        {
            let mut cfg = store.lock().unwrap();
            cfg.custom_app_dirs = vec![std::path::PathBuf::from("/opt/custom")];
        }
        let load_store = store.clone();
        let save_store = store.clone();

        let mut config_mock = MockConfigService::new();
        config_mock
            .expect_load_config()
            .returning(move || load_store.lock().unwrap().clone());
        config_mock
            .expect_save_config()
            .returning(move |cfg: &crate::domain::config::AppConfig| {
                *save_store.lock().unwrap() = cfg.clone();
                Ok(())
            });

        let mut repo_mock = MockAppRepository::new();
        repo_mock.expect_list_apps().returning(|| Ok(Vec::new()));

        let all_sources = vec![
            "desktop".to_string(),
            "flatpak".to_string(),
            "snap".to_string(),
            "appimage".to_string(),
            "nix".to_string(),
            "other".to_string(),
        ];

        let payload = set_disabled_sources_logic(&config_mock, &repo_mock, all_sources)
            .expect("custom dirs permit full disable");
        assert_eq!(payload.status.disabled_sources.len(), 6);
    }

    #[test]
    fn disabled_sources_from_config_skips_unknown_entries() {
        let mut mock = MockConfigService::new();
        let mut cfg = crate::domain::config::AppConfig::default();
        cfg.apply_defaults();
        cfg.disabled_sources = vec!["flatpak".to_string(), "garbage".to_string()];
        let stored_cfg = cfg.clone();
        mock.expect_load_config()
            .returning(move || stored_cfg.clone());

        let parsed = disabled_sources_from_config(&mock);
        assert_eq!(parsed, vec![AppSource::Flatpak]);
    }

    #[test]
    fn app_index_status_updated_payload_serializes_with_status_key() {
        let payload = AppIndexStatusUpdatedPayload {
            status: AppIndexStatus {
                total_apps: 4,
                indexed_at: Some(42),
                truncated: false,
                disabled_sources: vec![AppSource::Flatpak],
                sources_with_counts: vec![(AppSource::Desktop, 4)],
            },
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["status"]["total_apps"], 4);
        assert_eq!(json["status"]["indexed_at"], 42);
        assert_eq!(json["status"]["disabled_sources"][0], "flatpak");
    }
}