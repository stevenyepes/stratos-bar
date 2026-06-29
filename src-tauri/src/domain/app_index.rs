use crate::domain::apps::AppSource;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AppIndexStatus {
    pub total_apps: usize,
    pub indexed_at: Option<u64>,
    pub truncated: bool,
    pub disabled_sources: Vec<AppSource>,
    pub sources_with_counts: Vec<(AppSource, usize)>,
}

impl AppIndexStatus {
    pub fn is_empty(&self) -> bool {
        self.total_apps == 0
    }

    pub fn is_ready(&self) -> bool {
        self.total_apps > 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AliasEntry {
    pub app_id: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexSourceToggle {
    pub source: AppSource,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LaunchError {
    UnvalidatedExec,
    CommandNotFound(String),
    EmptyCommand,
    LaunchFailed(String),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::UnvalidatedExec => {
                write!(f, "exec command was not validated against the canonical index entry")
            }
            LaunchError::CommandNotFound(cmd) => write!(f, "command not found in PATH: {cmd}"),
            LaunchError::EmptyCommand => write!(f, "empty command"),
            LaunchError::LaunchFailed(msg) => write!(f, "launch failed: {msg}"),
        }
    }
}

impl std::error::Error for LaunchError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_status(total: usize) -> AppIndexStatus {
        AppIndexStatus {
            total_apps: total,
            indexed_at: Some(1_700_000_000),
            truncated: false,
            disabled_sources: Vec::new(),
            sources_with_counts: vec![
                (AppSource::Desktop, total),
                (AppSource::Flatpak, 0),
            ],
        }
    }

    #[test]
    fn app_index_status_ready_with_nonzero_count() {
        let status = make_status(42);
        assert_eq!(status.total_apps, 42);
        assert!(status.is_ready());
        assert!(!status.is_empty());
    }

    #[test]
    fn app_index_status_empty_when_count_zero() {
        let status = make_status(0);
        assert_eq!(status.total_apps, 0);
        assert!(status.is_empty());
        assert!(!status.is_ready());
    }

    #[test]
    fn app_index_status_default_is_empty_and_not_ready() {
        let status = AppIndexStatus::default();
        assert!(status.is_empty());
        assert!(!status.is_ready());
        assert_eq!(status.total_apps, 0);
        assert!(status.indexed_at.is_none());
        assert!(!status.truncated);
        assert!(status.disabled_sources.is_empty());
        assert!(status.sources_with_counts.is_empty());
    }

    #[test]
    fn app_index_status_serde_round_trip() {
        let status = make_status(10);
        let json = serde_json::to_string(&status).unwrap();
        let parsed: AppIndexStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn app_index_status_reflects_truncated_flag() {
        let mut status = make_status(2000);
        status.truncated = true;
        assert!(status.truncated);
        assert!(status.is_ready());
    }

    #[test]
    fn alias_entry_serde_round_trip() {
        let entry = AliasEntry {
            app_id: "google-chrome".to_string(),
            aliases: vec!["browser".to_string(), "g".to_string()],
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: AliasEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, entry);
    }

    #[test]
    fn index_source_toggle_serde_round_trip() {
        let toggle = IndexSourceToggle {
            source: AppSource::Flatpak,
            enabled: false,
        };
        let json = serde_json::to_string(&toggle).unwrap();
        let parsed: IndexSourceToggle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, toggle);
    }

    #[test]
    fn launch_error_unvalidated_exec_is_constructible() {
        let err = LaunchError::UnvalidatedExec;
        let formatted = format!("{err}");
        assert!(formatted.to_lowercase().contains("validated"));
    }

    #[test]
    fn launch_error_unvalidated_exec_pattern_matchable() {
        let err = LaunchError::UnvalidatedExec;
        let matched = match err {
            LaunchError::UnvalidatedExec => true,
            LaunchError::CommandNotFound(_) => false,
            LaunchError::EmptyCommand => false,
            LaunchError::LaunchFailed(_) => false,
        };
        assert!(matched);
    }

    #[test]
    fn launch_error_variants_compare_distinctly() {
        let a = LaunchError::UnvalidatedExec;
        let b = LaunchError::UnvalidatedExec;
        assert_eq!(a, b);

        let c = LaunchError::CommandNotFound("x".to_string());
        assert_ne!(a, c);
    }
}
