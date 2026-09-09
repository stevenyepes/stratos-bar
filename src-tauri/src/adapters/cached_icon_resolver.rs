use crate::ports::icon_port::IconResolver;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Errors from the byte-serving path (`serve_icon_bytes`), distinct from the
/// `Option`-based `resolve_icon` path used by `IconResolver`.
#[derive(Debug, PartialEq, Eq)]
pub enum IconAccessError {
    NotFound,
    OutsideAllowedRoots,
    ReadFailed(String),
}

/// The single source of truth for "what counts as an icon root." Hand-rolled
/// from `dirs` + manual `XDG_DATA_HOME`/`XDG_DATA_DIRS` parsing (deliberately not
/// the `xdg` crate) so it stays testable through the same env-var seam as
/// `test_theme_fixture_dir`. Mirrors `FsAppRepository::collect_search_paths`
/// (fs_app_repository.rs:140-168): every candidate is existence-gated before
/// being included, live directories only, no persistence.
pub fn compute_icon_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    let xdg_data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")));
    if let Some(data_home) = xdg_data_home {
        let p = data_home.join("icons");
        if p.exists() {
            roots.push(p);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let icons = home.join(".icons");
        if icons.exists() {
            roots.push(icons);
        }
        let steam = home.join(".steam");
        if steam.exists() {
            roots.push(steam);
        }
    }

    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    for entry in xdg_data_dirs.split(':').filter(|s| !s.is_empty()) {
        let p = PathBuf::from(entry).join("icons");
        if p.exists() {
            roots.push(p);
        }
    }

    let pixmaps = PathBuf::from("/usr/share/pixmaps");
    if pixmaps.exists() {
        roots.push(pixmaps);
    }

    if let Some(local) = dirs::data_local_dir() {
        let p = local.join("flatpak/exports/share/icons");
        if p.exists() {
            roots.push(p);
        }
    }

    let system_flatpak = PathBuf::from("/var/lib/flatpak/exports/share/icons");
    if system_flatpak.exists() {
        roots.push(system_flatpak);
    }

    for snap_dir in ["/var/lib/snapd/desktop/icons", "/snap/desktop/icons"] {
        let p = PathBuf::from(snap_dir);
        if p.exists() {
            roots.push(p);
        }
    }

    roots
}

/// Canonical-path containment check: canonicalizes `candidate` and does a
/// component-wise prefix check against each canonicalized root, so a path like
/// `/usr/share/icons-evil` cannot match a root of `/usr/share/icons` (a raw
/// string prefix check would let it through). Returns `false` if `candidate`
/// fails to canonicalize, rather than silently falling back to the raw path.
pub fn is_within_roots(candidate: &Path, roots: &[PathBuf]) -> bool {
    let canonical_candidate = match std::fs::canonicalize(candidate) {
        Ok(p) => p,
        Err(_) => return false,
    };

    path_has_root_prefix(&canonical_candidate, roots)
}

/// Shared root-prefix check for a path that is already canonical (or, for a
/// not-yet-existing leaf, whose parent is canonical -- see `serve_icon_bytes`).
/// Split out from `is_within_roots` because that function requires `candidate`
/// itself to exist on disk (it re-canonicalizes), which a missing icon file
/// does not.
fn path_has_root_prefix(canonical_candidate: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| {
        std::fs::canonicalize(root)
            .map(|canonical_root| canonical_candidate.starts_with(&canonical_root))
            .unwrap_or(false)
    })
}

pub struct CachedIconResolver {
    cache: Arc<Mutex<HashMap<(String, u16, u16, String), Option<String>>>>,
    byte_cache: Arc<Mutex<HashMap<PathBuf, Arc<Vec<u8>>>>>,
    theme: String,
    icon_roots: Arc<Vec<PathBuf>>,
}

