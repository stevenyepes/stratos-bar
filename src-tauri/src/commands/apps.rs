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

pub fn set_icon_scale_logic(repo: &dyn AppRepository, scale: u16) -> Result<Vec<AppEntry>, String> {
    repo.set_icon_scale(scale);
    repo.rescan()
}

#[tauri::command]
pub async fn set_icon_scale(
    state: State<'_, AppState>,
    scale: u16,
) -> Result<Vec<AppEntry>, String> {
    set_icon_scale_logic(&*state.app_repository, scale)
}

const FIELD_CODES: &[&str] = &[
    "%f", "%F", "%u", "%U", "%i", "%c", "%k", "%d", "%D", "%n", "%N", "%v", "%m", "%M",
];

/// Matches `@@` and any flatpak `--file-forwarding` placeholder of the form `@@<char>`
/// (e.g. `@@u`). These are only meaningful to flatpak's own arg parser and must be
/// dropped as exact tokens, never substring-replaced.
fn is_flatpak_placeholder(token: &str) -> bool {
    token == "@@" || (token.len() == 3 && token.starts_with("@@"))
}

fn filter_exec_tokens(tokens: Vec<String>) -> Vec<String> {
    tokens
        .into_iter()
        .filter(|t| !FIELD_CODES.contains(&t.as_str()) && !is_flatpak_placeholder(t))
        .collect()
}

/// Tokenizes a desktop entry's `Exec=` value and strips field-code / flatpak-placeholder
/// tokens as an exact-match post-pass over the split result -- never a substring
/// replacement on the raw string, so quoted `%` sequences survive untouched.
fn build_exec_argv(exec: &str) -> Result<Vec<String>, String> {
    let tokens = shell_words::split(exec).map_err(|e| e.to_string())?;
    Ok(filter_exec_tokens(tokens))
}

fn split_command(cmd: &str) -> Result<Vec<String>, String> {
    let argv = shell_words::split(cmd).map_err(|e| e.to_string())?;
    if argv.is_empty() {
        return Err("Empty command".to_string());
    }
    Ok(argv)
}

fn spawn_argv(
    argv: &[String],
    env: &HashMap<String, String>,
    current_dir: Option<&str>,
) -> Result<(), String> {
    if argv.is_empty() {
        return Err("Empty command".to_string());
    }

    let cmd = &argv[0];
    if !cmd.contains('/') && which::which(cmd).is_err() {
        return Err(format!("Command not found in PATH: {cmd}"));
    }

    let mut command = std::process::Command::new(cmd);
    command
        .args(&argv[1..])
        .env_clear()
        .envs(env)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0);

    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }

    command.spawn().map_err(|e| e.to_string())?;

    Ok(())
}

pub fn launch_app_logic(
    repo: &dyn AppRepository,
    id: &str,
    env: &HashMap<String, String>,
) -> Result<(), String> {
    let entry = repo
        .resolve(id)
        .map_err(|e| format!("Failed to resolve app {id}: {e}"))?
        .ok_or_else(|| format!("No app found for id: {id}"))?;

    let argv = build_exec_argv(&entry.exec)?;
    spawn_argv(&argv, env, entry.working_dir.as_deref())
}

#[tauri::command]
pub async fn launch_app(state: State<'_, AppState>, id: String) -> Result<(), String> {
    launch_app_logic(&*state.app_repository, &id, &state.env_port.snapshot())
}

pub fn run_command_logic(cmd: &str, env: &HashMap<String, String>) -> Result<(), String> {
    let argv = split_command(cmd)?;
    spawn_argv(&argv, env, None)
}

