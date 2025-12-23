use crate::ports::icon_port::IconResolver;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct CachedIconResolver {
    cache: Arc<Mutex<HashMap<String, Option<String>>>>,
}

impl CachedIconResolver {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn resolve_icon_internal(&self, icon_name: &str) -> Option<String> {
        // 1. Direct path check
        let path = std::path::Path::new(icon_name);
        if path.is_absolute() && path.exists() {
            let resolved_path = match std::fs::canonicalize(path) {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => icon_name.to_string(),
            };
            return Some(resolved_path);
        }

        // 2. Use linicon
        // Iterate over all results instead of just first, prioritize scalable/png
        if let Some(icon_path) = linicon::lookup_icon(icon_name).next() {
            if let Ok(path_str) = icon_path {
                let resolved_path = match std::fs::canonicalize(&path_str.path) {
                    Ok(p) => p.to_string_lossy().to_string(),
                    Err(_) => path_str.path.to_string_lossy().to_string(),
                };
                return Some(resolved_path);
            }
        }

        // 3. Fallback: Manual search in standard paths
        // Linicon might fail if theme config is weird or specific sizes not found?
        // Let's do a robust manual search for common cases (steam, hicolor default)

        // This part requires `dirs`
        let mut search_paths = Vec::new();

        // Standard XDG paths
        if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
            for dir in dirs.split(':') {
                let p = std::path::Path::new(dir);
                search_paths.push(p.join("icons/hicolor/48x48/apps"));
                search_paths.push(p.join("icons/hicolor/32x32/apps"));
                search_paths.push(p.join("icons/hicolor/128x128/apps"));
                search_paths.push(p.join("icons/hicolor/scalable/apps"));
                search_paths.push(p.join("pixmaps"));
                search_paths.push(p.join("icons"));
            }
        } else {
            search_paths.push(std::path::PathBuf::from(
                "/usr/share/icons/hicolor/48x48/apps",
            ));
            search_paths.push(std::path::PathBuf::from(
                "/usr/share/icons/hicolor/32x32/apps",
            ));
            search_paths.push(std::path::PathBuf::from(
                "/usr/share/icons/hicolor/scalable/apps",
            ));
            search_paths.push(std::path::PathBuf::from("/usr/share/pixmaps"));
            search_paths.push(std::path::PathBuf::from("/usr/share/icons"));
        }

        // User local paths
        if let Some(home) = dirs::data_local_dir() {
            search_paths.push(home.join("icons/hicolor/48x48/apps"));
            search_paths.push(home.join("icons/hicolor/32x32/apps"));
            search_paths.push(home.join("icons/hicolor/128x128/apps"));
            search_paths.push(home.join("icons/hicolor/scalable/apps"));
            search_paths.push(home.join("icons"));
        }

        // Steam specific paths
        if let Some(home) = dirs::home_dir() {
            search_paths.push(home.join(".steam/root/appcache/librarycache"));
            search_paths.push(home.join(".local/share/icons/hicolor/48x48/apps"));
        }

        let extensions = vec!["png", "svg", "xpm", "ico", "jpg"];

        for base in &search_paths {
            if !base.exists() {
                continue;
            }

            // Steam specific cache check (icon_name usually steam_icon_APPID)
            if base.ends_with("librarycache") && icon_name.starts_with("steam_icon_") {
                let app_id = icon_name.trim_start_matches("steam_icon_");
                // try app_id_icon.jpg
                let p = base.join(format!("{}_icon.jpg", app_id));
                if p.exists() {
                    return Some(p.to_string_lossy().to_string());
                }
            }

            for ext in &extensions {
                let p = base.join(format!("{}.{}", icon_name, ext));
                if p.exists() {
                    let resolved_path = match std::fs::canonicalize(&p) {
                        Ok(canonical) => canonical.to_string_lossy().to_string(),
                        Err(_) => p.to_string_lossy().to_string(),
                    };
                    return Some(resolved_path);
                }
            }
        }

        // Try stripping extension if present in name but not a path
        if let Some(stem) = std::path::Path::new(icon_name).file_stem() {
            if stem != icon_name {
                let stem_str = stem.to_string_lossy();
                if let Some(icon_path) = linicon::lookup_icon(&stem_str).next() {
                    if let Ok(path_str) = icon_path {
                        return Some(path_str.path.to_string_lossy().to_string());
                    }
                }
            }
        }

        None
    }
}

impl IconResolver for CachedIconResolver {
    fn resolve_icon(&self, icon_name: &str) -> Option<String> {
        // Check cache
        if let Ok(guard) = self.cache.lock() {
            if let Some(cached) = guard.get(icon_name) {
                return cached.clone();
            }
        }

        let result = self.resolve_icon_internal(icon_name);

        // Update cache
        if let Ok(mut guard) = self.cache.lock() {
            guard.insert(icon_name.to_string(), result.clone());
        }

        result
    }
}
