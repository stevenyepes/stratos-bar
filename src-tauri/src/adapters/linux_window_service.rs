use crate::domain::windows::WindowEntry;
use crate::ports::window_port::WindowService;
use std::env;
use std::process::Command;
use std::sync::Arc;

#[cfg_attr(test, mockall::automock)]
pub trait CommandExecutor: Send + Sync {
    fn execute(&self, cmd: &str, args: Vec<String>) -> Result<std::process::Output, String>;
}

pub struct StdCommandExecutor;

impl CommandExecutor for StdCommandExecutor {
    fn execute(&self, cmd: &str, args: Vec<String>) -> Result<std::process::Output, String> {
        Command::new(cmd)
            .args(args)
            .output()
            .map_err(|e| format!("Failed to execute {}: {}", cmd, e))
    }
}

trait WindowBackend {
    fn list_windows(&self, executor: &dyn CommandExecutor) -> Result<Vec<WindowEntry>, String>;
    fn focus_window(&self, executor: &dyn CommandExecutor, id: &str) -> Result<(), String>;
}

struct HyprlandBackend;
struct WlrctlBackend;
struct WmctrlBackend;

impl WindowBackend for HyprlandBackend {
    fn list_windows(&self, executor: &dyn CommandExecutor) -> Result<Vec<WindowEntry>, String> {
        let output = executor.execute("hyprctl", vec!["clients".to_string(), "-j".to_string()])?;

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

    fn focus_window(&self, executor: &dyn CommandExecutor, id: &str) -> Result<(), String> {
        let output = executor.execute(
            "hyprctl",
            vec![
                "dispatch".to_string(),
                "focuswindow".to_string(),
                format!("address:{}", id),
            ],
        )?;

        if output.status.success() {
            Ok(())
        } else {
            Err("Failed to focus window via hyprctl".to_string())
        }
    }
}

impl WindowBackend for WlrctlBackend {
    fn list_windows(&self, executor: &dyn CommandExecutor) -> Result<Vec<WindowEntry>, String> {
        let output =
            executor.execute("wlrctl", vec!["toplevel".to_string(), "list".to_string()])?;

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

    fn focus_window(&self, executor: &dyn CommandExecutor, id: &str) -> Result<(), String> {
        let output = executor.execute(
            "wlrctl",
            vec!["toplevel".to_string(), "focus".to_string(), id.to_string()],
        )?;

        if output.status.success() {
            Ok(())
        } else {
            Err("Failed to focus window via wlrctl".to_string())
        }
    }
}

impl WindowBackend for WmctrlBackend {
    fn list_windows(&self, executor: &dyn CommandExecutor) -> Result<Vec<WindowEntry>, String> {
        let output = executor.execute("wmctrl", vec!["-l".to_string(), "-x".to_string()])?;

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

    fn focus_window(&self, executor: &dyn CommandExecutor, id: &str) -> Result<(), String> {
        let output = executor.execute(
            "wmctrl",
            vec!["-i".to_string(), "-a".to_string(), id.to_string()],
        )?;

        if output.status.success() {
            Ok(())
        } else {
            Err("Failed to focus window via wmctrl".to_string())
        }
    }
}

pub struct LinuxWindowService {
    executor: Arc<dyn CommandExecutor>,
}

impl LinuxWindowService {
    pub fn new(executor: Arc<dyn CommandExecutor>) -> Self {
        Self { executor }
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
        self.get_backend().list_windows(self.executor.as_ref())
    }

    fn focus_window(&self, id: &str) -> Result<(), String> {
        self.get_backend().focus_window(self.executor.as_ref(), id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    fn mock_success_output(stdout: &str) -> std::process::Output {
        std::process::Output {
            status: ExitStatus::from_raw(0), // 0 means success in unix
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn test_hyprland_backend_list_windows() {
        let mut mock = MockCommandExecutor::new();
        mock.expect_execute()
            .with(
                mockall::predicate::eq("hyprctl"),
                mockall::predicate::eq(vec!["clients".to_string(), "-j".to_string()]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(mock_success_output(
                    r#"[
                {
                    "class": "org.mozilla.firefox",
                    "title": "Mozilla Firefox",
                    "address": "0x12345678"
                }
            ]"#,
                ))
            });

        let backend = HyprlandBackend;
        let windows = backend.list_windows(&mock).unwrap();

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].title, "Mozilla Firefox");
        assert_eq!(windows[0].class, "org.mozilla.firefox");
        assert_eq!(windows[0].address, "0x12345678");
    }

    #[test]
    fn test_wlrctl_backend_list_windows() {
        let mut mock = MockCommandExecutor::new();
        mock.expect_execute()
            .with(
                mockall::predicate::eq("wlrctl"),
                mockall::predicate::eq(vec!["toplevel".to_string(), "list".to_string()]),
            )
            .times(1)
            .returning(|_, _| Ok(mock_success_output("org.wezfurlong.wezterm: WezTerm\n")));

        let backend = WlrctlBackend;
        let windows = backend.list_windows(&mock).unwrap();

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].title, "WezTerm");
        assert_eq!(windows[0].class, "org.wezfurlong.wezterm");
    }

    #[test]
    fn test_wmctrl_backend_list_windows() {
        let mut mock = MockCommandExecutor::new();
        mock.expect_execute()
            .with(
                mockall::predicate::eq("wmctrl"),
                mockall::predicate::eq(vec!["-l".to_string(), "-x".to_string()]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(mock_success_output(
                    "0x02800003  0 pycharm.PyCharm  ubuntu PyCharm Projects\n",
                ))
            });

        let backend = WmctrlBackend;
        let windows = backend.list_windows(&mock).unwrap();

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].title, "PyCharm Projects");
        assert_eq!(windows[0].class, "PyCharm");
    }
}
