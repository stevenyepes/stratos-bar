use crate::ports::app_launcher_port::AppLauncher;
use std::collections::HashMap;
use std::os::unix::process::CommandExt;

const SYSTEMD_RUN: &str = "systemd-run";

/// Hands `child` to a detached thread that `wait()`s on it. Every process this module
/// spawns directly is a child of stratos-bar and must be reaped by someone: nothing
/// else in the codebase installs a `SIGCHLD` handler or calls `waitpid`, and
/// `Child`'s `Drop` impl deliberately does not wait, so an un-reaped handle leaves a
/// zombie in our process table from the moment the process exits until stratos-bar
/// itself exits — which, for a bar meant to run a whole desktop session, is forever.
fn reap_in_background(mut child: std::process::Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

fn build_systemd_run_argv(scope_name: &str, argv: &[String]) -> Vec<String> {
    let mut result = vec![
        "--user".to_string(),
        "--scope".to_string(),
        format!("--unit={scope_name}"),
        "--collect".to_string(),
        "--".to_string(),
    ];
    result.extend(argv.iter().cloned());
    result
}

/// Direct fallback spawn used when `systemd-run` is unavailable or fails to start.
/// Hardens the launched process against dying with stratos-bar via `setsid` +
/// double-fork (reparenting it to init). Note: `Command::process_group(0)` is
/// deliberately NOT used here — it calls `setpgid(0, 0)` on the immediate child
/// before our `pre_exec` closure runs, which makes that child a process-group
/// leader and causes our closure's `setsid()` call to fail with `EPERM` (POSIX:
/// `setsid` requires the caller not already be a process-group leader). `setsid()`
/// already subsumes what `process_group(0)` was for here — it puts the process in
/// a brand new session and process group (as its leader), a strictly stronger
/// detachment from stratos-bar's process group and controlling terminal.
fn spawn_fallback(
    argv: &[String],
    env: &HashMap<String, String>,
    current_dir: Option<&str>,
) -> Result<(), String> {
    if argv.is_empty() {
        return Err("Empty command".to_string());
    }

    let mut command = std::process::Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .env_clear()
        .envs(env)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }

    // Safety: this closure runs between fork() and exec() in the child, so only
    // async-signal-safe calls are permitted. It calls only libc::setsid, libc::fork,
    // libc::_exit, and std::io::Error::last_os_error() (which only reads errno) — no
    // heap allocation, no logging, no panics/unwinding.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            match libc::fork() {
                -1 => Err(std::io::Error::last_os_error()),
                0 => Ok(()),
                _ => libc::_exit(0),
            }
        });
    }

    let child = command.spawn().map_err(|e| e.to_string())?;

    // The intermediate child (the one this `Child` handle refers to) exits within
    // microseconds of the second fork(); reap it so it never lingers as a zombie.
    // The grandchild (the real launched app) is already reparented to init and is not
    // this process's child to wait on.
    reap_in_background(child);

    Ok(())
}

fn spawn_systemd_run(
    program: &str,
    scope_name: &str,
    argv: &[String],
    env: &HashMap<String, String>,
    current_dir: Option<&str>,
) -> std::io::Result<std::process::Child> {
    let systemd_argv = build_systemd_run_argv(scope_name, argv);

    let mut command = std::process::Command::new(program);
    command
        .args(&systemd_argv)
        .env_clear()
        .envs(env)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }

    command.spawn()
}

/// Launches processes preferring `systemd-run --user --scope` (own cgroup); falls back
/// to a hardened direct spawn (setsid + double-fork) when `systemd-run` is unavailable
/// or its own process fails to start.
pub struct SystemdScopeLauncher {
    availability_override: Option<bool>,
    systemd_run_program: String,
}

impl SystemdScopeLauncher {
    pub fn new() -> Self {
        Self {
            availability_override: None,
            systemd_run_program: SYSTEMD_RUN.to_string(),
        }
    }

    #[cfg(test)]
    pub fn with_availability_override(available: bool) -> Self {
        Self {
            availability_override: Some(available),
            systemd_run_program: SYSTEMD_RUN.to_string(),
        }
    }

