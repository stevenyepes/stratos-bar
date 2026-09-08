use std::collections::HashMap;

#[cfg_attr(test, mockall::automock)]
pub trait EnvironmentPort: Send + Sync {
    fn snapshot(&self) -> HashMap<String, String>;
}