#[tauri::command]
pub async fn run_command(state: State<'_, AppState>, cmd: String) -> Result<(), String> {
    run_command_logic(&cmd, &state.env_port.snapshot())
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
                working_dir: None,
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
    fn test_set_icon_scale() {
        let mut mock = MockAppRepository::new();
        mock.expect_set_icon_scale()
            .times(1)
            .with(mockall::predicate::eq(2u16))
            .returning(|_| ());
        mock.expect_rescan().times(1).returning(|| Ok(vec![]));

        let result = set_icon_scale_logic(&mock, 2);
        assert!(result.is_ok());
    }

    fn fixture_app_entry(id: &str, exec: &str, working_dir: Option<String>) -> AppEntry {
        AppEntry {
            id: id.to_string(),
            name: id.to_string(),
            generic_name: None,
            description: None,
            keywords: Vec::new(),
            exec: exec.to_string(),
            try_exec: None,
            icon: None,
            categories: Vec::new(),
            startup_wm_class: None,
            source: AppSource::Desktop,
            path: format!("/tmp/{id}.desktop"),
            working_dir,
        }
    }

    fn unique_marker(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "stratos_bar_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn wait_for_marker(marker: &std::path::Path) -> bool {
        let mut waited = std::time::Duration::from_millis(0);
        let step = std::time::Duration::from_millis(50);
        let timeout = std::time::Duration::from_secs(2);
        while !marker.exists() && waited < timeout {
            std::thread::sleep(step);
            waited += step;
        }
        marker.exists()
    }

    #[test]
    fn test_run_command_logic_tokenizes_plain_command() {
        assert_eq!(
            split_command("firefox").unwrap(),
            vec!["firefox".to_string()]
        );
    }

    #[test]
    fn test_run_command_logic_tokenizes_multiple_args() {
        assert_eq!(
            split_command("echo hello world").unwrap(),
            vec!["echo".to_string(), "hello".to_string(), "world".to_string()]
        );
    }

    #[test]
    fn test_run_command_logic_tokenizes_quoted_args() {
        assert_eq!(
            split_command("grep \"hello world\" file.txt").unwrap(),
            vec![
                "grep".to_string(),
                "hello world".to_string(),
                "file.txt".to_string()
            ]
        );
    }

    #[test]
    fn test_run_command_logic_empty_command_errs() {
        assert!(run_command_logic("", &HashMap::new()).is_err());
    }

    #[test]
    fn test_build_exec_argv_quoted_percent_survives() {
        // Field-code/placeholder removal is an exact-token filter, so a `%` sitting
        // inside a quoted argument (not a bare field code) must pass through untouched.
        let argv = build_exec_argv(r#"echo "50% done""#).unwrap();
        assert_eq!(argv, vec!["echo".to_string(), "50% done".to_string()]);
    }

    #[test]
    fn test_build_exec_argv_strips_flatpak_file_forwarding_placeholders() {
        let argv =
            build_exec_argv("/usr/bin/flatpak run --file-forwarding org.app @@u %u @@").unwrap();
        assert_eq!(
            argv,
            vec![
                "/usr/bin/flatpak".to_string(),
                "run".to_string(),
                "--file-forwarding".to_string(),
                "org.app".to_string(),
            ]
        );
    }

    #[test]
    fn test_launch_app_logic_missing_id_errs() {
        let mut mock = MockAppRepository::new();
        mock.expect_resolve().returning(|_| Ok(None));

        let result = launch_app_logic(&mock, "nonexistent", &HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_launch_app_logic_resolve_error_errs() {
        let mut mock = MockAppRepository::new();
        mock.expect_resolve()
            .returning(|_| Err("repository failure".to_string()));

        let result = launch_app_logic(&mock, "any", &HashMap::new());
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_launch_app_logic_honours_working_dir() {
        let tempdir = tempfile::tempdir().unwrap();
        let marker = unique_marker("workdir");
        let entry = fixture_app_entry(
            "app-with-cwd",
            &format!("sh -c 'pwd > {}'", marker.display()),
            Some(tempdir.path().to_string_lossy().to_string()),
        );

        let mut mock = MockAppRepository::new();
        mock.expect_resolve()
            .returning(move |_| Ok(Some(entry.clone())));

        let snapshot: HashMap<String, String> = std::env::vars().collect();
        let result = launch_app_logic(&mock, "app-with-cwd", &snapshot);
        assert!(
            result.is_ok(),
            "launch_app_logic failed: {:?}",
            result.err()
        );

        assert!(
            wait_for_marker(&marker),
            "spawned command never wrote its marker file"
        );
        let recorded_cwd = std::fs::read_to_string(&marker).unwrap();
        let _ = std::fs::remove_file(&marker);

        assert_eq!(
            recorded_cwd.trim(),
            tempdir.path().to_string_lossy().to_string(),
            "child process was not spawned with entry.working_dir as its current_dir"
        );
    }

    /// Reproduces the GPU regression: main.rs sets WEBKIT_DISABLE_DMABUF_RENDERER on
    /// stratos-bar's own process to work around a WebKitGTK black-window bug, and
    /// spawned children inherit that var unless the env is built from a snapshot. This
    /// test simulates the poisoned parent process and asserts a child launched through
    /// run_command_logic does NOT see the var.
    #[tokio::test]
    async fn test_run_command_logic_inherits_poisoned_environment() {
        let marker = unique_marker("repro");
        let _ = std::fs::remove_file(&marker);

        // Simulate main.rs's pre-mutation snapshot, captured before the workaround below.
        let snapshot: HashMap<String, String> = std::env::vars().collect();

        // Simulate the workaround main.rs applies to its own process before doing
        // anything else.
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

        let cmd = format!(
            "sh -c 'if [ -z \"$WEBKIT_DISABLE_DMABUF_RENDERER\" ]; then touch {}; fi'",
            marker.display()
        );

        let result = run_command_logic(&cmd, &snapshot);

        std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER");

        assert!(
            result.is_ok(),
            "run_command_logic failed: {:?}",
            result.err()
        );

        // Spawns fire-and-forget, so poll briefly for the child to finish.
        let inherited = !wait_for_marker(&marker);
        let _ = std::fs::remove_file(&marker);

        assert!(
            !inherited,
            "child spawned by run_command_logic inherited WEBKIT_DISABLE_DMABUF_RENDERER from \
             the poisoned parent environment; run_command_logic must build the child's \
             environment from a snapshot captured before main.rs mutates it, not from live \
             inheritance"
        );
    }
}
