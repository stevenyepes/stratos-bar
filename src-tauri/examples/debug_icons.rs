use linicon::lookup_icon;
use std::path::{Path, PathBuf};

fn resolve_icon_manual(icon_name: &str) -> Option<String> {
    println!("Manual fallback check for: {}", icon_name);
    let mut search_paths = Vec::new();

    // Standard XDG paths
    if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in dirs.split(':') {
            let p = Path::new(dir);
            search_paths.push(p.join("icons/hicolor/48x48/apps"));
            search_paths.push(p.join("icons/hicolor/32x32/apps"));
            search_paths.push(p.join("icons/hicolor/scalable/apps"));
            search_paths.push(p.join("pixmaps"));
            search_paths.push(p.join("icons"));
        }
    }

    // User local paths
    if let Some(home) = dirs::data_local_dir() {
        search_paths.push(home.join("icons/hicolor/48x48/apps"));
        search_paths.push(home.join("icons/hicolor/32x32/apps"));
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

        // Steam specific cache check
        if base.ends_with("librarycache") && icon_name.starts_with("steam_icon_") {
            let app_id = icon_name.trim_start_matches("steam_icon_");
            let p = base.join(format!("{}_icon.jpg", app_id));
            if p.exists() {
                println!("  Found at: {:?}", p);
                return Some(p.to_string_lossy().to_string());
            }
        }

        for ext in &extensions {
            let p = base.join(format!("{}.{}", icon_name, ext));
            if p.exists() {
                println!("  Found at: {:?}", p);
                return Some(p.to_string_lossy().to_string());
            }
        }
    }
    println!("  Not found in manual paths.");
    None
}

fn main() {
    let icons_to_check = vec![
        "nautilus",
        "org.gnome.Nautilus",
        "steam",
        "steam_icon_10",
        "steam_icon_generic",
    ];

    for icon in icons_to_check {
        println!("--- Checking icon: {} ---", icon);

        // 1. Linicon
        let mut linicon_found = false;
        let iter = lookup_icon(icon);
        for (i, result) in iter.enumerate() {
            match result {
                Ok(icon_path) => {
                    println!(
                        "  Linicon match #{}: {:?} ({:?}, {:?}-{:?})",
                        i,
                        icon_path.path,
                        icon_path.icon_type,
                        icon_path.min_size,
                        icon_path.max_size
                    );
                    linicon_found = true;
                }
                Err(e) => println!("  Linicon error: {}", e),
            }
            if i > 2 {
                break;
            } // limit output
        }
        if !linicon_found {
            println!("  Linicon returned no results.");
        }

        // 2. Manual Fallback
        resolve_icon_manual(icon);
    }
}
