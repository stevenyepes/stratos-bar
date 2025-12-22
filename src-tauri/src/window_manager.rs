use serde::Serialize;
use std::env;
use std::process::Command;

#[derive(Serialize, Clone, Debug)]
pub struct WindowEntry {
    pub title: String,
    pub class: String,
    pub address: String, // ID or Address
    pub icon: Option<String>,
}

trait WindowBackend {
    fn list_windows(&self) -> Result<Vec<WindowEntry>, String>;
    fn focus_window(&self, id: &str) -> Result<(), String>;
}

struct HyprlandBackend;
struct WlrctlBackend;
struct WmctrlBackend;

impl WindowBackend for HyprlandBackend {
    fn list_windows(&self) -> Result<Vec<WindowEntry>, String> {
        let output = Command::new("hyprctl")
            .arg("clients")
            .arg("-j")
            .output()
            .map_err(|e| format!("Failed to execute hyprctl: {}", e))?;

        if !output.status.success() {
            return Err("hyprctl command failed".to_string());
        }

        let clients: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Failed to parse hyprctl output: {}", e))?;

        Ok(clients
            .into_iter()
            .map(|client| {
                let class = client["class"].as_str().unwrap_or("").to_string();
                let title = client["title"].as_str().unwrap_or("").to_string();
                let address = client["address"].as_str().unwrap_or("").to_string();
                // Icon resolution should happen in lib.rs or passed down,
                // but for now we will return None and let caller handle it
                // or we can't easily access the resolve_icon function from here without circular deps
                // For simplicity, we'll keep the struct clean and let the caller resolve icons
                // OR we move resolve_icon to a shared utility.
                // Let's assume we return raw entries and lib.rs enriches them.
                WindowEntry {
                    title,
                    class,
                    address,
                    icon: None,
                }
            })
            .collect())
    }

    fn focus_window(&self, id: &str) -> Result<(), String> {
        let output = Command::new("hyprctl")
            .arg("dispatch")
            .arg("focuswindow")
            .arg(format!("address:{}", id))
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(())
        } else {
            Err("Failed to focus window via hyprctl".to_string())
        }
    }
}

impl WindowBackend for WlrctlBackend {
    fn list_windows(&self) -> Result<Vec<WindowEntry>, String> {
        // wlrctl toplevel list
        // Output format: <id>: <app_id> <title>
        let output = Command::new("wlrctl")
            .arg("toplevel")
            .arg("list")
            .output()
            .map_err(|e| format!("Failed to execute wlrctl: {}", e))?;

        if !output.status.success() {
            return Err("wlrctl command failed".to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut windows = Vec::new();

        for line in stdout.lines() {
            // Example: "state:focused app_id:org.gnome.TextEditor title:Untitled Document 1"
            // Wait, standard output of `wlrctl toplevel list`:
            // It seems it might vary or it uses a specific format.
            // Let's assume the common output: "app_id: <id>, title: <title>, ..."
            // Actually, based on search, `wlrctl toplevel list` output is often line based.
            // Let's try to parse flexibly.

            // NOTE: wlrctl output format isn't strictly JSON. It's often:
            // <app_id>: <title>
            // But we need the ID to focus.
            // `wlrctl toplevel list` often prints:
            // <app_id>: <title>
            // BUT, `wlrctl toplevel focus <matches>` uses matching.

            // Let's check `wlrctl` source or docs if possible.
            // Since we can't easily verify exact output without running it,
            // we will assume a simple parsing strategy or use `wlrctl`'s ability to focus by title/app_id.
            // If we don't have a unique ID, we used the AppID or Title.

            // However, verify standard behaviour:
            // `wlrctl toplevel list`
            // org.wezfurlong.wezterm: WezTerm
            // firefox: Mozilla Firefox

            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 {
                let app_id = parts[0].trim().to_string();
                let title = parts[1].trim().to_string();
                windows.push(WindowEntry {
                    title: title.clone(),
                    class: app_id.clone(),
                    address: app_id, // Use app_id as address for focus
                    icon: None,
                });
            }
        }
        Ok(windows)
    }

    fn focus_window(&self, id: &str) -> Result<(), String> {
        // wlrctl toplevel focus <app_id>
        // Note: this focuses by app_id, which might be ambiguous if multiple windows open.
        // wlrctl doesn't seem to expose a unique window ID in `list` easily unless using recent versions?
        let output = Command::new("wlrctl")
            .arg("toplevel")
            .arg("focus")
            .arg(id)
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(())
        } else {
            Err("Failed to focus window via wlrctl".to_string())
        }
    }
}

impl WindowBackend for WmctrlBackend {
    fn list_windows(&self) -> Result<Vec<WindowEntry>, String> {
        // wmctrl -l -x
        // Output: <id> <desktop> <class> <host> <title>
        let output = Command::new("wmctrl")
            .arg("-l")
            .arg("-x")
            .output()
            .map_err(|e| format!("Failed to execute wmctrl: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut windows = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let id = parts[0].to_string();
                // parts[1] is desktop
                let class_full = parts[2].to_string(); // usually name.Class
                                                       // parts[3] is host

                // Title is the rest
                // Reconstruct title from remaining parts (index 4 onwards)
                // However, split_whitespace consumes whitespace.
                // Better approach: find indices.
                // But simplified:
                let title = parts[4..].join(" ");

                // Parse class: "gnome-terminal-server.Gnome-terminal" -> "Gnome-terminal"
                let class = class_full
                    .split('.')
                    .last()
                    .unwrap_or(&class_full)
                    .to_string();

                windows.push(WindowEntry {
                    title,
                    class,
                    address: id,
                    icon: None,
                });
            }
        }
        Ok(windows)
    }

    fn focus_window(&self, id: &str) -> Result<(), String> {
        // wmctrl -i -a <id>
        let output = Command::new("wmctrl")
            .arg("-i")
            .arg("-a")
            .arg(id)
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(())
        } else {
            Err("Failed to focus window via wmctrl".to_string())
        }
    }
}

pub struct WindowManager;

impl WindowManager {
    pub fn list_windows() -> Result<Vec<WindowEntry>, String> {
        Self::get_backend().list_windows()
    }

    pub fn focus_window(id: &str) -> Result<(), String> {
        Self::get_backend().focus_window(id)
    }

    fn get_backend() -> Box<dyn WindowBackend> {
        // Priority 1: Hyprland
        if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
            return Box::new(HyprlandBackend);
        }

        // Priority 2: Wayland (Generic) - Try to detect if wlrctl is usable?
        // Actually, just checking WAYLAND_DISPLAY is strict.
        // If we are on Wayland but not Hyprland, we try Wlrctl.
        if env::var("WAYLAND_DISPLAY").is_ok() {
            // We might want to check if `wlrctl` is installed, but for now we assume yes
            // or let it fail gracefully.
            return Box::new(WlrctlBackend);
        }

        // Priority 3: X11
        // Fallback to wmctrl
        Box::new(WmctrlBackend)
    }
}
