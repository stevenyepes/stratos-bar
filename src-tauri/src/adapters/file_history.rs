use crate::domain::action::Action;
use crate::ports::history::HistoryRepository;
use crate::utils::frecency::{current_time_ms, frecency_score, DEFAULT_HALF_LIFE_HOURS};
use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct FileHistoryAdapter {
    file_path: PathBuf,
    cache: Mutex<Vec<Action>>,
}

impl FileHistoryAdapter {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let file_path = app_data_dir.join("history.json");
        let cache = if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                serde_json::from_str(&content).unwrap_or_else(|_| Vec::new())
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Self {
            file_path,
            cache: Mutex::new(cache),
        }
    }

    fn save(&self) -> Result<(), String> {
        let cache = self.cache.lock().map_err(|e| e.to_string())?;
        let content = serde_json::to_string_pretty(&*cache).map_err(|e| e.to_string())?;
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&self.file_path, content).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[async_trait]
impl HistoryRepository for FileHistoryAdapter {
    async fn get_recent(&self, limit: usize) -> Result<Vec<Action>, String> {
        let cache = self.cache.lock().map_err(|e| e.to_string())?;
        let mut actions = cache.clone();

        actions.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));

        Ok(actions.into_iter().take(limit).collect())
    }

    async fn get_top_frecency(
        &self,
        limit: usize,
        kind_filter: Option<String>,
    ) -> Result<Vec<Action>, String> {
        let cache = self.cache.lock().map_err(|e| e.to_string())?;
        let now = current_time_ms();
        let mut actions: Vec<Action> = cache
            .iter()
            .filter(|a| match &kind_filter {
                Some(k) => &a.kind == k,
                None => true,
            })
            .cloned()
            .collect();

        actions.sort_by(|a, b| {
            let sa = frecency_score(a, now, DEFAULT_HALF_LIFE_HOURS);
            let sb = frecency_score(b, now, DEFAULT_HALF_LIFE_HOURS);
            sb.partial_cmp(&sa)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.last_accessed.cmp(&a.last_accessed))
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(actions.into_iter().take(limit).collect())
    }

    async fn record(&self, mut new_action: Action) -> Result<(), String> {
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;

        if let Some(existing) = cache.iter_mut().find(|a| a.id == new_action.id) {
            existing.last_accessed = new_action.last_accessed;
            existing.frequency += 1;
            existing.name = new_action.name;
            existing.content = new_action.content;
            if new_action.icon.is_some() {
                existing.icon = new_action.icon;
            }
        } else {
            new_action.frequency = 1;
            cache.push(new_action);
        }

        if cache.len() > 100 {
            cache.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));
            cache.truncate(100);
        }

        drop(cache);
        self.save()
    }

    async fn clear(&self) -> Result<(), String> {
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;
        cache.clear();
        drop(cache);
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_action(id: &str, kind: &str, freq: u64, last_accessed: u64) -> Action {
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

    fn fresh_adapter() -> (FileHistoryAdapter, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let adapter = FileHistoryAdapter::new(dir.path().to_path_buf());
        (adapter, dir)
    }

    #[tokio::test]
    async fn record_inserts_new_action_with_frequency_one() {
        let (adapter, _dir) = fresh_adapter();
        let now = current_time_ms();
        adapter
            .record(make_action("chrome", "app", 99, now))
            .await
            .unwrap();
        let v = adapter.get_recent(10).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "chrome");
        assert_eq!(v[0].frequency, 1);
        assert_eq!(v[0].last_accessed, now);
    }

    #[tokio::test]
    async fn record_increments_frequency_on_duplicate_id() {
        let (adapter, _dir) = fresh_adapter();
        let t1 = current_time_ms() - 60_000;
        let t2 = current_time_ms();
        adapter
            .record(make_action("chrome", "app", 1, t1))
            .await
            .unwrap();
        adapter
            .record(make_action("chrome", "app", 1, t2))
            .await
            .unwrap();
        let v = adapter.get_recent(10).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].frequency, 2);
        assert_eq!(v[0].last_accessed, t2);
    }

    #[tokio::test]
    async fn record_refreshes_last_accessed_on_duplicate() {
        let (adapter, _dir) = fresh_adapter();
        let t1 = current_time_ms() - 60_000;
        let t2 = current_time_ms();
        adapter
            .record(make_action("chrome", "app", 1, t1))
            .await
            .unwrap();
        adapter
            .record(make_action("chrome", "app", 1, t2))
            .await
            .unwrap();
        let v = adapter.get_recent(10).await.unwrap();
        assert_eq!(v.len(), 1, "must not push a duplicate");
        assert_eq!(v[0].last_accessed, t2);
    }

    #[tokio::test]
    async fn record_persists_to_disk_and_reloads() {
        let (adapter, dir) = fresh_adapter();
        let now = current_time_ms();
        adapter
            .record(make_action("firefox", "app", 1, now))
            .await
            .unwrap();

        let adapter2 = FileHistoryAdapter::new(dir.path().to_path_buf());
        let v = adapter2.get_recent(10).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "firefox");
        assert_eq!(v[0].frequency, 1);
    }

    #[tokio::test]
    async fn get_top_frecency_orders_by_decayed_score() {
        let (adapter, _dir) = fresh_adapter();
        let now = current_time_ms();
        let one_day_ms = 24 * 3_600_000;
        let two_weeks_ms = 14 * one_day_ms;

        adapter
            .record(make_action("ancient", "app", 1, now - two_weeks_ms))
            .await
            .unwrap();
        adapter
            .record(make_action("recent", "app", 1, now))
            .await
            .unwrap();
        adapter
            .record(make_action("moderate", "app", 1, now - one_day_ms))
            .await
            .unwrap();
        adapter
            .record(make_action("moderate", "app", 1, now - one_day_ms))
            .await
            .unwrap();

        let top = adapter.get_top_frecency(3, None).await.unwrap();
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].id, "moderate");
        assert_eq!(top[1].id, "recent");
        assert_eq!(top[2].id, "ancient");
    }

    #[tokio::test]
    async fn get_top_frecency_filters_by_kind() {
        let (adapter, _dir) = fresh_adapter();
        let now = current_time_ms();
        for _ in 0..5 {
            adapter
                .record(make_action("chrome", "app", 1, now))
                .await
                .unwrap();
        }
        for _ in 0..100 {
            adapter
                .record(make_action("my-script", "script", 1, now))
                .await
                .unwrap();
        }
        for _ in 0..100 {
            adapter
                .record(make_action("my-file", "file", 1, now))
                .await
                .unwrap();
        }

        let apps = adapter.get_top_frecency(10, Some("app".into())).await.unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "chrome");

        let scripts = adapter
            .get_top_frecency(10, Some("script".into()))
            .await
            .unwrap();
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].id, "my-script");
    }

    #[tokio::test]
    async fn get_top_frecency_kind_filter_no_matches_returns_empty() {
        let (adapter, _dir) = fresh_adapter();
        let now = current_time_ms();
        adapter
            .record(make_action("chrome", "app", 1, now))
            .await
            .unwrap();
        let v = adapter
            .get_top_frecency(10, Some("window".into()))
            .await
            .unwrap();
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn get_top_frecency_respects_limit() {
        let (adapter, _dir) = fresh_adapter();
        let now = current_time_ms();
        for i in 0..10 {
            for _ in 0..(i + 1) {
                adapter
                    .record(make_action(&format!("a{i}"), "app", 1, now))
                    .await
                    .unwrap();
            }
        }
        let top = adapter.get_top_frecency(3, None).await.unwrap();
        assert_eq!(top.len(), 3);
    }

    #[tokio::test]
    async fn get_top_frecency_empty_history_returns_empty() {
        let (adapter, _dir) = fresh_adapter();
        let v = adapter.get_top_frecency(10, None).await.unwrap();
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn get_top_frecency_zero_limit_returns_empty() {
        let (adapter, _dir) = fresh_adapter();
        let now = current_time_ms();
        adapter
            .record(make_action("chrome", "app", 1, now))
            .await
            .unwrap();
        let v = adapter.get_top_frecency(0, None).await.unwrap();
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn clear_empties_history() {
        let (adapter, _dir) = fresh_adapter();
        let now = current_time_ms();
        adapter
            .record(make_action("chrome", "app", 1, now))
            .await
            .unwrap();
        adapter.clear().await.unwrap();
        let v = adapter.get_recent(10).await.unwrap();
        assert!(v.is_empty());
        let top = adapter.get_top_frecency(10, None).await.unwrap();
        assert!(top.is_empty());
    }

    #[tokio::test]
    async fn record_caps_cache_at_100_entries() {
        let (adapter, _dir) = fresh_adapter();
        let now = current_time_ms();
        for i in 0..150 {
            adapter
                .record(make_action(&format!("a{i:04}"), "app", 1, now + i))
                .await
                .unwrap();
        }
        let v = adapter.get_recent(500).await.unwrap();
        assert!(v.len() <= 100);
    }
}
