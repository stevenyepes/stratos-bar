use crate::adapters::fuzzy_index::{install_shared, shared_index, FuzzyIndexAdapter};
use crate::domain::action::Action;
use crate::domain::discover::{BrowseSections, DiscoverableItem};
use crate::ports::discover_port::DiscoverService;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

pub async fn warm_discover_index(state: &AppState) -> Result<(), String> {
    if shared_index().read().ok().and_then(|g| g.clone()).is_some() {
        return Ok(());
    }
    let adapter = Arc::new(FuzzyIndexAdapter::new(
        state.app_repository.clone(),
        state.history_repository.clone(),
        state.config_service.clone(),
    ));
    adapter.warm().await?;
    install_shared(adapter);
    Ok(())
}

pub async fn search_discoverable_async(
    adapter: &FuzzyIndexAdapter,
    query: &str,
    limit: usize,
) -> Vec<DiscoverableItem> {
    <FuzzyIndexAdapter as DiscoverService>::search(adapter, query, limit).await
}

#[tauri::command]
pub async fn search_discoverable(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<Vec<DiscoverableItem>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let adapter = match shared_index().read().ok().and_then(|g| g.clone()) {
        Some(adapter) => adapter,
        None => {
            let adapter = Arc::new(FuzzyIndexAdapter::new(
                state.app_repository.clone(),
                state.history_repository.clone(),
                state.config_service.clone(),
            ));
            adapter.warm().await?;
            install_shared(adapter.clone());
            adapter
        }
    };

    let results = adapter.search(&query, limit).await;
    let _ = app_handle.emit("discoverable-updated", &results);
    Ok(results)
}

#[tauri::command]
pub async fn warm_discover_index_command(state: State<'_, AppState>) -> Result<(), String> {
    warm_discover_index(&state).await
}

const BROWSE_RECENT_FILE_LIMIT: usize = 20;

pub fn browse_discoverable_logic(
    adapter: &FuzzyIndexAdapter,
    scripts: Vec<crate::domain::config::ScriptConfig>,
    ai_tools: Vec<crate::domain::config::AiTool>,
    recent_files: Vec<Action>,
    shortcuts: std::collections::HashMap<String, String>,
) -> Result<BrowseSections, String> {
    let snap = adapter.current_snapshot().ok_or_else(|| {
        let _ = adapter.ensure_warm();
        "discover index not warm".to_string()
    });
    let snap = match snap {
        Ok(s) => s,
        Err(_) => return browse_from_state(adapter, scripts, ai_tools, recent_files, shortcuts),
    };

    let items = adapter.all_apps(&snap);
    let scripts_filtered = scripts;
    let ai_tools_filtered = ai_tools;
    let recent = recent_files
        .into_iter()
        .take(BROWSE_RECENT_FILE_LIMIT)
        .collect();
    Ok(BrowseSections::from_snapshot(
        items,
        scripts_filtered,
        ai_tools_filtered,
        recent,
        shortcuts,
    ))
}

fn browse_from_state(
    adapter: &FuzzyIndexAdapter,
    scripts: Vec<crate::domain::config::ScriptConfig>,
    ai_tools: Vec<crate::domain::config::AiTool>,
    recent_files: Vec<Action>,
    shortcuts: std::collections::HashMap<String, String>,
) -> Result<BrowseSections, String> {
    let apps = adapter
        .app_repository
        .list_apps()
        .unwrap_or_default()
        .iter()
        .map(crate::domain::discover::DiscoverableItem::from_app)
        .collect();
    let recent = recent_files
        .into_iter()
        .take(BROWSE_RECENT_FILE_LIMIT)
        .collect();
    Ok(BrowseSections::from_snapshot(
        apps,
        scripts,
        ai_tools,
        recent,
        shortcuts,
    ))
}

