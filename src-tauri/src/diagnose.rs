//! Assembles and formats a `--diagnose` report: what every launch-time detector
//! decided, without ever mutating the process or opening a window. Collection
//! (`collect_report`) is impure -- it re-runs the same detectors `main()` uses --
//! while `describe_launcher_backend` and `DiagnoseReport::format` are pure so they
//! stay unit-testable without a real display server, systemd session, or terminal.

use crate::adapters::cached_icon_resolver::CachedIconResolver;
use crate::adapters::fs_app_repository::FsAppRepository;
use crate::adapters::fs_config_service::FsConfigService;
use crate::adapters::webkit_nvidia_quirk::{
    decide_quirk, detect_gpu_vendor_verbose, detect_quirk_input, detect_session_type_verbose,
    GpuVendor, QuirkAction, SessionType,
};
use crate::commands::apps::{
    resolve_terminal_emulator, terminal_arg_style as terminal_style_for, TermArgStyle,
};
use crate::ports::app_port::AppRepository;
use crate::ports::config_port::ConfigService;
use crate::ports::icon_port::IconResolver;
use std::collections::HashMap;
use std::sync::Arc;

pub struct DiagnoseReport {
    pub gpu_vendor: GpuVendor,
    pub gpu_evidence: String,
    pub session_type: SessionType,
    pub session_source: &'static str,
    pub compositor_hint: Option<String>,
    pub quirk_action: QuirkAction,
    pub env_delta: Vec<EnvDeltaEntry>,
    pub launcher_backend: LauncherBackend,
    pub launcher_backend_reason: String,
    pub terminal: Option<String>,
    pub terminal_arg_style: Option<&'static str>,
    pub icon_theme: String,
    pub sample_icon_name: String,
    pub sample_icon_path: Option<String>,
    pub app_count: usize,
    pub app_count_error: Option<String>,
}