impl CachedIconResolver {
    pub fn new() -> Self {
        let theme = freedesktop_icons::default_theme_gtk().unwrap_or_else(|| "hicolor".to_string());
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            byte_cache: Arc::new(Mutex::new(HashMap::new())),
            theme,
            icon_roots: Arc::new(compute_icon_roots()),
        }
    }

    /// Skips GTK theme detection so tests are deterministic regardless of the
    /// machine's actual desktop settings.
    #[cfg(test)]
    pub fn new_with_theme(theme: String) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            byte_cache: Arc::new(Mutex::new(HashMap::new())),
            theme,
            icon_roots: Arc::new(Vec::new()),
        }
    }

    /// Skips live root computation so tests can inject a fixture root list,
    /// following the `new_with_theme` pattern above.
    #[cfg(test)]
    pub fn new_with_theme_and_roots(theme: String, roots: Vec<PathBuf>) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            byte_cache: Arc::new(Mutex::new(HashMap::new())),
            theme,
            icon_roots: Arc::new(roots),
        }
    }

    pub fn icon_roots(&self) -> &[PathBuf] {
        &self.icon_roots
    }

    /// Serves raw icon bytes for the future protocol handler. `requested_path`
    /// may arrive raw from a webview URI request, so it is canonicalized here
    /// rather than trusted -- callers must not pre-canonicalize and expect that
    /// to be sufficient. Cached on the canonicalized path so a file deleted
    /// after a first successful read still serves from memory (same staleness
    /// tradeoff as the `resolve_icon` path cache, see `test_caching_behavior`).
    pub fn serve_icon_bytes(&self, requested_path: &str) -> Result<Arc<Vec<u8>>, IconAccessError> {
        let requested = Path::new(requested_path);

        // `fs::canonicalize` requires every component, including the leaf, to
        // exist -- so a request for an icon that has since been deleted would
        // otherwise be indistinguishable from one that was never in an
        // allowed root. Fall back to canonicalizing the parent directory and
        // rejoining the file name purely for the containment check; the
        // `fs::read` below still fails on the missing file, surfacing
        // `NotFound` instead of misreporting it as `OutsideAllowedRoots`.
        let canonical_path = match std::fs::canonicalize(requested) {
            Ok(p) => p,
            Err(_) => {
                let parent = requested
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty());
                let file_name = requested.file_name();
                let (parent, file_name) = match (parent, file_name) {
                    (Some(parent), Some(file_name)) => (parent, file_name),
                    _ => return Err(IconAccessError::OutsideAllowedRoots),
                };
                let canonical_parent = std::fs::canonicalize(parent)
                    .map_err(|_| IconAccessError::OutsideAllowedRoots)?;
                canonical_parent.join(file_name)
            }
        };

        if !path_has_root_prefix(&canonical_path, &self.icon_roots) {
            return Err(IconAccessError::OutsideAllowedRoots);
        }

        if let Ok(guard) = self.byte_cache.lock() {
            if let Some(cached) = guard.get(&canonical_path) {
                return Ok(cached.clone());
            }
        }

        let bytes = std::fs::read(&canonical_path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                IconAccessError::NotFound
            } else {
                IconAccessError::ReadFailed(err.to_string())
            }
        })?;

        let bytes = Arc::new(bytes);
        if let Ok(mut guard) = self.byte_cache.lock() {
            guard.insert(canonical_path, bytes.clone());
        }

        Ok(bytes)
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

                let index_theme_contents = "[Icon Theme]\n\
                     Name=ThemeName\n\
                     Comment=Test fixture theme\n\
                     \n\
                     [48x48/apps]\n\
                     Size=48\n\
                     Type=Fixed\n\
                     \n\
                     [16x16/apps]\n\
                     Size=16\n\
                     Type=Fixed\n";
                std::fs::write(theme_dir.join("index.theme"), index_theme_contents).unwrap();

                File::create(dir_48.join("test-themed-icon.png")).unwrap();
                File::create(dir_48.join("cache-key-icon.png")).unwrap();
                File::create(dir_16.join("cache-key-icon.png")).unwrap();

                // A second copy of the theme, shaped like a flatpak export root
                // (<data-dir>/flatpak/exports/share/icons/<theme>/...), added to
                // XDG_DATA_DIRS below so `freedesktop_icons::lookup` can find it.
                // This lets tests prove a real lookup hit under that shape passes
                // `is_within_roots` -- the exact bug (flatpak/snap icons resolved
                // but then rejected) this feature exists to fix.
                let flatpak_share_dir = dir.path().join("flatpak/exports/share");
                let flatpak_theme_dir = flatpak_share_dir.join("icons").join("ThemeName");
                let flatpak_dir_48 = flatpak_theme_dir.join("48x48/apps");
                std::fs::create_dir_all(&flatpak_dir_48).unwrap();
                std::fs::write(flatpak_theme_dir.join("index.theme"), index_theme_contents)
                    .unwrap();
                File::create(flatpak_dir_48.join("flatpak-icon.png")).unwrap();

                std::env::set_var("XDG_DATA_HOME", dir.path());
                std::env::set_var(
                    "XDG_DATA_DIRS",
                    format!("{}:{}", dir.path().display(), flatpak_share_dir.display()),
                );

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

    #[test]
    fn test_is_within_roots_rejects_path_outside_roots() {
        let roots_dir = tempdir().unwrap();
        let allowed_root = roots_dir.path().join("icons");
        std::fs::create_dir_all(&allowed_root).unwrap();

        let home_dir = tempdir().unwrap();
        let outside_file = home_dir.path().join("secret.png");
        File::create(&outside_file).unwrap();

        let roots = vec![allowed_root.clone()];
        assert!(!is_within_roots(&outside_file, &roots));

        // A sibling directory whose name is a raw superstring of the allowed
        // root's name ("icons-evil" vs "icons") must also be rejected -- a
        // naive string-prefix check (rather than a canonicalized,
        // component-wise check) would incorrectly accept it.
        let sibling_dir = roots_dir.path().join("icons-evil");
        std::fs::create_dir_all(&sibling_dir).unwrap();
        let sibling_file = sibling_dir.join("secret.png");
        File::create(&sibling_file).unwrap();

        let roots = vec![allowed_root];
        assert!(!is_within_roots(&sibling_file, &roots));
    }

    #[test]
    fn test_is_within_roots_rejects_symlink_escape() {
        let roots_dir = tempdir().unwrap();
        let allowed_root = roots_dir.path().join("icons");
        std::fs::create_dir_all(&allowed_root).unwrap();

        let home_dir = tempdir().unwrap();
        let outside_file = home_dir.path().join("secret.png");
        File::create(&outside_file).unwrap();

        let escape_link = allowed_root.join("escape-link.png");
        std::os::unix::fs::symlink(&outside_file, &escape_link).unwrap();

        let roots = vec![allowed_root];
        assert!(!is_within_roots(&escape_link, &roots));
    }

    #[test]
    fn test_is_within_roots_accepts_each_root_kind() {
        let base = tempdir().unwrap();

        let home_icons = base.path().join("home/.icons");
        let usr_share_icons = base.path().join("usr/share/icons");
        let flatpak_icons = base.path().join("flatpak/exports/share/icons");
        let snap_icons = base.path().join("snap/desktop/icons");

        for root in [&home_icons, &usr_share_icons, &flatpak_icons, &snap_icons] {
            std::fs::create_dir_all(root).unwrap();
        }

        let home_icon_file = home_icons.join("app.png");
        let usr_share_icon_file = usr_share_icons.join("app.png");
        let flatpak_icon_file = flatpak_icons.join("app.png");
        let snap_icon_file = snap_icons.join("app.png");

        for file in [
            &home_icon_file,
            &usr_share_icon_file,
            &flatpak_icon_file,
            &snap_icon_file,
        ] {
            File::create(file).unwrap();
        }

        let roots = vec![
            home_icons.clone(),
            usr_share_icons.clone(),
            flatpak_icons.clone(),
            snap_icons.clone(),
        ];

        assert!(is_within_roots(&home_icon_file, &roots));
        assert!(is_within_roots(&usr_share_icon_file, &roots));
        assert!(is_within_roots(&flatpak_icon_file, &roots));
        assert!(is_within_roots(&snap_icon_file, &roots));
    }

    /// Round-trip proof for AC5: a real `freedesktop_icons::lookup` hit for an
    /// icon that only exists under a flatpak-exports-shaped directory must pass
    /// the same `is_within_roots` containment check the protocol handler uses,
    /// not just a synthetic path fed directly to `is_within_roots`.
    #[test]
    fn test_flatpak_lookup_result_passes_validation() {
        let fixture_dir = test_theme_fixture_dir();
        let flatpak_icons_root = fixture_dir.join("flatpak/exports/share/icons");

        let resolver = CachedIconResolver::new_with_theme_and_roots(
            "ThemeName".to_string(),
            vec![flatpak_icons_root],
        );

        let result = resolver.resolve_icon("flatpak-icon", 48, 1);

        assert!(result.is_some());
        let result_path = result.unwrap();
        assert!(result_path.contains("flatpak/exports/share/icons"));
        assert!(is_within_roots(
            Path::new(&result_path),
            resolver.icon_roots()
        ));
    }

    #[test]
    fn test_serve_icon_bytes_caches_after_deletion() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_icon_bytes.png");
        std::fs::write(&file_path, b"fake-png-bytes").unwrap();

        let resolver = CachedIconResolver::new_with_theme_and_roots(
            "hicolor".to_string(),
            vec![dir.path().to_path_buf()],
        );
        let path_str = file_path.to_str().unwrap();

        let first = resolver.serve_icon_bytes(path_str).unwrap();
        assert_eq!(first.as_slice(), b"fake-png-bytes");

        std::fs::remove_file(&file_path).unwrap();

        // Second call should still succeed with the same bytes, served from
        // byte_cache rather than the (now-deleted) filesystem entry.
        let second = resolver.serve_icon_bytes(path_str).unwrap();
        assert_eq!(second.as_slice(), b"fake-png-bytes");
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn test_serve_icon_bytes_missing_file_returns_err_not_panic() {
        let dir = tempdir().unwrap();
        let missing_path = dir.path().join("does-not-exist.png");

        let resolver = CachedIconResolver::new_with_theme_and_roots(
            "hicolor".to_string(),
            vec![dir.path().to_path_buf()],
        );

        let result = resolver.serve_icon_bytes(missing_path.to_str().unwrap());
        assert_eq!(result, Err(IconAccessError::NotFound));
    }

    #[test]
    fn test_serve_icon_bytes_rejects_path_outside_roots() {
        let roots_dir = tempdir().unwrap();
        let allowed_root = roots_dir.path().join("icons");
        std::fs::create_dir_all(&allowed_root).unwrap();

        let outside_dir = tempdir().unwrap();
        let outside_file = outside_dir.path().join("secret.png");
        std::fs::write(&outside_file, b"secret-bytes").unwrap();

        let resolver = CachedIconResolver::new_with_theme_and_roots(
            "hicolor".to_string(),
            vec![allowed_root],
        );

        let result = resolver.serve_icon_bytes(outside_file.to_str().unwrap());
        assert_eq!(result, Err(IconAccessError::OutsideAllowedRoots));
    }
}
