use crate::domain::apps::AppEntry;
use crate::state::AppState;
use std::os::unix::process::CommandExt;
use tauri::State;

#[tauri::command]
pub async fn list_apps(state: State<'_, AppState>) -> Result<Vec<AppEntry>, String> {
    state.app_repository.list_apps()
}

#[tauri::command]
pub async fn launch_app(exec_cmd: String) -> Result<(), String> {
    // Helper to clean exec command
    fn parse_exec_command(exec_cmd: &str) -> Option<(String, Vec<String>)> {
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
