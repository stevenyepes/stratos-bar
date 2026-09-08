use crate::ports::env_port::EnvironmentPort;
use std::collections::HashMap;

/// Holds the environment snapshot captured before `main.rs` mutates the
/// process environment, so spawned children can be built from it instead of
/// inheriting whatever the process's live environment has become.
pub struct ProcessEnvironment {
    vars: HashMap<String, String>,
}

impl ProcessEnvironment {
    pub fn new(vars: HashMap<String, String>) -> Self {
        Self { vars }
    }
}

impl EnvironmentPort for ProcessEnvironment {
    fn snapshot(&self) -> HashMap<String, String> {
        self.vars.clone()
    }
}