#[tauri::command]
pub async fn browse_discoverable(
    state: State<'_, AppState>,
) -> Result<BrowseSections, String> {
    let adapter = match shared_index().read().ok().and_then(|g| g.clone()) {
        Some(adapter) => adapter,
        None => {
            warm_discover_index(&state).await?;
            shared_index()
                .read()
                .map_err(|e| e.to_string())?
                .clone()
                .ok_or_else(|| "discover index not initialized".to_string())?
        }
    };

    let config = state.config_service.load_config();
    let recent = state
        .history_repository
        .get_recent(BROWSE_RECENT_FILE_LIMIT * 4)
        .await
        .map_err(|e| e.to_string())?;
    let recent_files: Vec<Action> = recent
        .into_iter()
        .filter(|a| a.kind == "file")
        .take(BROWSE_RECENT_FILE_LIMIT)
        .collect();

    browse_discoverable_logic(
        &adapter,
        config.scripts,
        config.ai_tools,
        recent_files,
        config.shortcuts,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::action::Action;
    use crate::domain::apps::{AppEntry, AppSource};
    use crate::domain::config::{AiTool, AppConfig, ScriptConfig};
    use crate::domain::discover::{DiscoverableKind, DiscoverableLaunch};
    use crate::ports::app_port::MockAppRepository;
    use crate::ports::config_port::MockConfigService;
    use crate::ports::history::HistoryRepository;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn make_app(id: &str, name: &str) -> AppEntry {
        AppEntry {
            id: id.to_string(),
            name: name.to_string(),
            generic_name: None,
            description: None,
            keywords: Vec::new(),
            exec: id.to_string(),
            try_exec: None,
            icon: None,
            categories: Vec::new(),
            startup_wm_class: None,
            source: AppSource::Desktop,
            path: format!("/usr/share/applications/{id}.desktop"),
        }
    }

    fn make_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.apply_defaults();
        let mut shortcuts = HashMap::new();
        shortcuts.insert("chrome-launcher".to_string(), "google-chrome".to_string());
        config.shortcuts = shortcuts;
        config.scripts = vec![ScriptConfig {
            id: "deploy".to_string(),
            alias: "deploy".to_string(),
            path: "/srv/scripts/deploy.sh".to_string(),
            args: Some("--staging".to_string()),
        }];
        config.ai_tools = vec![AiTool {
            id: "rephrase".to_string(),
            name: "Rephrase".to_string(),
            description: "Improve clarity".to_string(),
            prompt_template: String::new(),
            keywords: vec!["rewrite".to_string()],
            icon: String::new(),
        }];
        config
    }

    fn make_adapter(apps: Vec<AppEntry>, config: AppConfig) -> FuzzyIndexAdapter {
        let mut app_mock = MockAppRepository::new();
        app_mock
            .expect_list_apps()
            .returning(move || Ok(apps.clone()));
        let mut config_mock = MockConfigService::new();
        let cfg = config.clone();
        config_mock.expect_load_config().returning(move || cfg.clone());
        let history: Arc<dyn HistoryRepository> = Arc::new(StubHistory::default());
        FuzzyIndexAdapter::new(
            Arc::new(app_mock),
            history,
            Arc::new(config_mock),
        )
    }

    fn make_adapter_with_history(
        apps: Vec<AppEntry>,
        config: AppConfig,
        actions: Vec<Action>,
    ) -> FuzzyIndexAdapter {
        let mut app_mock = MockAppRepository::new();
        app_mock
            .expect_list_apps()
            .returning(move || Ok(apps.clone()));
        let mut config_mock = MockConfigService::new();
        let cfg = config.clone();
        config_mock.expect_load_config().returning(move || cfg.clone());
        let history: Arc<dyn HistoryRepository> = Arc::new(StubHistory::with(actions.clone()));
        FuzzyIndexAdapter::new(
            Arc::new(app_mock),
            history,
            Arc::new(config_mock),
        )
    }

    #[derive(Default)]
    struct StubHistory {
        inner: Mutex<Vec<Action>>,
    }

    impl StubHistory {
        fn with(actions: Vec<Action>) -> Self {
            Self {
                inner: Mutex::new(actions),
            }
        }
    }

    #[async_trait]
    impl HistoryRepository for StubHistory {
        async fn get_recent(&self, limit: usize) -> Result<Vec<Action>, String> {
            let g = self.inner.lock().unwrap();
            let mut v = g.clone();
            v.sort_by_key(|a| std::cmp::Reverse(a.last_accessed));
            Ok(v.into_iter().take(limit).collect())
        }

        async fn get_top_frecency(
            &self,
            limit: usize,
            _kind_filter: Option<String>,
        ) -> Result<Vec<Action>, String> {
            let g = self.inner.lock().unwrap();
            let now = crate::utils::frecency::current_time_ms();
            let mut v = g.clone();
            v.sort_by(|a, b| {
                let sa = crate::utils::frecency::frecency_score(a, now, 336.0);
                let sb = crate::utils::frecency::frecency_score(b, now, 336.0);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });
            Ok(v.into_iter().take(limit).collect())
        }

        async fn record(&self, _action: Action) -> Result<(), String> {
            Ok(())
        }

        async fn clear(&self) -> Result<(), String> {
            self.inner.lock().unwrap().clear();
            Ok(())
        }
    }

    #[tokio::test]
    async fn search_discoverable_async_returns_unified_list() {
        let apps = vec![make_app("chrome", "Chrome")];
        let adapter = make_adapter(apps, make_config());
        let results = search_discoverable_async(&adapter, "chr", 20).await;
        assert!(!results.is_empty());
        assert!(results
            .iter()
            .any(|r| matches!(r.kind, DiscoverableKind::App) && r.name == "Chrome"));
        assert!(results
            .iter()
            .any(|r| matches!(r.kind, DiscoverableKind::Shortcut) && r.name == "chrome-launcher"));
    }

    #[tokio::test]
    async fn search_discoverable_async_excludes_empty_query() {
        let adapter = make_adapter(vec![], make_config());
        let r = search_discoverable_async(&adapter, "", 10).await;
        assert!(r.is_empty());
        let r = search_discoverable_async(&adapter, "   ", 10).await;
        assert!(r.is_empty());
    }

    #[test]
    fn discoverable_item_shape_matches_spec() {
        let item = DiscoverableItem {
            kind: DiscoverableKind::App,
            id: "google-chrome".to_string(),
            name: "Google Chrome".to_string(),
            description: Some("Browser".to_string()),
            icon: None,
            keywords: Vec::new(),
            score: 2.0,
            source: "app:desktop".to_string(),
            launch: DiscoverableLaunch::App {
                exec: "google-chrome".to_string(),
            },
            category: None,
            aliases: Vec::new(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"kind\":\"app\""));
        assert!(json.contains("\"id\":\"google-chrome\""));
        assert!(json.contains("\"exec\":\"google-chrome\""));
    }

    #[tokio::test]
    async fn history_port_drives_usage_frequency_boost() {
        let now = crate::utils::frecency::current_time_ms();
        let one_day_ms = 24 * 3_600_000;
        let apps = vec![make_app("hot", "Hot"), make_app("cold", "Cold")];
        let actions = vec![
            Action {
                id: "hot".to_string(),
                kind: "app".to_string(),
                content: "/usr/bin/hot".to_string(),
                name: "Hot".to_string(),
                icon: None,
                last_accessed: now,
                frequency: 100,
            },
            Action {
                id: "cold".to_string(),
                kind: "app".to_string(),
                content: "/usr/bin/cold".to_string(),
                name: "Cold".to_string(),
                icon: None,
                last_accessed: now - one_day_ms * 14,
                frequency: 1,
            },
        ];
        let adapter = make_adapter_with_history(apps, make_config(), actions);
        let results = adapter.search("o", 10).await;
        let hot = results.iter().find(|r| r.id == "hot").unwrap();
        let cold = results.iter().find(|r| r.id == "cold").unwrap();
        assert!(
            hot.score > cold.score,
            "hot should outrank cold: hot={}, cold={}",
            hot.score,
            cold.score
        );
    }

    #[tokio::test]
    async fn warm_installs_shared_index() {
        let apps = vec![make_app("chrome", "Chrome")];
        let adapter = make_adapter(apps, make_config());
        adapter.warm().await.unwrap();
        install_shared(Arc::new(adapter));
        let shared = shared_index().read().unwrap().clone();
        assert!(shared.is_some());
    }

    fn make_categorized_app(id: &str, name: &str, category: &str) -> AppEntry {
        let mut app = make_app(id, name);
        app.categories = vec![category.to_string()];
        app
    }

    #[tokio::test]
    async fn browse_discoverable_logic_groups_apps_by_category() {
        let apps = vec![
            make_categorized_app("chrome", "Chrome", "Internet"),
            make_categorized_app("firefox", "Firefox", "Internet"),
            make_categorized_app("code", "Code", "Development"),
        ];
        let adapter = make_adapter(apps, make_config());
        adapter.warm().await.unwrap();

        let mut shortcuts = HashMap::new();
        shortcuts.insert("browser".to_string(), "google-chrome".to_string());
        let recent_files = vec![Action {
            id: "f1".to_string(),
            kind: "file".to_string(),
            content: "/tmp/note.md".to_string(),
            name: "note.md".to_string(),
            icon: None,
            last_accessed: 1,
            frequency: 1,
        }];
        let result = browse_discoverable_logic(
            &adapter,
            make_config().scripts,
            make_config().ai_tools,
            recent_files,
            shortcuts,
        )
        .unwrap();
        assert_eq!(result.apps_by_category.get("Internet").unwrap().len(), 2);
        assert_eq!(
            result.apps_by_category.get("Development").unwrap().len(),
            1
        );
        assert_eq!(result.recent_files.len(), 1);
        assert_eq!(
            result.shortcuts.get("browser").map(|s| s.as_str()),
            Some("google-chrome")
        );
    }

    #[tokio::test]
    async fn browse_discoverable_logic_respects_recent_file_limit() {
        let adapter = make_adapter(vec![], make_config());
        adapter.warm().await.unwrap();

        let mut recent = Vec::new();
        for i in 0..50 {
            recent.push(Action {
                id: format!("f{i}"),
                kind: "file".to_string(),
                content: format!("/tmp/file{i}"),
                name: format!("file{i}"),
                icon: None,
                last_accessed: i as u64,
                frequency: 1,
            });
        }
        let result = browse_discoverable_logic(
            &adapter,
            Vec::new(),
            Vec::new(),
            recent,
            HashMap::new(),
        )
        .unwrap();
        assert_eq!(result.recent_files.len(), 20);
    }
}