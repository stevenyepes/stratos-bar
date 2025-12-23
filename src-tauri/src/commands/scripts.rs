use crate::domain::config::ScriptConfig;
use crate::state::AppState;
use tauri::{Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[tauri::command]
pub async fn list_scripts(state: State<'_, AppState>) -> Result<Vec<ScriptConfig>, String> {
    let config = state.config_service.load_config();
    Ok(config.scripts)
}

#[tauri::command]
pub async fn execute_script(
    window: tauri::Window,
    path: String,
    args: Option<String>,
) -> Result<(), String> {
    let path = path.trim();
    let mut cmd;

    if path.ends_with(".sh") {
        cmd = Command::new("sh");
        cmd.arg(path);
    } else {
        cmd = Command::new(path);
    }

    if let Some(args_str) = args {
        let parts = shell_words::split(&args_str).map_err(|e| e.to_string())?;
        cmd.args(parts);
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn script: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to open stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to open stderr")?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    window.emit("script-start", ()).map_err(|e| e.to_string())?;

    // Spawn tasks to handle stdout and stderr concurrently
    let window_clone1 = window.clone();
    tokio::spawn(async move {
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            let _ = window_clone1.emit("script-output", format!("> {}\n", line));
        }
    });

    let window_clone2 = window.clone();
    tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            let _ = window_clone2.emit("script-output", format!("ERR> {}\n", line));
        }
    });

    // Wait for the child process to finish
    let output = child.wait().await.map_err(|e| e.to_string())?;

    let status_msg = if output.success() {
        format!("> Success! (Exit code 0)\n")
    } else {
        format!("> Failed! (Exit code {})\n", output.code().unwrap_or(-1))
    };

    window
        .emit("script-output", status_msg)
        .map_err(|e| e.to_string())?;

    window.emit("script-done", ()).map_err(|e| e.to_string())?;

    Ok(())
}
