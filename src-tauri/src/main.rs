// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;

/// Captures the process environment as it stands right now. Must run before
/// any `std::env::set_var` call in `main()` — later code mutates the live
/// process environment (see the WebKitGTK workaround below), and every
/// process-spawning site in the app needs a pre-mutation copy so those
/// mutations don't leak into launched children.
fn capture_env_snapshot() -> HashMap<String, String> {
    std::env::vars().collect()
}

fn main() {
    let env_snapshot = capture_env_snapshot();
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    stratos_bar_lib::run(env_snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_excludes_var_set_after_capture() {
        std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER");

        let snapshot = capture_env_snapshot();
        assert!(!snapshot.contains_key("WEBKIT_DISABLE_DMABUF_RENDERER"));

        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        assert_eq!(
            std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").as_deref(),
            Ok("1")
        );

        // The snapshot was captured before the mutation above, so it must
        // still not contain the variable even though the live process does.
        assert!(!snapshot.contains_key("WEBKIT_DISABLE_DMABUF_RENDERER"));

        std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER");
    }
}
