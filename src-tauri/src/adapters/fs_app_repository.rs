use crate::domain::apps::{AppEntry, AppSource};
use crate::ports::app_port::AppRepository;
use crate::ports::icon_port::IconResolver;
use freedesktop_desktop_entry::{default_paths, get_languages_from_env, Iter, PathSource};
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const DEBOUNCE_WINDOW_MS: u64 = 500;

pub struct FsAppRepository {
    icon_resolver: Arc<dyn IconResolver>,
    custom_paths: Mutex<Vec<PathBuf>>,
    cache: Mutex<Vec<AppEntry>>,
    app_handle: Option<AppHandle>,
    watcher: Mutex<Option<Debouncer<notify::RecommendedWatcher, RecommendedCache>>>,
    use_default_paths: bool,
}

impl FsAppRepository {
    pub fn new(icon_resolver: Arc<dyn IconResolver>) -> Self {
        Self {
            icon_resolver,
            custom_paths: Mutex::new(Vec::new()),
            cache: Mutex::new(Vec::new()),
            app_handle: None,
            watcher: Mutex::new(None),
            use_default_paths: true,
        }
    }

    pub fn new_with_handle(icon_resolver: Arc<dyn IconResolver>, app_handle: AppHandle) -> Self {
        Self {
            icon_resolver,
            custom_paths: Mutex::new(Vec::new()),
            cache: Mutex::new(Vec::new()),
            app_handle: Some(app_handle),
            watcher: Mutex::new(None),
            use_default_paths: true,
        }
    }

    /// Kick off the warm-up scan and filesystem watcher.
    ///
    /// Must be called *after* `AppState` is `manage()`d, because the spawned
    /// scan and the watcher callback both look the state up via the
    /// `AppHandle`. Calling this from inside the constructor races `manage()`
    /// and panics with `state() called before manage()`.
    pub fn start_background_tasks(&self) {
        self.spawn_initial_scan();
        self.start_watcher();
    }

    #[cfg(test)]
    pub fn new_with_paths(icon_resolver: Arc<dyn IconResolver>, paths: Vec<PathBuf>) -> Self {
        Self {
            icon_resolver,
            custom_paths: Mutex::new(paths),
            cache: Mutex::new(Vec::new()),
            app_handle: None,
            watcher: Mutex::new(None),
            use_default_paths: false,
        }
    }

    fn spawn_initial_scan(&self) {
        let handle = match self.app_handle.clone() {
            Some(h) => h,
            None => return,
        };
        tauri::async_runtime::spawn_blocking(move || {
            let state = handle.state::<crate::state::AppState>();
            let apps = state.app_repository.list_apps();
            if let Ok(apps) = apps {
                if !apps.is_empty() {
                    let _ = handle.emit("apps-updated", &apps);
                }
            }
        });
    }