pub struct EnvDeltaEntry {
    pub key: String,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherBackend {
    Systemd,
    Fallback,
}

/// Pure: derives the launcher backend + human-readable reason from a single
/// `which::which("systemd-run")` probe result, so it is testable without a real
/// systemd user session.
pub fn describe_launcher_backend(systemd_run_available: bool) -> (LauncherBackend, String) {
    if systemd_run_available {
        (
            LauncherBackend::Systemd,
            "systemd-run found on PATH".to_string(),
        )
    } else {
        (
            LauncherBackend::Fallback,
            "systemd-run not found on PATH, using hardened setsid+double-fork fallback"
                .to_string(),
        )
    }
}

/// Compares a captured env snapshot against a live env read, producing one
/// entry per key present in either side. Never carries a value -- only the key
/// name and whether the two sides agree.
fn compute_env_delta(
    snapshot: &HashMap<String, String>,
    live: &HashMap<String, String>,
) -> Vec<EnvDeltaEntry> {
    let mut keys: Vec<&String> = snapshot.keys().chain(live.keys()).collect();
    keys.sort();
    keys.dedup();

    keys.into_iter()
        .map(|key| EnvDeltaEntry {
            key: key.clone(),
            changed: snapshot.get(key) != live.get(key),
        })
        .collect()
}

/// Assembles the icon-theme/sample-icon/app-count section from an already-constructed
/// resolver and repository. Split out from `collect_report` so tests can inject
/// fixture adapters (`CachedIconResolver::new_with_theme_and_roots`,
/// `FsAppRepository::new_with_paths`) without touching the real filesystem.
fn collect_icon_and_app_section(
    icon_resolver: &CachedIconResolver,
    app_repository: &dyn AppRepository,
) -> (String, String, Option<String>, usize, Option<String>) {
    let icon_theme = icon_resolver.theme().to_string();

    let (app_count, app_count_error, apps) = match app_repository.list_apps() {
        Ok(apps) => (apps.len(), None, apps),
        Err(err) => (0, Some(err), Vec::new()),
    };

    let sample_icon_name = apps
        .first()
        .and_then(|app| app.icon.clone())
        .unwrap_or_else(|| "application-x-executable".to_string());
    let sample_icon_path = icon_resolver.resolve_icon(&sample_icon_name, 24, 1);

    (
        icon_theme,
        sample_icon_name,
        sample_icon_path,
        app_count,
        app_count_error,
    )
}

/// Impure: re-runs every launch-time detector read-only (never `apply_quirk`, never
/// `std::env::set_var`) and assembles the full report. The live-env read for the
/// delta section is the one deliberate exception to "always read the snapshot" --
/// see `commands/apps.rs`'s own documented carve-out for the same pattern.
pub fn collect_report(env_snapshot: &HashMap<String, String>) -> DiagnoseReport {
    let (gpu_vendor, gpu_evidence) = detect_gpu_vendor_verbose();
    let (session_type, session_source) = detect_session_type_verbose();

    let quirk_input = detect_quirk_input();
    let quirk_action = decide_quirk(&quirk_input);

    let compositor_hint = if quirk_input.is_hyprland {
        Some("Hyprland".to_string())
    } else {
        env_snapshot.get("XDG_CURRENT_DESKTOP").cloned()
    };

    let live_env: HashMap<String, String> = std::env::vars().collect();
    let env_delta = compute_env_delta(env_snapshot, &live_env);

    let systemd_run_available = which::which("systemd-run").is_ok();
    let (launcher_backend, launcher_backend_reason) =
        describe_launcher_backend(systemd_run_available);

    let config = FsConfigService::new().load_config();
    let terminal = resolve_terminal_emulator(env_snapshot, &config, |p| which::which(p).is_ok());
    let terminal_arg_style = terminal.as_deref().map(|t| match terminal_style_for(t) {
        TermArgStyle::DashE => "-e",
        TermArgStyle::DoubleDash => "--",
    });

    let icon_resolver = Arc::new(CachedIconResolver::new());
    let app_repository = FsAppRepository::new(icon_resolver.clone());
    let (icon_theme, sample_icon_name, sample_icon_path, app_count, app_count_error) =
        collect_icon_and_app_section(&icon_resolver, &app_repository);

    DiagnoseReport {
        gpu_vendor,
        gpu_evidence,
        session_type,
        session_source,
        compositor_hint,
        quirk_action,
        env_delta,
        launcher_backend,
        launcher_backend_reason,
        terminal,
        terminal_arg_style,
        icon_theme,
        sample_icon_name,
        sample_icon_path,
        app_count,
        app_count_error,
    }
}

impl DiagnoseReport {
    /// Pure: renders every field as labeled lines suitable for pasting into a bug
    /// report. Never renders a raw environment value -- only key names and a
    /// changed/unchanged flag for the delta section.
    pub fn format(&self) -> String {
        let mut out = String::new();

        out.push_str("=== GPU ===\n");
        out.push_str(&format!("Vendor: {:?}\n", self.gpu_vendor));
        out.push_str(&format!("Evidence: {}\n\n", self.gpu_evidence));

        out.push_str("=== Session ===\n");
        out.push_str(&format!("Type: {:?}\n", self.session_type));
        out.push_str(&format!("Source: {}\n", self.session_source));
        match &self.compositor_hint {
            Some(hint) => out.push_str(&format!("Compositor hint: {hint}\n\n")),
            None => out.push_str("Compositor hint: unknown\n\n"),
        }

        out.push_str("=== WebKit/NVIDIA quirk ===\n");
        match self.quirk_action {
            QuirkAction::None => out.push_str("Quirk: no quirk applied\n\n"),
            QuirkAction::SetVar { key, value } => {
                out.push_str(&format!("Quirk: sets {key}={value}\n\n"));
            }
        }

        out.push_str("=== Launcher backend ===\n");
        out.push_str(&format!("Backend: {:?}\n", self.launcher_backend));
        out.push_str(&format!("Reason: {}\n\n", self.launcher_backend_reason));

        out.push_str("=== Terminal ===\n");
        match (&self.terminal, self.terminal_arg_style) {
            (Some(term), Some(style)) => {
                out.push_str(&format!("Emulator: {term}\n"));
                out.push_str(&format!("Argument style: {style}\n\n"));
            }
            _ => out.push_str("Emulator: no terminal emulator found\n\n"),
        }

        out.push_str("=== Icons & Apps ===\n");
        out.push_str(&format!("Icon theme: {}\n", self.icon_theme));
        out.push_str(&format!("Sample icon name: {}\n", self.sample_icon_name));
        match &self.sample_icon_path {
            Some(path) => out.push_str(&format!("Sample icon path: {path}\n")),
            None => out.push_str("Sample icon path: not found\n"),
        }
        out.push_str(&format!("App count: {}\n", self.app_count));
        if let Some(err) = &self.app_count_error {
            out.push_str(&format!("App count error: {err}\n"));
        }
        out.push('\n');

        out.push_str("=== Environment delta (names only) ===\n");
        for entry in &self.env_delta {
            let flag = if entry.changed { "changed" } else { "unchanged" };
            out.push_str(&format!("{}: {}\n", entry.key, flag));
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::config::AppConfig;
    use std::fs::File;
    use tempfile::tempdir;

    fn fixture_report(quirk_action: QuirkAction, env_delta: Vec<EnvDeltaEntry>) -> DiagnoseReport {
        DiagnoseReport {
            gpu_vendor: GpuVendor::Other,
            gpu_evidence: "no NVIDIA device found under /sys/class/drm".to_string(),
            session_type: SessionType::Unknown,
            session_source: "none",
            compositor_hint: None,
            quirk_action,
            env_delta,
            launcher_backend: LauncherBackend::Fallback,
            launcher_backend_reason:
                "systemd-run not found on PATH, using hardened setsid+double-fork fallback"
                    .to_string(),
            terminal: None,
            terminal_arg_style: None,
            icon_theme: "hicolor".to_string(),
            sample_icon_name: "application-x-executable".to_string(),
            sample_icon_path: None,
            app_count: 0,
            app_count_error: None,
        }
    }

    // --- describe_launcher_backend (AC3) ---

    #[test]
    fn describe_launcher_backend_true_is_systemd() {
        let (backend, reason) = describe_launcher_backend(true);
        assert_eq!(backend, LauncherBackend::Systemd);
        assert!(reason.contains("systemd-run found on PATH"));
    }

    #[test]
    fn describe_launcher_backend_false_is_fallback() {
        let (backend, reason) = describe_launcher_backend(false);
        assert_eq!(backend, LauncherBackend::Fallback);
        assert!(reason.contains("systemd-run not found on PATH"));
        assert!(reason.contains("setsid"));
    }

    // --- format() over QuirkAction (AC2) ---

    #[test]
    fn format_names_no_quirk_applied() {
        let report = fixture_report(QuirkAction::None, Vec::new());
        assert!(report.format().contains("no quirk applied"));
    }

    #[test]
    fn format_names_every_set_var_key() {
        let cases = [
            QuirkAction::SetVar {
                key: "WEBKIT_DISABLE_DMABUF_RENDERER",
                value: "1",
            },
            QuirkAction::SetVar {
                key: "__NV_DISABLE_EXPLICIT_SYNC",
                value: "1",
            },
        ];

        for action in cases {
            let report = fixture_report(action, Vec::new());
            let formatted = report.format();
            if let QuirkAction::SetVar { key, value } = action {
                assert!(
                    formatted.contains(key),
                    "formatted report missing key {key}: {formatted}"
                );
                assert!(
                    formatted.contains(value),
                    "formatted report missing value {value}: {formatted}"
                );
            }
        }
    }

    // --- env delta / redaction (AC6) ---

    #[test]
    fn compute_env_delta_marks_unchanged_key() {
        let mut before = HashMap::new();
        before.insert("HOME".to_string(), "/home/user".to_string());
        let after = before.clone();

        let delta = compute_env_delta(&before, &after);
        let entry = delta.iter().find(|e| e.key == "HOME").unwrap();
        assert!(!entry.changed);
    }

    #[test]
    fn compute_env_delta_marks_changed_value_key() {
        let mut before = HashMap::new();
        before.insert("FOO".to_string(), "1".to_string());
        let mut after = HashMap::new();
        after.insert("FOO".to_string(), "2".to_string());

        let delta = compute_env_delta(&before, &after);
        let entry = delta.iter().find(|e| e.key == "FOO").unwrap();
        assert!(entry.changed);
    }

    #[test]
    fn compute_env_delta_marks_added_key() {
        let before = HashMap::new();
        let mut after = HashMap::new();
        after.insert("NEWVAR".to_string(), "x".to_string());

        let delta = compute_env_delta(&before, &after);
        let entry = delta.iter().find(|e| e.key == "NEWVAR").unwrap();
        assert!(entry.changed);
    }

    #[test]
    fn compute_env_delta_marks_removed_key() {
        let mut before = HashMap::new();
        before.insert("GONE".to_string(), "x".to_string());
        let after = HashMap::new();

        let delta = compute_env_delta(&before, &after);
        let entry = delta.iter().find(|e| e.key == "GONE").unwrap();
        assert!(entry.changed);
    }

    #[test]
    fn format_env_delta_reports_flags_and_never_a_value() {
        let env_delta = vec![
            EnvDeltaEntry {
                key: "UNCHANGED_VAR".to_string(),
                changed: false,
            },
            EnvDeltaEntry {
                key: "CHANGED_VAR".to_string(),
                changed: true,
            },
            EnvDeltaEntry {
                key: "ADDED_VAR".to_string(),
                changed: true,
            },
            EnvDeltaEntry {
                key: "REMOVED_VAR".to_string(),
                changed: true,
            },
        ];
        let report = fixture_report(QuirkAction::None, env_delta);
        let formatted = report.format();

        assert!(formatted.contains("UNCHANGED_VAR: unchanged"));
        assert!(formatted.contains("CHANGED_VAR: changed"));
        assert!(formatted.contains("ADDED_VAR: changed"));
        assert!(formatted.contains("REMOVED_VAR: changed"));
    }

    #[test]
    fn format_never_leaks_secret_value_only_the_key_name() {
        let secret_value = "ghp_fake_token_value";
        let mut before = HashMap::new();
        before.insert("GITHUB_TOKEN".to_string(), secret_value.to_string());
        let after = HashMap::new();

        let env_delta = compute_env_delta(&before, &after);
        let report = fixture_report(QuirkAction::None, env_delta);
        let formatted = report.format();

        assert!(!formatted.contains(secret_value));
        assert!(formatted.contains("GITHUB_TOKEN"));
    }

    // --- terminal precedence (AC4), calling resolve_terminal_emulator directly ---

    #[test]
    fn terminal_env_override_present_and_probe_ok_wins() {
        let mut env = HashMap::new();
        env.insert("TERMINAL".to_string(), "alacritty".to_string());
        let config = AppConfig::default();

        let result = resolve_terminal_emulator(&env, &config, |p| p == "alacritty");
        assert_eq!(result, Some("alacritty".to_string()));
    }

    #[test]
    fn terminal_env_override_present_but_probe_fails_falls_through_to_config() {
        let mut env = HashMap::new();
        env.insert("TERMINAL".to_string(), "ghost-term".to_string());
        let mut config = AppConfig::default();
        config.terminal_emulator = Some("kitty".to_string());

        let result = resolve_terminal_emulator(&env, &config, |p| p == "kitty");
        assert_eq!(result, Some("kitty".to_string()));
    }

    #[test]
    fn terminal_config_override_used_when_env_absent() {
        let env = HashMap::new();
        let mut config = AppConfig::default();
        config.terminal_emulator = Some("konsole".to_string());

        let result = resolve_terminal_emulator(&env, &config, |p| p == "konsole");
        assert_eq!(result, Some("konsole".to_string()));
    }

    #[test]
    fn terminal_known_terminals_fallback_used_when_nothing_else_resolves() {
        let env = HashMap::new();
        let config = AppConfig::default();

        let result = resolve_terminal_emulator(&env, &config, |p| p == "xterm");
        assert_eq!(result, Some("xterm".to_string()));
    }

    #[test]
    fn terminal_none_resolves_and_format_says_so() {
        let env = HashMap::new();
        let config = AppConfig::default();

        let result = resolve_terminal_emulator(&env, &config, |_| false);
        assert_eq!(result, None);

        let report = fixture_report(QuirkAction::None, Vec::new());
        assert!(report.format().contains("no terminal emulator found"));
    }

    // --- icon theme / sample icon / app count (AC5) ---

    fn write_desktop(dir: &std::path::Path, name: &str, contents: &str) {
        std::fs::write(dir.join(format!("{name}.desktop")), contents).unwrap();
    }

    // Deliberately avoids `freedesktop_icons`' theme-name lookup path: that crate
    // caches XDG_DATA_HOME/XDG_DATA_DIRS discovery in process-wide `Lazy` statics
    // on first access (see the equivalent caveat in
    // `cached_icon_resolver.rs::tests::test_theme_fixture_dir`), which races other
    // tests in this binary. An absolute, already-existing icon path takes
    // `resolve_icon_internal`'s direct-path branch instead, which never touches
    // that global cache -- deterministic regardless of test execution order.
    #[test]
    fn icon_section_resolves_sample_icon_from_fixture_root() {
        let icon_dir = tempdir().unwrap();
        let icon_path = icon_dir.path().join("fixture-icon.png");
        File::create(&icon_path).unwrap();

        let apps_dir = tempdir().unwrap();
        write_desktop(
            apps_dir.path(),
            "fixture-app",
            &format!(
                "[Desktop Entry]\nName=Fixture App\nExec=fixture-app\nIcon={}\nType=Application\n",
                icon_path.display()
            ),
        );

        let icon_resolver = Arc::new(CachedIconResolver::new_with_theme_and_roots(
            "FixtureTheme".to_string(),
            vec![icon_dir.path().to_path_buf()],
        ));
        let app_repository = FsAppRepository::new_with_paths(
            icon_resolver.clone(),
            vec![apps_dir.path().to_path_buf()],
        );

        let (_, sample_icon_name, sample_icon_path, _, _) =
            collect_icon_and_app_section(&icon_resolver, &app_repository);

        assert!(
            sample_icon_name.contains("fixture-icon.png"),
            "expected sample icon name to come from the fixture app's Icon= path: {sample_icon_name}"
        );
        assert_eq!(sample_icon_path, Some(sample_icon_name));
    }

    #[test]
    fn icon_section_with_empty_roots_reports_not_found() {
        let empty_apps_dir = tempdir().unwrap();
        let icon_resolver = Arc::new(CachedIconResolver::new_with_theme_and_roots(
            "FixtureTheme".to_string(),
            Vec::new(),
        ));
        let app_repository = FsAppRepository::new_with_paths(
            icon_resolver.clone(),
            vec![empty_apps_dir.path().to_path_buf()],
        );

        let (icon_theme, _, sample_icon_path, _, _) =
            collect_icon_and_app_section(&icon_resolver, &app_repository);

        assert_eq!(icon_theme, "FixtureTheme");
        assert_eq!(sample_icon_path, None);

        let mut report = fixture_report(QuirkAction::None, Vec::new());
        report.sample_icon_path = sample_icon_path;
        assert!(report.format().contains("Sample icon path: not found"));
    }

    #[test]
    fn app_count_matches_fixture_desktop_dir_size() {
        let dir = tempdir().unwrap();
        let apps_dir = dir.path().join("applications");
        std::fs::create_dir(&apps_dir).unwrap();
        write_desktop(
            &apps_dir,
            "one",
            "[Desktop Entry]\nName=One\nExec=one\nType=Application\n",
        );
        write_desktop(
            &apps_dir,
            "two",
            "[Desktop Entry]\nName=Two\nExec=two\nType=Application\n",
        );
        write_desktop(
            &apps_dir,
            "three",
            "[Desktop Entry]\nName=Three\nExec=three\nType=Application\n",
        );

        let icon_resolver = Arc::new(CachedIconResolver::new_with_theme_and_roots(
            "hicolor".to_string(),
            Vec::new(),
        ));
        let app_repository = FsAppRepository::new_with_paths(icon_resolver.clone(), vec![apps_dir]);

        let (_, _, _, app_count, app_count_error) =
            collect_icon_and_app_section(&icon_resolver, &app_repository);

        assert_eq!(app_count, 3);
        assert_eq!(app_count_error, None);
    }

    #[test]
    fn app_count_falls_back_to_fixed_name_when_no_apps_discovered() {
        let dir = tempdir().unwrap();
        let apps_dir = dir.path().join("applications");
        std::fs::create_dir(&apps_dir).unwrap();

        let icon_resolver = Arc::new(CachedIconResolver::new_with_theme_and_roots(
            "hicolor".to_string(),
            Vec::new(),
        ));
        let app_repository = FsAppRepository::new_with_paths(icon_resolver.clone(), vec![apps_dir]);

        let (_, sample_icon_name, _, app_count, _) =
            collect_icon_and_app_section(&icon_resolver, &app_repository);

        assert_eq!(app_count, 0);
        assert_eq!(sample_icon_name, "application-x-executable");
    }
}
