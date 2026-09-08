use crate::domain::action::Action;
use crate::ports::history::HistoryRepository;
use async_trait::async_trait;
use std::collections::HashMap;
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

        // Sort by last_accessed descending
        actions.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));

        Ok(actions.into_iter().take(limit).collect())
    }

    async fn record(&self, mut new_action: Action) -> Result<(), String> {
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;

        if let Some(existing) = cache.iter_mut().find(|a| a.id == new_action.id) {
            existing.last_accessed = new_action.last_accessed;
            existing.frequency += 1;
            // Update other fields just in case they changed (dynamic titles etc)
            existing.name = new_action.name;
            existing.content = new_action.content;
            if new_action.icon.is_some() {
                existing.icon = new_action.icon;
            }
        } else {
            new_action.frequency = 1;
            cache.push(new_action);
        }

        // Limit total history size to avoid infinte growth
        if cache.len() > 100 {
            // Remove least recently used or least frequent.
            // For now simple LRU removal
            cache.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));
            cache.truncate(100);
        }

        drop(cache); // Release lock before saving which does I/O
        self.save()
    }

    async fn clear(&self) -> Result<(), String> {
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;
        cache.clear();
        drop(cache);
        self.save()
    }

    async fn rekey_app_ids(&self, mapping: &HashMap<String, String>) -> Result<(), String> {
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;

        // Determine the new id (if any) for each existing action, without mutating yet.
        let targets: Vec<Option<String>> = cache
            .iter()
            .map(|action| {
                if action.kind == "app" {
                    action
                        .id
                        .strip_prefix("app:")
                        .and_then(|exec| mapping.get(exec))
                        .map(|desktop_id| format!("app:{}", desktop_id))
                } else {
                    None
                }
            })
            .collect();

        if targets.iter().all(Option::is_none) {
            // Nothing to migrate: already-migrated data or no matching exec. No-op.
            return Ok(());
        }

        let old_actions = std::mem::take(&mut *cache);
        let mut merged: Vec<Action> = Vec::with_capacity(old_actions.len());

        for (action, target) in old_actions.into_iter().zip(targets.into_iter()) {
            let final_id = target.unwrap_or_else(|| action.id.clone());
            if let Some(existing) = merged.iter_mut().find(|a: &&mut Action| a.id == final_id) {
                existing.frequency += action.frequency;
                if action.last_accessed > existing.last_accessed {
                    existing.last_accessed = action.last_accessed;
                }
            } else {
                let mut rekeyed = action;
                rekeyed.id = final_id;
                merged.push(rekeyed);
            }
        }

        *cache = merged;
        drop(cache);
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_action(id: &str, exec: &str, freq: u64, last_accessed: u64) -> Action {
        Action {
            id: id.to_string(),
            kind: "app".to_string(),
            content: exec.to_string(),
            name: exec.to_string(),
            icon: None,
            last_accessed,
            frequency: freq,
        }
    }

    fn seed(adapter: &FileHistoryAdapter, actions: Vec<Action>) {
        let mut cache = adapter.cache.lock().unwrap();
        *cache = actions;
    }

    fn snapshot(adapter: &FileHistoryAdapter) -> Vec<Action> {
        adapter.cache.lock().unwrap().clone()
    }

    #[tokio::test]
    async fn test_rekey_basic_remaps_legacy_exec_id() {
        let dir = tempdir().unwrap();
        let adapter = FileHistoryAdapter::new(dir.path().to_path_buf());
        seed(
            &adapter,
            vec![make_action(
                "app:/usr/bin/firefox",
                "/usr/bin/firefox",
                5,
                100,
            )],
        );

        let mut mapping = HashMap::new();
        mapping.insert(
            "/usr/bin/firefox".to_string(),
            "firefox.desktop".to_string(),
        );

        adapter.rekey_app_ids(&mapping).await.unwrap();

        let actions = snapshot(&adapter);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "app:firefox.desktop");
        assert_eq!(actions[0].frequency, 5);
        assert_eq!(actions[0].last_accessed, 100);
    }

    #[tokio::test]
    async fn test_rekey_is_idempotent() {
        let dir = tempdir().unwrap();
        let adapter = FileHistoryAdapter::new(dir.path().to_path_buf());
        seed(
            &adapter,
            vec![make_action(
                "app:/usr/bin/firefox",
                "/usr/bin/firefox",
                5,
                100,
            )],
        );

        let mut mapping = HashMap::new();
        mapping.insert(
            "/usr/bin/firefox".to_string(),
            "firefox.desktop".to_string(),
        );

        adapter.rekey_app_ids(&mapping).await.unwrap();
        adapter.rekey_app_ids(&mapping).await.unwrap();

        let actions = snapshot(&adapter);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "app:firefox.desktop");
        assert_eq!(actions[0].frequency, 5);
        assert_eq!(actions[0].last_accessed, 100);
    }

    #[tokio::test]
    async fn test_rekey_merges_colliding_legacy_ids() {
        let dir = tempdir().unwrap();
        let adapter = FileHistoryAdapter::new(dir.path().to_path_buf());
        seed(
            &adapter,
            vec![
                make_action("app:/usr/bin/firefox", "/usr/bin/firefox", 3, 100),
                make_action("app:firefox %u", "firefox %u", 7, 200),
            ],
        );

        let mut mapping = HashMap::new();
        mapping.insert(
            "/usr/bin/firefox".to_string(),
            "firefox.desktop".to_string(),
        );
        mapping.insert("firefox %u".to_string(), "firefox.desktop".to_string());

        adapter.rekey_app_ids(&mapping).await.unwrap();

        let actions = snapshot(&adapter);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "app:firefox.desktop");
        assert_eq!(actions[0].frequency, 10);
        assert_eq!(actions[0].last_accessed, 200);
    }

    #[tokio::test]
    async fn test_rekey_leaves_unmatched_entries_untouched() {
        let dir = tempdir().unwrap();
        let adapter = FileHistoryAdapter::new(dir.path().to_path_buf());
        seed(
            &adapter,
            vec![make_action(
                "app:/usr/bin/unknown",
                "/usr/bin/unknown",
                2,
                50,
            )],
        );

        let mapping = HashMap::new();
        adapter.rekey_app_ids(&mapping).await.unwrap();

        let actions = snapshot(&adapter);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "app:/usr/bin/unknown");
    }
}
