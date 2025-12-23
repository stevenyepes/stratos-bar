use crate::domain::windows::WindowEntry;
use crate::ports::window_port::WindowService;
use std::env;
use std::process::Command;

// Reuse struct definitions from window_manager.rs, but private here or separate?
// We can define internal helpers here.

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
                WindowEntry {
                    title,
                    class,
                    address,
                    icon: None, // Will be resolved by service or caller
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
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 {
                let app_id = parts[0].trim().to_string();
                let title = parts[1].trim().to_string();
                windows.push(WindowEntry {
                    title: title.clone(),
                    class: app_id.clone(),
                    address: app_id,
                    icon: None,
                });
            }
        }
        Ok(windows)
    }

    fn focus_window(&self, id: &str) -> Result<(), String> {
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
                let class_full = parts[2].to_string();
                let title = parts[4..].join(" ");
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

pub struct LinuxWindowService;

impl LinuxWindowService {
    pub fn new() -> Self {
        Self
    }

    fn get_backend(&self) -> Box<dyn WindowBackend> {
        if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
            return Box::new(HyprlandBackend);
        }
        if env::var("WAYLAND_DISPLAY").is_ok() {
            return Box::new(WlrctlBackend);
        }
        Box::new(WmctrlBackend)
    }
}

impl WindowService for LinuxWindowService {
    fn list_windows(&self) -> Result<Vec<WindowEntry>, String> {
        self.get_backend().list_windows()
    }

    fn focus_window(&self, id: &str) -> Result<(), String> {
        self.get_backend().focus_window(id)
    }
}
