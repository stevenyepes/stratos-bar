use crate::domain::apps::AppEntry;
use crate::domain::config::AppConfig;
use crate::ports::app_launcher_port::AppLauncher;
use crate::ports::app_port::AppRepository;
use crate::ports::config_port::ConfigService;
use crate::state::AppState;
use std::collections::HashMap;
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

fn generate_scope_name(app_id: &str) -> String {
    let sanitized: String = app_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!(
        "stratos-bar-{sanitized}-{}-{}.scope",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TermArgStyle {
    /// `<term> -e <argv...>` — argv elements appended individually after `-e`.
    DashE,
    /// `<term> -- <argv...>` — argv elements appended individually after `--`.
    DoubleDash,
}

struct TerminalSpec {
    /// Binary basename as it appears on $PATH, e.g. "alacritty".
    program: &'static str,
    style: TermArgStyle,
}

const KNOWN_TERMINALS: &[TerminalSpec] = &[
    TerminalSpec {
        program: "alacritty",
        style: TermArgStyle::DashE,
    },
    TerminalSpec {
        program: "kitty",
        style: TermArgStyle::DashE,
    },
    TerminalSpec {
        program: "foot",
        style: TermArgStyle::DashE,
    },
    TerminalSpec {
        program: "konsole",
        style: TermArgStyle::DashE,
    },
    TerminalSpec {
        program: "xterm",
        style: TermArgStyle::DashE,
    },
    TerminalSpec {
        program: "gnome-terminal",
        style: TermArgStyle::DoubleDash,
    },
    TerminalSpec {
        program: "wezterm",
        style: TermArgStyle::DoubleDash,
    },
];

/// Looks up the argument style for `program` by basename, defaulting to
/// `TermArgStyle::DashE` when the basename isn't in `KNOWN_TERMINALS` -- a
/// deliberate fallback, not a gap (see spec Open Questions).
fn terminal_arg_style(program: &str) -> TermArgStyle {
    let basename = std::path::Path::new(program)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(program);
    KNOWN_TERMINALS
        .iter()
        .find(|spec| spec.program == basename)
        .map(|spec| spec.style)
        .unwrap_or(TermArgStyle::DashE)
}

/// Builds `[terminal, "-e"|"--", ...argv]` as discrete `Vec<String>` elements --
/// never re-serialized into a joined string and re-split.
fn wrap_in_terminal(terminal: &str, style: TermArgStyle, argv: Vec<String>) -> Vec<String> {
    let flag = match style {
        TermArgStyle::DashE => "-e",
        TermArgStyle::DoubleDash => "--",
    };
    let mut wrapped = Vec::with_capacity(argv.len() + 2);
    wrapped.push(terminal.to_string());
    wrapped.push(flag.to_string());
    wrapped.extend(argv);
    wrapped
}

/// Resolves which terminal emulator to use, in precedence order: `$TERMINAL` (read
/// from the passed-in `env` snapshot, never `std::env::var` directly) -> the
/// `AppConfig::terminal_emulator` key -> the first entry of `KNOWN_TERMINALS` (in
/// table order) that `probe` reports as installed. Each step is skipped, not
/// treated as fatal, when its candidate fails `probe`.
fn resolve_terminal_emulator(
    env: &HashMap<String, String>,
    config: &AppConfig,
    probe: impl Fn(&str) -> bool,
) -> Option<String> {
    if let Some(from_env) = env.get("TERMINAL") {
        if probe(from_env) {
            return Some(from_env.clone());
        }
    }

    if let Some(from_config) = &config.terminal_emulator {
        if probe(from_config) {
            return Some(from_config.clone());
        }
    }

    KNOWN_TERMINALS
        .iter()
        .find(|spec| probe(spec.program))
        .map(|spec| spec.program.to_string())
}

/// Combines `build_exec_argv`, `entry.terminal`, and terminal resolution/wrapping into
/// one `Result`-returning step: non-terminal entries pass through unchanged, terminal
/// entries are wrapped with the resolved emulator's argv, and an unresolvable emulator
/// is a visible `Err` naming the entry rather than a silent no-op.
fn resolve_terminal_argv(
    entry: &AppEntry,
    env: &HashMap<String, String>,
    config: &AppConfig,
    probe: impl Fn(&str) -> bool,
) -> Result<Vec<String>, String> {
    let argv = build_exec_argv(&entry.exec)?;
    if !entry.terminal {
        return Ok(argv);
    }
    let terminal = resolve_terminal_emulator(env, config, probe)
        .ok_or_else(|| format!("No terminal emulator found to launch {}", entry.name))?;
    Ok(wrap_in_terminal(
        &terminal,
        terminal_arg_style(&terminal),
        argv,
    ))
}

pub fn launch_app_logic(
    repo: &dyn AppRepository,
    launcher: &dyn AppLauncher,
    id: &str,
    env: &HashMap<String, String>,
    config: &dyn ConfigService,
) -> Result<(), String> {
    let entry = repo
        .resolve(id)
        .map_err(|e| format!("Failed to resolve app {id}: {e}"))?
        .ok_or_else(|| format!("No app found for id: {id}"))?;

    let argv = resolve_terminal_argv(&entry, env, &config.load_config(), |p| {
        which::which(p).is_ok()
    })?;
    let scope_name = generate_scope_name(&entry.id);
    launcher.launch(&argv, env, entry.working_dir.as_deref(), &scope_name)
}

#[tauri::command]
pub async fn launch_app(state: State<'_, AppState>, id: String) -> Result<(), String> {
    launch_app_logic(
        &*state.app_repository,
        &*state.app_launcher,
        &id,
        &state.env_port.snapshot(),
        &*state.config_service,
    )
}

pub fn run_command_logic(
    launcher: &dyn AppLauncher,
    cmd: &str,
    env: &HashMap<String, String>,
) -> Result<(), String> {
    let argv = split_command(cmd)?;
    let scope_name = generate_scope_name("run-command");
    launcher.launch(&argv, env, None, &scope_name)
}

#[tauri::command]
pub async fn run_command(state: State<'_, AppState>, cmd: String) -> Result<(), String> {
    run_command_logic(&*state.app_launcher, &cmd, &state.env_port.snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::systemd_launcher::SystemdScopeLauncher;
    use crate::domain::apps::AppSource;
    use crate::ports::app_launcher_port::MockAppLauncher;
    use crate::ports::app_port::MockAppRepository;
    use crate::ports::config_port::MockConfigService;
    use mockall::Predicate;

    fn noop_config() -> MockConfigService {
        let mut mock = MockConfigService::new();
        mock.expect_load_config().returning(|| AppConfig::default());
        mock
    }

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
                terminal: false,
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

    fn fixture_app_entry(
        id: &str,
        exec: &str,
        working_dir: Option<String>,
        terminal: bool,
    ) -> AppEntry {
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
            terminal,
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
        let mock = MockAppLauncher::new();
        assert!(run_command_logic(&mock, "", &HashMap::new()).is_err());
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
        let launcher = MockAppLauncher::new();

        let result = launch_app_logic(
            &mock,
            &launcher,
            "nonexistent",
            &HashMap::new(),
            &noop_config(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_launch_app_logic_resolve_error_errs() {
        let mut mock = MockAppRepository::new();
        mock.expect_resolve()
            .returning(|_| Err("repository failure".to_string()));
        let launcher = MockAppLauncher::new();

        let result = launch_app_logic(&mock, &launcher, "any", &HashMap::new(), &noop_config());
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
            false,
        );

        let mut mock = MockAppRepository::new();
        mock.expect_resolve()
            .returning(move |_| Ok(Some(entry.clone())));
        let launcher = SystemdScopeLauncher::with_availability_override(false);

        let snapshot: HashMap<String, String> = std::env::vars().collect();
        let result = launch_app_logic(&mock, &launcher, "app-with-cwd", &snapshot, &noop_config());
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

        let launcher = SystemdScopeLauncher::with_availability_override(false);
        let result = run_command_logic(&launcher, &cmd, &snapshot);

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

    #[test]
    fn test_launch_app_logic_non_terminal_entry_unaffected() {
        let entry = fixture_app_entry("app-a", "echo hi", Some("/tmp".to_string()), false);
        let mut repo = MockAppRepository::new();
        repo.expect_resolve()
            .returning(move |_| Ok(Some(entry.clone())));

        let mut launcher = MockAppLauncher::new();
        launcher
            .expect_launch()
            .times(1)
            .withf(|argv, env, current_dir, _scope_name| {
                mockall::predicate::eq(vec!["echo".to_string(), "hi".to_string()]).eval(argv)
                    && mockall::predicate::eq(HashMap::new()).eval(env)
                    && *current_dir == Some("/tmp")
            })
            .returning(|_, _, _, _| Ok(()));

        let result = launch_app_logic(&repo, &launcher, "app-a", &HashMap::new(), &noop_config());
        assert!(
            result.is_ok(),
            "launch_app_logic failed: {:?}",
            result.err()
        );
    }

    #[cfg(unix)]
    fn make_executable_fixture(dir: &std::path::Path, name: &str, contents: &str) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    fn test_launch_app_logic_wraps_terminal_entry_argv() {
        let tempdir = tempfile::tempdir().unwrap();
        let fixture_path =
            make_executable_fixture(tempdir.path(), "fake-term", "#!/bin/sh\nexit 0\n");

        let entry = fixture_app_entry("btop", "btop", None, true);
        let mut repo = MockAppRepository::new();
        repo.expect_resolve()
            .returning(move |_| Ok(Some(entry.clone())));

        let mut config = MockConfigService::new();
        let terminal_emulator = fixture_path.clone();
        config.expect_load_config().returning(move || {
            let mut cfg = AppConfig::default();
            cfg.terminal_emulator = Some(terminal_emulator.clone());
            cfg
        });

        let expected_argv = vec![fixture_path.clone(), "-e".to_string(), "btop".to_string()];
        let mut launcher = MockAppLauncher::new();
        launcher
            .expect_launch()
            .times(1)
            .withf(move |argv, _env, _current_dir, _scope_name| argv == expected_argv.as_slice())
            .returning(|_, _, _, _| Ok(()));

        let result = launch_app_logic(&repo, &launcher, "btop", &HashMap::new(), &config);
        assert!(
            result.is_ok(),
            "launch_app_logic failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_launch_app_logic_terminal_entry_reaches_real_launcher() {
        let tempdir = tempfile::tempdir().unwrap();
        let fixture_path = make_executable_fixture(
            tempdir.path(),
            "fake-term-e",
            "#!/bin/sh\nshift\nexec \"$@\"\n",
        );

        let marker = unique_marker("terminal-e2e");
        let entry = fixture_app_entry(
            "term-app",
            &format!("sh -c 'touch {}'", marker.display()),
            None,
            true,
        );

        let mut repo = MockAppRepository::new();
        repo.expect_resolve()
            .returning(move |_| Ok(Some(entry.clone())));

        let mut config = MockConfigService::new();
        let terminal_emulator = fixture_path.clone();
        config.expect_load_config().returning(move || {
            let mut cfg = AppConfig::default();
            cfg.terminal_emulator = Some(terminal_emulator.clone());
            cfg
        });

        let launcher = SystemdScopeLauncher::with_availability_override(false);
        let snapshot: HashMap<String, String> = std::env::vars().collect();
        let result = launch_app_logic(&repo, &launcher, "term-app", &snapshot, &config);
        assert!(
            result.is_ok(),
            "launch_app_logic failed: {:?}",
            result.err()
        );

        assert!(
            wait_for_marker(&marker),
            "wrapped terminal command never reached the real launcher"
        );
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn test_wrap_in_terminal_dash_e_style() {
        assert_eq!(
            wrap_in_terminal("alacritty", TermArgStyle::DashE, vec!["btop".to_string()]),
            vec![
                "alacritty".to_string(),
                "-e".to_string(),
                "btop".to_string()
            ]
        );
    }

    #[test]
    fn test_wrap_in_terminal_double_dash_style() {
        assert_eq!(
            wrap_in_terminal(
                "gnome-terminal",
                TermArgStyle::DoubleDash,
                vec!["htop".to_string()]
            ),
            vec![
                "gnome-terminal".to_string(),
                "--".to_string(),
                "htop".to_string()
            ]
        );
    }

    #[test]
    fn test_terminal_arg_style_defaults_for_unknown_program() {
        assert_eq!(
            terminal_arg_style("/opt/custom/my-term"),
            TermArgStyle::DashE
        );
    }

    #[test]
    fn test_terminal_arg_style_matches_by_basename() {
        assert_eq!(
            terminal_arg_style("/usr/bin/gnome-terminal"),
            TermArgStyle::DoubleDash
        );
    }

    #[test]
    fn test_resolve_terminal_prefers_env_var() {
        let mut env = HashMap::new();
        env.insert("TERMINAL".to_string(), "alacritty".to_string());
        let config = AppConfig::default();

        let result = resolve_terminal_emulator(&env, &config, |p| p == "alacritty");
        assert_eq!(result, Some("alacritty".to_string()));
    }

    #[test]
    fn test_resolve_terminal_env_var_not_installed_falls_through_to_config() {
        let mut env = HashMap::new();
        env.insert("TERMINAL".to_string(), "ghost-term".to_string());
        let mut config = AppConfig::default();
        config.terminal_emulator = Some("kitty".to_string());

        let result = resolve_terminal_emulator(&env, &config, |p| p == "kitty");
        assert_eq!(result, Some("kitty".to_string()));
    }

    #[test]
    fn test_resolve_terminal_config_key_used_when_env_absent() {
        let env = HashMap::new();
        let mut config = AppConfig::default();
        config.terminal_emulator = Some("konsole".to_string());

        let result = resolve_terminal_emulator(&env, &config, |p| p == "konsole");
        assert_eq!(result, Some("konsole".to_string()));
    }

    #[test]
    fn test_resolve_terminal_config_key_not_installed_falls_through_to_probe_list() {
        let env = HashMap::new();
        let mut config = AppConfig::default();
        config.terminal_emulator = Some("ghost-term".to_string());

        let result = resolve_terminal_emulator(&env, &config, |p| p == "xterm");
        assert_eq!(result, Some("xterm".to_string()));
    }

    #[test]
    fn test_resolve_terminal_probes_known_list_in_order() {
        let env = HashMap::new();
        let config = AppConfig::default();
        let second_candidate = KNOWN_TERMINALS[1].program;

        let result = resolve_terminal_emulator(&env, &config, |p| p == second_candidate);
        assert_eq!(result, Some(second_candidate.to_string()));
    }

    #[test]
    fn test_resolve_terminal_returns_none_when_nothing_resolves() {
        let env = HashMap::new();
        let config = AppConfig::default();

        let result = resolve_terminal_emulator(&env, &config, |_| false);
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_terminal_argv_wraps_terminal_entry() {
        let entry = fixture_app_entry("btop", "btop", None, true);
        let env = HashMap::new();
        let mut config = AppConfig::default();
        config.terminal_emulator = Some("alacritty".to_string());

        let result = resolve_terminal_argv(&entry, &env, &config, |p| p == "alacritty");

        assert_eq!(
            result,
            Ok(vec![
                "alacritty".to_string(),
                "-e".to_string(),
                "btop".to_string()
            ])
        );
    }

    #[test]
    fn test_resolve_terminal_argv_passes_through_non_terminal_entry() {
        let entry = fixture_app_entry("echo hi", "echo hi", None, false);
        let env = HashMap::new();
        let config = AppConfig::default();

        let result = resolve_terminal_argv(&entry, &env, &config, |_| true);

        assert_eq!(result, Ok(build_exec_argv(&entry.exec).unwrap()));
    }

    #[test]
    fn test_resolve_terminal_argv_errs_when_unresolvable() {
        let entry = fixture_app_entry("btop", "btop", None, true);
        let env = HashMap::new();
        let config = AppConfig::default();

        let result = resolve_terminal_argv(&entry, &env, &config, |_| false);

        let err = result.expect_err("expected Err when no terminal emulator resolves");
        assert!(
            err.contains(&entry.name),
            "error {err:?} does not contain entry name {:?}",
            entry.name
        );
    }

    #[test]
    fn test_generate_scope_name_is_distinct_across_calls() {
        let first = generate_scope_name("firefox");
        let second = generate_scope_name("firefox");
        assert_ne!(first, second);
    }

    #[test]
    fn test_generate_scope_name_sanitizes_special_characters() {
        let name = generate_scope_name("org/app with space");
        assert!(!name.contains('/'));
        assert!(!name.contains(' '));
        assert!(name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')));
    }
}