    fn start_watcher(&self) {
        let handle = match self.app_handle.clone() {
            Some(h) => h,
            None => return,
        };
        let paths = self.collect_search_paths();
        if paths.is_empty() {
            return;
        }

        let mut debouncer = match new_debouncer(
            Duration::from_millis(DEBOUNCE_WINDOW_MS),
            None,
            move |result: DebounceEventResult| {
                if let Ok(events) = result {
                    if !events.is_empty() {
                        let state = handle.state::<crate::state::AppState>();
                        let _ = state.app_repository.rescan();
                    }
                }
            },
        ) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[stratos-bar] failed to create apps debouncer: {e}");
                return;
            }
        };

        for path in &paths {
            if path.exists() {
                if let Err(e) = debouncer.watch(path, RecursiveMode::Recursive) {
                    eprintln!("[stratos-bar] watch {} failed: {e}", path.display());
                }
            }
        }

        if let Some(home) = dirs::home_dir() {
            let apps_dir = home.join("Applications");
            if apps_dir.exists() {
                let _ = debouncer.watch(&apps_dir, RecursiveMode::NonRecursive);
            }
        }

        if let Ok(mut slot) = self.watcher.lock() {
            *slot = Some(debouncer);
        }
    }

    fn collect_search_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = if self.use_default_paths {
            default_paths().collect()
        } else {
            Vec::new()
        };

        if self.use_default_paths {
            if let Some(local) = dirs::data_local_dir() {
                let p = local.join("flatpak/exports/share/applications");
                if p.exists() {
                    paths.push(p);
                }
            }
            let system_flatpak = PathBuf::from("/var/lib/flatpak/exports/share/applications");
            if system_flatpak.exists() {
                paths.push(system_flatpak);
            }

            for snap_dir in [
                "/var/lib/snapd/desktop/applications",
                "/snap/desktop/applications",
            ] {
                let p = PathBuf::from(snap_dir);
                if p.exists() {
                    paths.push(p);
                }
            }
        }

        if let Ok(custom) = self.custom_paths.lock() {
            for p in custom.iter() {
                if p.exists() {
                    paths.push(p.clone());
                }
            }
        }

        paths
    }

    fn scan(&self) -> Result<Vec<AppEntry>, String> {
        let locales = get_languages_from_env();
        let search_paths = self.collect_search_paths();
        let mut by_id: HashMap<String, AppEntry> = HashMap::new();

        for entry in Iter::new(search_paths.into_iter()).entries(Some(&locales)) {
            if let Some(app) = self.parse_entry(&entry, PathSource::guess_from(&entry.path)) {
                by_id.entry(app.id.clone()).or_insert(app);
            }
        }

        if let Some(home) = dirs::home_dir() {
            self.scan_appimages(&home.join("Applications"), &mut by_id);
        }

        let mut apps: Vec<AppEntry> = by_id.into_values().collect();
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(apps)
    }

    fn parse_entry(
        &self,
        entry: &freedesktop_desktop_entry::DesktopEntry,
        path_src: PathSource,
    ) -> Option<AppEntry> {
        let locales: &[String] = &[];
        if entry.type_().unwrap_or("Application") != "Application" {
            return None;
        }
        if entry.no_display() || entry.hidden() {
            return None;
        }

        if let Ok(current) = std::env::var("XDG_CURRENT_DESKTOP") {
            let des: Vec<&str> = current.split(':').collect();
            if let Some(only) = entry.only_show_in() {
                if !des.iter().any(|de| only.contains(de)) {
                    return None;
                }
            }
            if let Some(not) = entry.not_show_in() {
                if des.iter().any(|de| not.contains(de)) {
                    return None;
                }
            }
        }

        let name = entry.name(locales)?.into_owned();
        let exec_raw = entry.exec()?.to_string();
        if let Some(try_exec) = entry.try_exec() {
            if !try_exec_path_ok(try_exec) {
                return None;
            }
        }
        let exec = clean_exec(&exec_raw);

        let id = entry
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| entry.id().to_string());

        let icon = entry
            .icon()
            .and_then(|i| self.icon_resolver.resolve_icon(i))
            .filter(|s| !s.is_empty());

        let source = path_src_to_source(&path_src);

        Some(AppEntry {
            id,
            name,
            generic_name: entry.generic_name(locales).map(|s| s.into_owned()),
            description: entry.comment(locales).map(|s| s.into_owned()),
            keywords: entry
                .keywords(locales)
                .map(|v| v.into_iter().map(|s| s.into_owned()).collect())
                .unwrap_or_default(),
            exec,
            try_exec: entry.try_exec().map(|s| s.to_string()),
            icon,
            categories: entry
                .categories()
                .map(|v| v.into_iter().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            startup_wm_class: entry.startup_wm_class().map(|s| s.to_string()),
            source,
            path: entry.path.to_string_lossy().to_string(),
        })
    }

    fn scan_appimages(
        &self,
        applications_dir: &std::path::Path,
        by_id: &mut HashMap<String, AppEntry>,
    ) {
        if !applications_dir.exists() {
            return;
        }
        let entries = match std::fs::read_dir(applications_dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e,
                None => continue,
            };
            if !ext.eq_ignore_ascii_case("appimage") {
                continue;
            }
            let name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => continue,
            };
            let id = format!("appimage:{}", name);
            if by_id.contains_key(&id) {
                continue;
            }
            let icon = self
                .icon_resolver
                .resolve_icon(&name)
                .or_else(|| self.icon_resolver.resolve_icon("application-x-executable"))
                .filter(|s| !s.is_empty());

            by_id.insert(
                id.clone(),
                AppEntry {
                    id,
                    name,
                    generic_name: None,
                    description: None,
                    keywords: Vec::new(),
                    exec: path.to_string_lossy().to_string(),
                    try_exec: None,
                    icon,
                    categories: Vec::new(),
                    startup_wm_class: None,
                    source: AppSource::AppImage,
                    path: path.to_string_lossy().to_string(),
                },
            );
        }
    }
}

