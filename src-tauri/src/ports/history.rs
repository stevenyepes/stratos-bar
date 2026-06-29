use crate::domain::action::Action;
use async_trait::async_trait;

#[async_trait]
pub trait HistoryRepository: Send + Sync {
    async fn get_recent(&self, limit: usize) -> Result<Vec<Action>, String>;

    async fn get_top_frecency(
        &self,
        limit: usize,
        kind_filter: Option<String>,
    ) -> Result<Vec<Action>, String>;

    async fn record(&self, action: Action) -> Result<(), String>;

    async fn clear(&self) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct InMemoryHistory {
        inner: Mutex<Vec<Action>>,
    }

    #[async_trait]
    impl HistoryRepository for InMemoryHistory {
        async fn get_recent(&self, limit: usize) -> Result<Vec<Action>, String> {
            let cache = self.inner.lock().map_err(|e| e.to_string())?;
            let mut v = cache.clone();
            v.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));
            Ok(v.into_iter().take(limit).collect())
        }

        async fn get_top_frecency(
            &self,
            limit: usize,
            kind_filter: Option<String>,
        ) -> Result<Vec<Action>, String> {
            let cache = self.inner.lock().map_err(|e| e.to_string())?;
            let now = crate::utils::frecency::current_time_ms();
            let mut v: Vec<Action> = cache
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
            let mut cache = self.inner.lock().map_err(|e| e.to_string())?;
            if let Some(existing) = cache.iter_mut().find(|a| a.id == action.id) {
                existing.last_accessed = action.last_accessed;
                existing.frequency += 1;
                existing.name = action.name;
                existing.content = action.content;
                if action.icon.is_some() {
                    existing.icon = action.icon;
                }
            } else {
                let mut a = action;
                a.frequency = 1;
                cache.push(a);
            }
            Ok(())
        }

        async fn clear(&self) -> Result<(), String> {
            self.inner.lock().map_err(|e| e.to_string())?.clear();
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
    async fn trait_get_top_frecency_filters_by_kind() {
        let h = InMemoryHistory::default();
        let now = crate::utils::frecency::current_time_ms();
        for (id, kind) in [("c", "app"), ("s", "script"), ("w", "window")] {
            for _ in 0..5 {
                h.record(action(id, kind, 1, now)).await.unwrap();
            }
        }
        let apps_only = h.get_top_frecency(10, Some("app".into())).await.unwrap();
        assert_eq!(apps_only.len(), 1);
        assert_eq!(apps_only[0].id, "c");

        let none = h.get_top_frecency(10, None).await.unwrap();
        assert_eq!(none.len(), 3);
    }

    #[tokio::test]
    async fn trait_get_top_frecency_respects_limit() {
        let h = InMemoryHistory::default();
        let now = crate::utils::frecency::current_time_ms();
        for i in 0..5 {
            for _ in 0..(i + 1) {
                h.record(action(&format!("a{i}"), "app", 1, now))
                    .await
                    .unwrap();
            }
        }
        let v = h.get_top_frecency(2, None).await.unwrap();
        assert_eq!(v.len(), 2);
        assert!(v[0].frequency >= v[1].frequency);
    }

    #[tokio::test]
    async fn trait_record_merges_duplicates() {
        let h = InMemoryHistory::default();
        let now = crate::utils::frecency::current_time_ms();
        h.record(action("c", "app", 1, now - 1000)).await.unwrap();
        h.record(action("c", "app", 1, now)).await.unwrap();
        let v = h.get_recent(10).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].frequency, 2);
        assert_eq!(v[0].last_accessed, now);
    }

    #[allow(dead_code)]
    fn _unused_hashmap_to_keep_import() {
        let _: HashMap<String, u32> = HashMap::new();
    }
}
