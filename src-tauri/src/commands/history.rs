use crate::domain::action::Action;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_recent_actions(
    state: State<'_, AppState>,
    limit: usize,
) -> Result<Vec<Action>, String> {
    state.history_repository.get_recent(limit).await
}

#[tauri::command]
pub async fn get_top_frecency(
    state: State<'_, AppState>,
    limit: usize,
    kind_filter: Option<String>,
) -> Result<Vec<Action>, String> {
    state
        .history_repository
        .get_top_frecency(limit, kind_filter)
        .await
}

#[tauri::command]
pub async fn record_action(state: State<'_, AppState>, action: Action) -> Result<(), String> {
    state.history_repository.record(action).await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.history_repository.clear().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::history::HistoryRepository;
    use async_trait::async_trait;
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
            limit: usize,
            kind_filter: Option<String>,
        ) -> Result<Vec<Action>, String> {
            let g = self.inner.lock().unwrap();
            let now = crate::utils::frecency::current_time_ms();
            let mut v: Vec<Action> = g
                .iter()
                .filter(|a| match &kind_filter {
                    Some(k) => &a.kind == k,
                    None => true,
                })
                .cloned()
                .collect();
            v.sort_by(|a, b| {
                let sa = crate::utils::frecency::frecency_score(a, now, 336.0);
                let sb = crate::utils::frecency::frecency_score(b, now, 336.0);
                sb.partial_cmp(&sa)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.last_accessed.cmp(&a.last_accessed))
            });
            Ok(v.into_iter().take(limit).collect())
        }

        async fn record(&self, action: Action) -> Result<(), String> {
            let mut g = self.inner.lock().unwrap();
            if let Some(existing) = g.iter_mut().find(|a| a.id == action.id) {
                existing.frequency += 1;
                existing.last_accessed = action.last_accessed;
            } else {
                let mut a = action;
                a.frequency = 1;
                g.push(a);
            }
            Ok(())
        }

        async fn clear(&self) -> Result<(), String> {
            self.inner.lock().unwrap().clear();
            Ok(())
        }
    }

    fn action(id: &str, kind: &str, freq: u64, last_accessed: u64) -> Action {
        Action {
            id: id.to_string(),
            kind: kind.to_string(),
            content: format!("/usr/bin/{id}"),
            name: id.to_string(),
            icon: None,
            last_accessed,
            frequency: freq,
        }
    }

    #[tokio::test]
    async fn command_get_top_frecency_delegates_to_port() {
        let now = crate::utils::frecency::current_time_ms();
        let one_day_ms = 24 * 3_600_000;

        let recent = StubHistory {
            inner: Mutex::new(vec![
                action("ancient", "app", 100, now - one_day_ms),
                action("recent", "app", 5, now),
            ]),
        };

        let top_all = recent.get_top_frecency(10, None).await.unwrap();
        assert_eq!(top_all.len(), 2);
        assert_eq!(top_all[0].id, "ancient");
        assert_eq!(top_all[1].id, "recent");

        let top_apps_only = recent
            .get_top_frecency(10, Some("script".into()))
            .await
            .unwrap();
        assert!(top_apps_only.is_empty());
    }

    #[tokio::test]
    async fn command_record_increments_existing_frequency() {
        let stub = StubHistory::default();
        let t1 = crate::utils::frecency::current_time_ms() - 60_000;
        let t2 = crate::utils::frecency::current_time_ms();
        stub.record(action("c", "app", 1, t1)).await.unwrap();
        stub.record(action("c", "app", 1, t2)).await.unwrap();
        let v = stub.get_recent(10).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].frequency, 2);
        assert_eq!(v[0].last_accessed, t2);
    }
}
