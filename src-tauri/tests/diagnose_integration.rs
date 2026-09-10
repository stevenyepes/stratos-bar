//! Integration test for `--diagnose`: spawns the real built binary (never the
//! library directly) so a regression that only shows up at the process
//! boundary -- a section dropped from `format()`, a panic on missing env, a
//! window that actually opens and blocks -- fails here even if every unit
//! test in `diagnose.rs` still passes.
//!
//! A real Tauri window blocks forever inside `.run()`'s event loop, so a
//! bounded, clean process return is the provable proxy for "did not open a
//! window" (AC1). The wait uses a detached thread + `mpsc::Receiver::recv_timeout`
//! per the spec's constraint against adding a new dependency for this.

use std::collections::HashMap;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const SPAWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Spawns `stratos-bar --diagnose` with the given env removed/added on top of
/// the test process's own environment, and waits up to `SPAWN_TIMEOUT` for it
/// to exit. Panics (failing the test) if the process is still running when
/// the timeout elapses -- that would mean it fell through to `run()` and
/// opened (or tried to open) a window instead of short-circuiting.
fn run_diagnose(remove_env: &[&str], extra_env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stratos-bar"));
    cmd.arg("--diagnose")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for key in remove_env {
        cmd.env_remove(key);
    }
    for (key, value) in extra_env {
        cmd.env(key, value);
    }

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = cmd.output();
        // The receiver may already have timed out and been dropped; that's
        // fine, there's nothing left to report the result to.
        let _ = tx.send(result);
    });

    match rx.recv_timeout(SPAWN_TIMEOUT) {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => panic!("failed to spawn --diagnose process: {err}"),
        Err(_) => panic!(
            "--diagnose process did not exit within {SPAWN_TIMEOUT:?}; \
             a real Tauri window would block here, so this is the timeout \
             AC1 is supposed to catch"
        ),
    }
}

#[test]
fn diagnose_exits_cleanly_and_reports_every_section() {
    let output = run_diagnose(&[], &[]);

    assert!(
        output.status.success(),
        "expected exit code 0, got {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    let required_sections: HashMap<&str, &str> = HashMap::from([
        ("GPU/quirk", "=== GPU ==="),
        ("quirk action", "=== WebKit/NVIDIA quirk ==="),
        ("session", "=== Session ==="),
        ("launcher backend", "=== Launcher backend ==="),
        ("terminal", "=== Terminal ==="),
        ("icon theme", "Icon theme:"),
        ("app count", "App count:"),
        ("env-delta header", "=== Environment delta (names only) ==="),
    ]);

    for (label, needle) in required_sections {
        assert!(
            stdout.contains(needle),
            "expected {label} section (looking for {needle:?}) missing from --diagnose stdout:\n{stdout}"
        );
    }
}

#[test]
fn diagnose_with_no_session_hints_reports_unknown_session_and_never_panics() {
    // GDK_BACKEND is removed too, on top of the three the ticket names, so
    // the assertion on "Type: Unknown" below is deterministic regardless of
    // what the host running this test happens to have set -- detect_session_type_verbose
    // checks GDK_BACKEND before any of the other three.
    let output = run_diagnose(
        &[
            "XDG_SESSION_TYPE",
            "WAYLAND_DISPLAY",
            "DISPLAY",
            "GDK_BACKEND",
        ],
        &[],
    );

    assert!(
        output.status.success(),
        "expected exit code 0 even with no session-type env hints, got {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Type: Unknown"),
        "expected unresolved session type to report as Unknown:\n{stdout}"
    );
}

#[test]
fn diagnose_never_leaks_a_planted_secret_value_to_stdout() {
    let secret_value = "ghp_fake_token_value";
    let output = run_diagnose(&[], &[("STRATOS_BAR_TEST_SECRET", secret_value)]);

    assert!(
        output.status.success(),
        "expected exit code 0, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(secret_value),
        "planted secret value leaked into --diagnose stdout:\n{stdout}"
    );
}
