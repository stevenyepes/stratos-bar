use crate::domain::apps::AppEntry;
use crate::ports::app_port::AppRepository;
use crate::state::AppState;
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use tauri::State;

pub fn list_apps_logic(repo: &dyn AppRepository) -> Result<Vec<AppEntry>, String> {
    repo.list_apps()
}

#[tauri::command]
pub async fn list_apps(state: State<'_, AppState>) -> Result<Vec<AppEntry>, String> {
    list_apps_logic(&*state.app_repository)
}

pub fn rescan_apps_logic(repo: &dyn AppRepository) -> Result<Vec<AppEntry>, String> {
    repo.rescan()
}

#[tauri::command]
pub async fn rescan_apps(state: State<'_, AppState>) -> Result<Vec<AppEntry>, String> {
    rescan_apps_logic(&*state.app_repository)
}

#[tauri::command]
pub async fn set_custom_app_dirs(
    state: State<'_, AppState>,
    paths: Vec<PathBuf>,
) -> Result<(), String> {
    state.app_repository.set_custom_paths(paths);
    Ok(())
}

pub fn parse_exec_command(exec_cmd: &str) -> Option<(String, Vec<String>)> {
    let cleaned = exec_cmd
        .replace("%f", "")
        .replace("%F", "")
        .replace("%u", "")
        .replace("%U", "")
        .replace("%i", "")
        .replace("%c", "")
        .replace("%k", "")
        .replace("%d", "")
        .replace("%D", "")
        .replace("%n", "")
        .replace("%N", "")
        .replace("%v", "")
        .replace("%m", "")
        .replace("%M", "");

    let parts = shell_words::split(&cleaned).ok()?;
    if parts.is_empty() {
        return None;
    }

    let cmd = parts[0].clone();
    let args = parts[1..].to_vec();
    Some((cmd, args))
}

pub fn launch_app_logic(exec_cmd: &str, env: &HashMap<String, String>) -> Result<(), String> {
    let (cmd, args) = parse_exec_command(exec_cmd).ok_or_else(|| "Empty command".to_string())?;

    if !cmd.contains('/') && which::which(&cmd).is_err() {
        return Err(format!("Command not found in PATH: {cmd}"));
    }

    std::process::Command::new(cmd)
        .args(args)
        .env_clear()
        .envs(env)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn launch_app(state: State<'_, AppState>, exec_cmd: String) -> Result<(), String> {
    launch_app_logic(&exec_cmd, &state.env_port.snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::apps::AppSource;
    use crate::ports::app_port::MockAppRepository;

    #[test]
    fn test_list_apps() {
        let mut mock = MockAppRepository::new();
        mock.expect_list_apps().times(1).returning(|| {
            Ok(vec![AppEntry {
                id: "test".to_string(),
                name: "Test App".to_string(),
                generic_name: None,
                description: None,
                keywords: Vec::new(),
                exec: "test".to_string(),
                try_exec: None,
                icon: None,
                categories: Vec::new(),
                startup_wm_class: None,
                source: AppSource::Desktop,
                path: "/tmp/test.desktop".to_string(),
            }])
        });

        let result = list_apps_logic(&mock);
        assert!(result.is_ok());
        let apps = result.unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Test App");
    }

    #[test]
    fn test_rescan_apps() {
        let mut mock = MockAppRepository::new();
        mock.expect_rescan().times(1).returning(|| Ok(vec![]));
        let result = rescan_apps_logic(&mock);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_exec_command() {
        assert_eq!(
            parse_exec_command("firefox"),
            Some(("firefox".to_string(), vec![]))
        );
        assert_eq!(
            parse_exec_command("echo hello world"),
            Some((
                "echo".to_string(),
                vec!["hello".to_string(), "world".to_string()]
            ))
        );
        assert_eq!(
            parse_exec_command("vlc %U"),
            Some(("vlc".to_string(), vec![]))
        );
        assert_eq!(
            parse_exec_command("grep \"hello world\" file.txt"),
            Some((
                "grep".to_string(),
                vec!["hello world".to_string(), "file.txt".to_string()]
            ))
        );
        assert_eq!(parse_exec_command(""), None);
    }

    /// Reproduces the GPU regression: main.rs sets WEBKIT_DISABLE_DMABUF_RENDERER on
    /// stratos-bar's own process to work around a WebKitGTK black-window bug, and
    /// launch_app spawns children via std::process::Command with no environment of its
    /// own, so they inherit that var. This test simulates the poisoned parent process
    /// and asserts a child launched through launch_app does NOT see the var. It fails
    /// today because launch_app has no mechanism to build the child's environment from
    /// anything other than live inheritance.
    #[tokio::test]
    async fn test_launch_app_inherits_poisoned_environment() {
        let marker = std::env::temp_dir().join(format!(
            "stratos_bar_repro_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&marker);

        // Simulate main.rs's pre-mutation snapshot, captured before the workaround below.
        let snapshot: HashMap<String, String> = std::env::vars().collect();

        // Simulate the workaround main.rs applies to its own process before doing
        // anything else.
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

        let exec_cmd = format!(
            "sh -c 'if [ -z \"$WEBKIT_DISABLE_DMABUF_RENDERER\" ]; then touch {}; fi'",
            marker.display()
        );

        let result = launch_app_logic(&exec_cmd, &snapshot);

        std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER");

        assert!(result.is_ok(), "launch_app failed: {:?}", result.err());

        // launch_app spawns fire-and-forget, so poll briefly for the child to finish.
        let mut waited = std::time::Duration::from_millis(0);
        let step = std::time::Duration::from_millis(50);
        let timeout = std::time::Duration::from_secs(2);
        while !marker.exists() && waited < timeout {
            std::thread::sleep(step);
            waited += step;
        }

        let inherited = !marker.exists();
        let _ = std::fs::remove_file(&marker);

        assert!(
            !inherited,
            "child spawned by launch_app inherited WEBKIT_DISABLE_DMABUF_RENDERER from the \
             poisoned parent environment; launch_app must build the child's environment from \
             a snapshot captured before main.rs mutates it, not from live inheritance"
        );
    }
}
