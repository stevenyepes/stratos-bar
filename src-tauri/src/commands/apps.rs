use crate::domain::apps::AppEntry;
use crate::ports::app_port::AppRepository;
use crate::state::AppState;
use std::os::unix::process::CommandExt;
use tauri::State;

// Handler logic for testing
pub fn list_apps_logic(repo: &dyn AppRepository) -> Result<Vec<AppEntry>, String> {
    repo.list_apps()
}

#[tauri::command]
pub async fn list_apps(state: State<'_, AppState>) -> Result<Vec<AppEntry>, String> {
    list_apps_logic(&*state.app_repository)
}

// Helper to clean exec command - exposed for testing
pub fn parse_exec_command(exec_cmd: &str) -> Option<(String, Vec<String>)> {
    let cleaned = exec_cmd
        .replace("%f", "")
        .replace("%F", "")
        .replace("%u", "")
        .replace("%U", "")
        .replace("%i", "")
        .replace("%c", "")
        .replace("%k", "");

    let parts = shell_words::split(&cleaned).ok()?;
    if parts.is_empty() {
        return None;
    }

    let cmd = parts[0].clone();
    let args = parts[1..].to_vec();
    Some((cmd, args))
}

#[tauri::command]
pub async fn launch_app(exec_cmd: String) -> Result<(), String> {
    let (cmd, args) = parse_exec_command(&exec_cmd).ok_or_else(|| "Empty command".to_string())?;

    std::process::Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0) // Sets the process group ID to the child's PID effectively creating a new session
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::app_port::MockAppRepository;

    #[test]
    fn test_list_apps() {
        let mut mock = MockAppRepository::new();
        mock.expect_list_apps().times(1).returning(|| {
            Ok(vec![AppEntry {
                name: "Test App".to_string(),
                exec: "test".to_string(),
                icon: None,
            }])
        });

        let result = list_apps_logic(&mock);
        assert!(result.is_ok());
        let apps = result.unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Test App");
    }

    #[test]
    fn test_parse_exec_command() {
        // Simple command
        assert_eq!(
            parse_exec_command("firefox"),
            Some(("firefox".to_string(), vec![]))
        );

        // Command with args
        assert_eq!(
            parse_exec_command("echo hello world"),
            Some((
                "echo".to_string(),
                vec!["hello".to_string(), "world".to_string()]
            ))
        );

        // Command with % codes that should be removed
        assert_eq!(
            parse_exec_command("vlc %U"),
            Some(("vlc".to_string(), vec![]))
        );

        // Complex quoted args
        assert_eq!(
            parse_exec_command("grep \"hello world\" file.txt"),
            Some((
                "grep".to_string(),
                vec!["hello world".to_string(), "file.txt".to_string()]
            ))
        );

        // Empty/Invalid
        assert_eq!(parse_exec_command(""), None);
    }
}
