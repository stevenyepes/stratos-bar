use crate::domain::apps::AppEntry;
use crate::ports::app_port::AppRepository;
use crate::ports::icon_port::IconResolver;
use freedesktop_desktop_entry::Iter;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub struct FsAppRepository {
    icon_resolver: Arc<dyn IconResolver>,
}

impl FsAppRepository {
    pub fn new(icon_resolver: Arc<dyn IconResolver>) -> Self {
        Self { icon_resolver }
    }

    fn parse_desktop_file(&self, path: &std::path::Path) -> Option<AppEntry> {
        let content = std::fs::read_to_string(path).ok()?;
        let mut in_desktop_entry = false;
        let mut name = None;
        let mut exec = None;
        let mut icon = None;
        let mut no_display = false;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                in_desktop_entry = line == "[Desktop Entry]";
                continue;
            }

            if in_desktop_entry {
                if let Some(val) = line.strip_prefix("Name=") {
                    if name.is_none() {
                        name = Some(val.to_string());
                    }
                } else if let Some(val) = line.strip_prefix("Exec=") {
                    if exec.is_none() {
                        // Cleanup exec command (remove field codes)
                        let clean_exec = val
                            .replace("%f", "")
                            .replace("%F", "")
                            .replace("%u", "")
                            .replace("%U", "")
                            .replace("%i", "")
                            .replace("%c", "")
                            .replace("%k", "")
                            .trim()
                            .to_string();
                        exec = Some(clean_exec);
                    }
                } else if let Some(val) = line.strip_prefix("Icon=") {
                    if icon.is_none() {
                        icon = Some(val.to_string());
                    }
                } else if let Some(val) = line.strip_prefix("NoDisplay=") {
                    if val.to_lowercase() == "true" {
                        no_display = true;
                    }
                }
            }
        }

        if !no_display {
            if let (Some(name), Some(exec)) = (name, exec) {
                // Check if icon resolves
                // If icon is None, maybe we can try to guess from name or use default?
                // lib.rs logic: icon.and_then(|i| resolve_icon(&i))
                let icon_path = if let Some(i) = icon {
                    self.icon_resolver.resolve_icon(&i)
                } else {
                    None
                };

                return Some(AppEntry {
                    name,
                    exec,
                    icon: icon_path.filter(|i| !i.is_empty()),
                });
            }
        }
        None
    }
}

impl AppRepository for FsAppRepository {
    fn list_apps(&self) -> Result<Vec<AppEntry>, String> {
        let mut apps = Vec::new();
        let mut seen_ids = HashSet::new();

        // Construct search paths
        let mut search_paths = Vec::new();
        if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
            for dir in dirs.split(':') {
                search_paths.push(PathBuf::from(dir).join("applications"));
            }
        } else {
            search_paths.push(PathBuf::from("/usr/share/applications"));
            search_paths.push(PathBuf::from("/usr/local/share/applications"));
        }
        if let Some(home) = dirs::data_local_dir() {
            search_paths.push(home.join("applications"));
        }

        // Iterate over paths using the crate's iterator
        for path in Iter::new(search_paths.clone().into_iter()) {
            if let Some(app) = self.parse_desktop_file(&path) {
                let id = app.name.clone();
                if !seen_ids.contains(&id) {
                    apps.push(app);
                    seen_ids.insert(id);
                }
            }
        }

        // 2. Scan Flatpak Exports explicitly if available
        let flatpak_paths = vec![
            PathBuf::from("/var/lib/flatpak/exports/share/applications"),
            dirs::data_local_dir()
                .map(|d| d.join("flatpak/exports/share/applications"))
                .unwrap_or_default(),
        ];

        for path in Iter::new(flatpak_paths.into_iter()) {
            if let Some(app) = self.parse_desktop_file(&path) {
                let id = app.name.clone();
                if !seen_ids.contains(&id) {
                    apps.push(app);
                    seen_ids.insert(id);
                }
            }
        }

        // 3. Scan AppImages
        if let Some(home) = dirs::home_dir() {
            let applications_dir = home.join("Applications");
            if applications_dir.exists() {
                let glob_pattern = applications_dir.join("*.AppImage");
                if let Ok(glob_paths) = glob::glob(&glob_pattern.to_string_lossy()) {
                    for entry in glob_paths.filter_map(|e| e.ok()) {
                        let name = entry
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        if name.is_empty() {
                            continue;
                        }
                        if seen_ids.contains(&name) {
                            continue;
                        }

                        let icon = self.icon_resolver.resolve_icon(&name).or_else(|| {
                            self.icon_resolver.resolve_icon("application-x-executable")
                        });

                        apps.push(AppEntry {
                            name: name.clone(),
                            exec: entry.to_string_lossy().to_string(),
                            icon,
                        });
                        seen_ids.insert(name);
                    }
                }
            }
        }

        Ok(apps)
    }
}
