use crate::ports::icon_port::IconResolver;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct CachedIconResolver {
    cache: Arc<Mutex<HashMap<(String, u16, u16, String), Option<String>>>>,
    theme: String,
}

impl CachedIconResolver {
    pub fn new() -> Self {
        let theme = freedesktop_icons::default_theme_gtk().unwrap_or_else(|| "hicolor".to_string());
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            theme,
        }
    }

    /// Skips GTK theme detection so tests are deterministic regardless of the
    /// machine's actual desktop settings.
    #[cfg(test)]
    pub fn new_with_theme(theme: String) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            theme,
        }
    }

    fn resolve_icon_internal(&self, icon_name: &str, size: u16, scale: u16) -> Option<String> {
        // 1. Direct path check
        let path = std::path::Path::new(icon_name);
        if path.is_absolute() && path.exists() {
            let resolved_path = match std::fs::canonicalize(path) {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => icon_name.to_string(),
            };
            return Some(resolved_path);
        }

        // 2. Spec-correct theme lookup, with built-in fallback to hicolor/pixmaps.
        let icon_path = freedesktop_icons::lookup(icon_name)
            .with_size(size)
            .with_scale(scale)
            .with_theme(&self.theme)
            .find()?;

        let resolved_path = match std::fs::canonicalize(&icon_path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => icon_path.to_string_lossy().to_string(),
        };
        Some(resolved_path)
    }
}

impl IconResolver for CachedIconResolver {
    fn resolve_icon(&self, icon_name: &str, size: u16, scale: u16) -> Option<String> {
        let key = (icon_name.to_string(), size, scale, self.theme.clone());

        // Check cache
        if let Ok(guard) = self.cache.lock() {
            if let Some(cached) = guard.get(&key) {
                return cached.clone();
            }
        }

        let result = self.resolve_icon_internal(icon_name, size, scale);

        // Update cache
        if let Ok(mut guard) = self.cache.lock() {
            guard.insert(key, result.clone());
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::sync::OnceLock;
    use tempfile::{tempdir, TempDir};

    /// `freedesktop_icons` caches its theme/base-path discovery in
    /// process-wide `Lazy` statics that read `XDG_DATA_HOME`/`XDG_DATA_DIRS`
    /// only on first access. Every test in this module that can reach the
    /// `freedesktop_icons::lookup` path must call this first so the fixture
    /// theme is in place before those statics are ever initialized,
    /// regardless of which test's thread gets there first.
    fn test_theme_fixture_dir() -> &'static std::path::Path {
        static FIXTURE: OnceLock<TempDir> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let dir = tempdir().unwrap();
                let theme_dir = dir.path().join("icons").join("ThemeName");

                let dir_48 = theme_dir.join("48x48/apps");
                let dir_16 = theme_dir.join("16x16/apps");
                std::fs::create_dir_all(&dir_48).unwrap();
                std::fs::create_dir_all(&dir_16).unwrap();

                std::fs::write(
                    theme_dir.join("index.theme"),
                    "[Icon Theme]\n\
                     Name=ThemeName\n\
                     Comment=Test fixture theme\n\
                     \n\
                     [48x48/apps]\n\
                     Size=48\n\
                     Type=Fixed\n\
                     \n\
                     [16x16/apps]\n\
                     Size=16\n\
                     Type=Fixed\n",
                )
                .unwrap();

                File::create(dir_48.join("test-themed-icon.png")).unwrap();
                File::create(dir_48.join("cache-key-icon.png")).unwrap();
                File::create(dir_16.join("cache-key-icon.png")).unwrap();

                std::env::set_var("XDG_DATA_HOME", dir.path());
                std::env::set_var("XDG_DATA_DIRS", dir.path());

                dir
            })
            .path()
    }

    #[test]
    fn test_resolve_absolute_path() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_icon.png");
        File::create(&file_path).unwrap();

        let resolver = CachedIconResolver::new_with_theme("hicolor".to_string());
        let result = resolver.resolve_icon(file_path.to_str().unwrap(), 24, 1);

        assert!(result.is_some());
        // Canonicalization might resolve symlinks or change format, but for temp dir it should be close
        // We just check it's some valid path
        assert!(result.unwrap().contains("test_icon.png"));
    }

    #[test]
    fn test_caching_behavior() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_cache.png");
        File::create(&file_path).unwrap();

        let resolver = CachedIconResolver::new_with_theme("hicolor".to_string());
        let path_str = file_path.to_str().unwrap();

        // First resolution
        assert!(resolver.resolve_icon(path_str, 24, 1).is_some());

        // Delete valid file
        std::fs::remove_file(&file_path).unwrap();

        // Second resolution should hit cache and still return result even if file is gone
        // functionality of "resolve_icon_internal" checks existence, but "resolve_icon" checks cache first.
        let result = resolver.resolve_icon(path_str, 24, 1);
        assert!(result.is_some());
    }

    #[test]
    fn test_missing_icon_returns_none() {
        test_theme_fixture_dir();
        let resolver = CachedIconResolver::new_with_theme("ThemeName".to_string());
        let result = resolver.resolve_icon("/non/existent/path/icon.png", 24, 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_themed_icon_resolves_in_theme_dir() {
        test_theme_fixture_dir();
        let resolver = CachedIconResolver::new_with_theme("ThemeName".to_string());

        let result = resolver.resolve_icon("test-themed-icon", 48, 1);

        assert!(result.is_some());
        assert!(result.unwrap().contains("ThemeName"));
    }

    #[test]
    fn test_cache_key_includes_size() {
        let fixture_dir = test_theme_fixture_dir();
        let resolver = CachedIconResolver::new_with_theme("ThemeName".to_string());

        let result_16_first = resolver.resolve_icon("cache-key-icon", 16, 1);
        assert!(result_16_first.is_some());
        assert!(result_16_first.as_ref().unwrap().contains("16x16"));

        // Remove the size-16 backing file: if the cache key collapsed sizes
        // together, a later size-48 lookup or a re-fetch of size-16 would be
        // affected by this.
        std::fs::remove_file(fixture_dir.join("icons/ThemeName/16x16/apps/cache-key-icon.png"))
            .unwrap();

        let result_48 = resolver.resolve_icon("cache-key-icon", 48, 1);
        assert!(result_48.is_some());
        assert!(result_48.as_ref().unwrap().contains("48x48"));
        assert_ne!(result_48, result_16_first);

        // The size-16 entry must still be served from cache, unaffected by
        // the deletion or by the intervening size-48 lookup.
        let result_16_second = resolver.resolve_icon("cache-key-icon", 16, 1);
        assert_eq!(result_16_second, result_16_first);
    }
}