    /// Test-only: forces the availability verdict *and* substitutes the program the
    /// "available" branch actually spawns, so both outcomes of that branch (spawn
    /// succeeds / spawn fails) are reachable deterministically on any machine,
    /// whether or not a real `systemd-run` is on PATH.
    #[cfg(test)]
    pub fn with_program_override(available: bool, program: impl Into<String>) -> Self {
        Self {
            availability_override: Some(available),
            systemd_run_program: program.into(),
        }
    }
}

impl Default for SystemdScopeLauncher {
    fn default() -> Self {
        Self::new()
    }
}

impl AppLauncher for SystemdScopeLauncher {
    fn launch<'a>(
        &self,
        argv: &'a [String],
        env: &'a HashMap<String, String>,
        current_dir: Option<&'a str>,
        scope_name: &'a str,
    ) -> Result<(), String> {
        let systemd_run_available = self
            .availability_override
            .unwrap_or_else(|| which::which(&self.systemd_run_program).is_ok());

        if systemd_run_available {
            // With `--scope`, systemd-run execs the target in place, so this `Child`
            // handle *is* the launched app for its whole lifetime — still our direct
            // child (the scope changes its cgroup, not its parent pid). It must be
            // reaped or it becomes a zombie the moment the user closes the app.
            if let Ok(child) = spawn_systemd_run(
                &self.systemd_run_program,
                scope_name,
                argv,
                env,
                current_dir,
            ) {
                reap_in_background(child);
                return Ok(());
            }
        }

        spawn_fallback(argv, env, current_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_marker(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "stratos_bar_systemd_launcher_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    /// Writes an executable stand-in for `systemd-run` into `dir`. The script records
    /// its own pid and the argv it was handed, then exits immediately — standing in for
    /// the real thing exec'ing the target app and later being closed by the user.
    fn write_fake_systemd_run(
        dir: &std::path::Path,
        pid_marker: &std::path::Path,
        argv_marker: &std::path::Path,
    ) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let script = dir.join("systemd-run");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s' \"$*\" > {}\necho $$ > {}\n",
                argv_marker.display(),
                pid_marker.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    /// `Z` if `pid` is an un-reaped zombie, another state char if it is still alive,
    /// `None` once it has been reaped (or never existed).
    fn process_state(pid: &str) -> Option<char> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // Field 3 is the state, but field 2 (comm) is parenthesised and may itself
        // contain spaces, so scan past its closing paren first.
        let after_comm = stat.rsplit_once(')')?.1;
        after_comm.split_whitespace().next()?.chars().next()
    }

    fn wait_until_reaped(pid: &str) -> bool {
        let mut waited = std::time::Duration::from_millis(0);
        let step = std::time::Duration::from_millis(50);
        let timeout = std::time::Duration::from_secs(2);
        loop {
            if process_state(pid).is_none() {
                return true;
            }
            if waited >= timeout {
                return false;
            }
            std::thread::sleep(step);
            waited += step;
        }
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
    fn test_build_systemd_run_argv_matches_expected_shape() {
        let argv = vec![
            "firefox".to_string(),
            "--new-window".to_string(),
            "https://example.com".to_string(),
        ];
        let result = build_systemd_run_argv("stratos-bar-firefox-123-456.scope", &argv);
        assert_eq!(
            result,
            vec![
                "--user".to_string(),
                "--scope".to_string(),
                "--unit=stratos-bar-firefox-123-456.scope".to_string(),
                "--collect".to_string(),
                "--".to_string(),
                "firefox".to_string(),
                "--new-window".to_string(),
                "https://example.com".to_string(),
            ]
        );
    }

    #[test]
    fn test_fallback_path_actually_runs_the_process() {
        let marker = unique_marker("runs");
        let launcher = SystemdScopeLauncher::with_availability_override(false);
        let argv = vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("touch {}", marker.display()),
        ];
        let env: HashMap<String, String> = std::env::vars().collect();

        let result = launcher.launch(&argv, &env, None, "stratos-bar-test.scope");
        assert!(result.is_ok(), "launch failed: {:?}", result.err());
        assert!(wait_for_marker(&marker), "marker file was never created");

        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn test_fallback_path_reparents_to_init() {
        let marker = unique_marker("ppid");
        let launcher = SystemdScopeLauncher::with_availability_override(false);
        let argv = vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("echo $PPID > {}", marker.display()),
        ];
        let env: HashMap<String, String> = std::env::vars().collect();

        let result = launcher.launch(&argv, &env, None, "stratos-bar-test.scope");
        assert!(result.is_ok());
        assert!(wait_for_marker(&marker), "marker file was never created");

        let contents = std::fs::read_to_string(&marker).unwrap();
        let ppid = contents.trim();
        let this_pid = std::process::id().to_string();
        assert!(
            ppid == "1" || ppid != this_pid,
            "expected reparenting to init, got ppid={ppid} this_pid={this_pid}"
        );

        let _ = std::fs::remove_file(&marker);
    }

    /// Drives the *success* arm of the systemd-run branch — the feature's primary path,
    /// which no test reached before — and asserts the spawned process does not survive
    /// as a zombie. With `--scope` the systemd-run process execs the target app in
    /// place, so it stays stratos-bar's direct child for the app's whole lifetime; if
    /// its `Child` handle is dropped without `wait()`, every app the user ever launches
    /// leaves a permanent zombie behind.
    #[test]
    fn test_systemd_run_success_path_reaps_its_child() {
        let dir = tempfile::tempdir().unwrap();
        let pid_marker = unique_marker("systemd_pid");
        let argv_marker = unique_marker("systemd_argv");
        let script = write_fake_systemd_run(dir.path(), &pid_marker, &argv_marker);

        let launcher = SystemdScopeLauncher::with_program_override(true, script.to_string_lossy());
        let argv = vec!["firefox".to_string(), "--new-window".to_string()];
        let env: HashMap<String, String> = std::env::vars().collect();

        let result = launcher.launch(&argv, &env, None, "stratos-bar-test-123-456.scope");
        assert!(result.is_ok(), "launch failed: {:?}", result.err());

        assert!(
            wait_for_marker(&pid_marker),
            "the systemd-run stand-in was never executed — the success arm was not taken"
        );
        assert!(wait_for_marker(&argv_marker));

        let recorded_argv = std::fs::read_to_string(&argv_marker).unwrap();
        assert_eq!(
            recorded_argv.trim(),
            "--user --scope --unit=stratos-bar-test-123-456.scope --collect -- firefox --new-window",
            "systemd-run was invoked with unexpected argv"
        );

        let pid = std::fs::read_to_string(&pid_marker).unwrap();
        let pid = pid.trim().to_string();
        let reaped = wait_until_reaped(&pid);

        let _ = std::fs::remove_file(&pid_marker);
        let _ = std::fs::remove_file(&argv_marker);

        assert!(
            reaped,
            "pid {pid} spawned through the systemd-run path was left un-reaped (state \
             {:?}); the returned Child must be waited on in a detached thread or every \
             launched app becomes a permanent zombie",
            process_state(&pid)
        );
    }

    /// AC 2's second fallback trigger: `systemd-run` is reported available, but spawning
    /// it fails. The launch must still succeed by falling through to the direct spawn.
    #[test]
    fn test_available_but_spawn_failure_falls_back_to_direct_spawn() {
        let marker = unique_marker("spawn_failure");
        let missing = std::env::temp_dir().join("stratos-bar-no-such-systemd-run-binary");
        assert!(!missing.exists());

        let launcher = SystemdScopeLauncher::with_program_override(true, missing.to_string_lossy());
        let argv = vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("touch {}", marker.display()),
        ];
        let env: HashMap<String, String> = std::env::vars().collect();

        let result = launcher.launch(&argv, &env, None, "stratos-bar-test.scope");
        assert!(
            result.is_ok(),
            "launch should fall back when systemd-run itself fails to spawn: {:?}",
            result.err()
        );
        assert!(
            wait_for_marker(&marker),
            "fallback spawn never ran after the systemd-run spawn failure"
        );

        let _ = std::fs::remove_file(&marker);
    }
}
