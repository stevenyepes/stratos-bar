use crate::domain::action::Action;
use crate::ports::history::HistoryRepository;
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
}
