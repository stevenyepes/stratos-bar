use std::collections::HashMap;

#[cfg_attr(test, mockall::automock)]
pub trait AppLauncher: Send + Sync {
    /// Prefers `systemd-run --user --scope` so the launched app gets its own cgroup
    /// instead of staying in stratos-bar's; falls back to a hardened direct spawn
    /// (setsid + double-fork) when systemd-run is unavailable or its own process fails
    /// to start. Fire-and-forget — never blocks on the launched process's exit, though
    /// implementations must still reap any process they spawn directly so it cannot
    /// linger as a zombie.
    fn launch<'a>(
        &self,
        argv: &'a [String],
        env: &'a HashMap<String, String>,
        current_dir: Option<&'a str>,
        scope_name: &'a str,
    ) -> Result<(), String>;
}