impl AppRepository for FsAppRepository {
    fn list_apps(&self) -> Result<Vec<AppEntry>, String> {
        if let Ok(cache) = self.cache.lock() {
            if !cache.is_empty() {
                return Ok(cache.clone());
            }
        }
        let apps = self.scan()?;
        if let Ok(mut cache) = self.cache.lock() {
            *cache = apps.clone();
        }
        Ok(apps)
    }

    fn rescan(&self) -> Result<Vec<AppEntry>, String> {
        let apps = self.scan()?;
        if let Ok(mut cache) = self.cache.lock() {
            *cache = apps.clone();
        }
        if let Some(handle) = &self.app_handle {
            let _ = handle.emit("apps-updated", &apps);
        }
        Ok(apps)
    }

    fn set_custom_paths(&self, paths: Vec<PathBuf>) {
        if let Ok(mut slot) = self.custom_paths.lock() {
            *slot = paths;
        }
        // Drop the existing watcher and start a new one with the new paths.
        if let Ok(mut slot) = self.watcher.lock() {
            *slot = None;
        }
        self.start_watcher();
        let _ = self.rescan();
    }
}

fn path_src_to_source(src: &PathSource) -> AppSource {
    match src {
        PathSource::SystemFlatpak | PathSource::LocalFlatpak => AppSource::Flatpak,
        PathSource::SystemSnap => AppSource::Snap,
        PathSource::Nix | PathSource::LocalNix => AppSource::Nix,
        _ => AppSource::Desktop,
    }
}

fn try_exec_path_ok(try_exec: &str) -> bool {
    if try_exec.is_empty() {
        return true;
    }
    let p = std::path::Path::new(try_exec);
    if p.is_absolute() {
        p.exists()
    } else {
        which::which(try_exec).is_ok()
    }
}

