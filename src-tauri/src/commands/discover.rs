use crate::domain::action::Action;
use crate::domain::discover::{BrowseSections, ScoredDiscoverable};
use crate::ports::config_port::ConfigService;
use crate::ports::discover_port::DiscoverService;
use crate::ports::history::HistoryRepository;
use crate::state::AppState;
use tauri::State;

const RECENT_FILE_LIMIT: usize = 20;

pub fn search_discoverable_logic(
    service: &dyn DiscoverService,
    query: &str,
    limit: usize,
) -> Vec<ScoredDiscoverable> {
    service.search(query, limit)
}

#[tauri::command]
pub async fn search_discoverable(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<Vec<ScoredDiscoverable>, String> {
    Ok(search_discoverable_logic(&*state.discover_service, &query, limit))
}

pub async fn browse_discoverable_logic(
    discover: &dyn DiscoverService,
    config: &dyn ConfigService,
    history: &dyn HistoryRepository,
) -> Result<BrowseSections, String> {
    let app_config = config.load_config();
    let recent = history
        .get_recent(RECENT_FILE_LIMIT * 4)
        .await
        .map_err(|e| e.to_string())?;
    let recent_files: Vec<Action> = recent
        .into_iter()
        .filter(|a| a.kind == "file")
        .take(RECENT_FILE_LIMIT)
        .collect();
    Ok(discover.browse(
        app_config.scripts,
        app_config.ai_tools,
        recent_files,
        app_config.shortcuts,
    ))
}

#[tauri::command]
pub async fn browse_discoverable(state: State<'_, AppState>) -> Result<BrowseSections, String> {
    browse_discoverable_logic(
        &*state.discover_service,
        &*state.config_service,
        &*state.history_repository,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::apps::AppSource;
    use crate::domain::config::{AiTool, ScriptConfig};
    use crate::domain::discover::{DiscoverableItem, DiscoverableKind};
    use crate::ports::config_port::MockConfigService;
    use crate::ports::discover_port::MockDiscoverService;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct StubHistory {
        inner: Mutex<Vec<Action>>,
    }

    #[async_trait]
    impl HistoryRepository for StubHistory {
        async fn get_recent(&self, limit: usize) -> Result<Vec<Action>, String> {
            let g = self.inner.lock().unwrap();
            let mut v = g.clone();
            v.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));
            Ok(v.into_iter().take(limit).collect())
        }
        async fn get_top_frecency(
            &self,
            _limit: usize,
            _kind: Option<String>,
        ) -> Result<Vec<Action>, String> {
            Ok(Vec::new())
        }
        async fn record(&self, _a: Action) -> Result<(), String> {
            Ok(())
        }
        async fn clear(&self) -> Result<(), String> {
            Ok(())
        }
    }

    fn action(id: &str, kind: &str, last: u64) -> Action {
        Action {
            id: id.to_string(),
            kind: kind.to_string(),
            content: format!("/tmp/{id}"),
            name: id.to_string(),
            icon: None,
            last_accessed: last,
            frequency: 1,
        }
    }

    #[tokio::test]
    async fn search_discoverable_logic_delegates_to_port() {
        let mut mock = MockDiscoverService::new();
        mock.expect_search()
            .times(1)
            .returning(|_q, limit| {
                vec![ScoredDiscoverable {
                    item: DiscoverableItem {
                        id: "google-chrome".to_string(),
                        name: "Chrome".to_string(),
                        kind: DiscoverableKind::App,
                        ..Default::default()
                    },
                    score: 1.5,
                    matched_field: crate::domain::discover::MatchedField::AliasExact,
                }]
                .into_iter()
                .take(limit)
                .collect()
            });
        let result = search_discoverable_logic(&mock, "browser", 10);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].item.id, "google-chrome");
    }

    #[tokio::test]
    async fn browse_discoverable_logic_returns_structured_sections() {
        let mut discover = MockDiscoverService::new();
        discover.expect_browse().returning(|scripts, ai_tools, recent_files, shortcuts| {
            let mut apps_by_category = HashMap::new();
            apps_by_category.insert(
                "Development".to_string(),
                vec![DiscoverableItem {
                    id: "code".to_string(),
                    name: "Code".to_string(),
                    kind: DiscoverableKind::App,
                    category: Some("Development".to_string()),
                    ..Default::default()
                }],
            );
            BrowseSections {
                apps_by_category,
                scripts,
                ai_tools,
                recent_files,
                shortcuts,
            }
        });

        let mut config = MockConfigService::new();
        config.expect_load_config().returning(|| {
            let mut c = crate::domain::config::AppConfig::default();
            c.scripts.push(ScriptConfig {
                id: "deploy".to_string(),
                alias: "deploy".to_string(),
                path: "/srv/deploy.sh".to_string(),
                args: None,
            });
            c.ai_tools.push(AiTool {
                id: "summarize".to_string(),
                name: "Summarize".to_string(),
                description: "Summarize selection".to_string(),
                prompt_template: "Summarize: {{selection}}".to_string(),
                keywords: vec!["summarize".to_string()],
                icon: "🧠".to_string(),
            });
            c.shortcuts.insert("cmd+shift+s".to_string(), "summarize".to_string());
            c
        });

        let history = StubHistory {
            inner: Mutex::new(vec![
                action("file-1", "file", 5_000),
                action("file-2", "file", 4_000),
                action("app-1", "app", 3_000),
            ]),
        };

        let result = browse_discoverable_logic(&discover, &config, &history)
            .await
            .unwrap();
        assert_eq!(result.apps_by_category.len(), 1);
        assert_eq!(result.scripts.len(), 1);
        assert_eq!(result.scripts[0].alias, "deploy");
        assert_eq!(result.ai_tools.len(), 1);
        assert_eq!(result.ai_tools[0].id, "summarize");
        assert_eq!(result.recent_files.len(), 2);
        assert_eq!(result.shortcuts.get("cmd+shift+s").map(|s| s.as_str()), Some("summarize"));
    }

    #[tokio::test]
    async fn browse_discoverable_logic_recent_files_respects_limit() {
        let mut discover = MockDiscoverService::new();
        discover.expect_browse().returning(|_, _, recent_files, _| BrowseSections {
            recent_files: recent_files.clone(),
            ..Default::default()
        });

        let mut config = MockConfigService::new();
        config.expect_load_config().returning(|| crate::domain::config::AppConfig::default());

        let mut actions = Vec::new();
        for i in 0..50 {
            actions.push(action(&format!("file-{i}"), "file", 1_000 + i as u64));
        }
        let history = StubHistory {
            inner: Mutex::new(actions),
        };

        let result = browse_discoverable_logic(&discover, &config, &history)
            .await
            .unwrap();
        assert_eq!(result.recent_files.len(), RECENT_FILE_LIMIT);
    }

    #[tokio::test]
    async fn browse_discoverable_logic_filters_non_file_actions() {
        let mut discover = MockDiscoverService::new();
        discover.expect_browse().returning(|_, _, recent_files, _| BrowseSections {
            recent_files: recent_files.clone(),
            ..Default::default()
        });

        let mut config = MockConfigService::new();
        config.expect_load_config().returning(|| crate::domain::config::AppConfig::default());

        let history = StubHistory {
            inner: Mutex::new(vec![
                action("app-1", "app", 5_000),
                action("script-1", "script", 4_000),
                action("window-1", "window", 3_000),
            ]),
        };

        let result = browse_discoverable_logic(&discover, &config, &history)
            .await
            .unwrap();
        assert!(result.recent_files.is_empty());
    }

    #[allow(dead_code)]
    fn _unused_appsource() {
        let _ = AppSource::Desktop;
    }
}