fn clean_exec(raw: &str) -> String {
    let mut s = raw.to_string();
    for code in [
        "%f", "%F", "%u", "%U", "%i", "%c", "%k", "%d", "%D", "%n", "%N", "%v", "%m", "%M",
    ] {
        s = s.replace(code, "");
    }
    if let Some(start) = s.find("--file-forwarding") {
        s.truncate(start);
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::icon_port::IconResolver;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    struct MockIconResolver;
    impl IconResolver for MockIconResolver {
        fn resolve_icon(&self, _icon_name: &str) -> Option<String> {
            Some("/tmp/icon.png".to_string())
        }
    }

    fn write_desktop(dir: &std::path::Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(format!("{name}.desktop"));
        let mut f = File::create(&path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_list_apps_finds_desktop_entry() {
        let dir = tempdir().unwrap();
        let apps_dir = dir.path().join("applications");
        std::fs::create_dir(&apps_dir).unwrap();
        write_desktop(
            &apps_dir,
            "test-app",
            "[Desktop Entry]\nName=Test App\nExec=test-exec %f\nIcon=test-icon\nType=Application\n",
        );

        let resolver = Arc::new(MockIconResolver);
        let repo = FsAppRepository::new_with_paths(resolver, vec![apps_dir]);

        let apps = repo.list_apps().unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Test App");
        assert_eq!(apps[0].exec, "test-exec");
        assert_eq!(apps[0].icon, Some("/tmp/icon.png".to_string()));
        assert_eq!(apps[0].source, AppSource::Desktop);
        assert!(apps[0].categories.is_empty());
    }

    #[test]
    fn test_list_apps_ignores_no_display() {
        let dir = tempdir().unwrap();
        let apps_dir = dir.path().join("applications");
        std::fs::create_dir(&apps_dir).unwrap();
        write_desktop(
            &apps_dir,
            "hidden-app",
            "[Desktop Entry]\nName=Hidden App\nExec=hidden\nNoDisplay=true\nType=Application\n",
        );

        let resolver = Arc::new(MockIconResolver);
        let repo = FsAppRepository::new_with_paths(resolver, vec![apps_dir]);
        let apps = repo.list_apps().unwrap();
        assert!(apps.is_empty());
    }

    #[test]
    fn test_list_apps_ignores_hidden() {
        let dir = tempdir().unwrap();
        let apps_dir = dir.path().join("applications");
        std::fs::create_dir(&apps_dir).unwrap();
        write_desktop(
            &apps_dir,
            "secret",
            "[Desktop Entry]\nName=Secret\nExec=secret\nHidden=true\nType=Application\n",
        );
        let repo = FsAppRepository::new_with_paths(Arc::new(MockIconResolver), vec![apps_dir]);
        assert!(repo.list_apps().unwrap().is_empty());
    }

    #[test]
    fn test_list_apps_filters_non_application_type() {
        let dir = tempdir().unwrap();
        let apps_dir = dir.path().join("applications");
        std::fs::create_dir(&apps_dir).unwrap();
        write_desktop(
            &apps_dir,
            "link",
            "[Desktop Entry]\nName=Link\nExec=ignored\nType=Link\nURL=https://example.com\n",
        );
        let repo = FsAppRepository::new_with_paths(Arc::new(MockIconResolver), vec![apps_dir]);
        assert!(repo.list_apps().unwrap().is_empty());
    }

    #[test]
    fn test_clean_exec_strips_all_field_codes() {
        assert_eq!(clean_exec("firefox %u"), "firefox");
        assert_eq!(clean_exec("vlc %F"), "vlc");
        assert_eq!(clean_exec("a %d %D %n %N %v %m %M"), "a");
    }

    #[test]
    fn test_clean_exec_strips_flatpak_file_forwarding() {
        let raw = "/usr/bin/flatpak run --command=imhex --file-forwarding net.app @@u %U @@";
        let out = clean_exec(raw);
        assert!(!out.contains("--file-forwarding"));
        assert!(!out.contains("@@"));
        assert!(!out.contains("%U"));
        assert!(out.contains("/usr/bin/flatpak"));
        assert!(out.contains("--command=imhex"));
    }

    #[test]
    fn test_appimage_case_insensitive() {
        let dir = tempdir().unwrap();
        let apps_dir = dir.path().join("Applications");
        std::fs::create_dir(&apps_dir).unwrap();
        std::fs::write(apps_dir.join("Foo.Appimage"), b"#!/bin/sh\n").unwrap();
        std::fs::write(apps_dir.join("Bar.appimage"), b"#!/bin/sh\n").unwrap();
        std::fs::write(apps_dir.join("Baz.APPIMAGE"), b"#!/bin/sh\n").unwrap();

        let resolver = Arc::new(MockIconResolver);
        let repo = FsAppRepository::new_with_paths(resolver, vec![apps_dir.clone()]);
        // We pass Applications dir as a custom path, so it gets scanned.
        // But scan_appimages checks the default ~/Applications, not custom paths.
        // So we need to call it via a manual test. Let's just call scan_appimages directly.
        let mut by_id = HashMap::new();
        repo.scan_appimages(&apps_dir, &mut by_id);
        let names: Vec<String> = by_id.values().map(|a| a.name.clone()).collect();
        assert!(names.contains(&"Foo".to_string()));
        assert!(names.contains(&"Bar".to_string()));
        assert!(names.contains(&"Baz".to_string()));
        for app in by_id.values() {
            assert_eq!(app.source, AppSource::AppImage);
        }
    }

    #[test]
    fn test_try_exec_filters_missing_binary() {
        let dir = tempdir().unwrap();
        let apps_dir = dir.path().join("applications");
        std::fs::create_dir(&apps_dir).unwrap();
        write_desktop(
            &apps_dir,
            "needs-missing",
            "[Desktop Entry]\nName=Needs Missing\nExec=echo hi\nTryExec=definitely-not-a-binary-12345\nType=Application\n",
        );
        let repo = FsAppRepository::new_with_paths(Arc::new(MockIconResolver), vec![apps_dir]);
        assert!(repo.list_apps().unwrap().is_empty());
    }
}
