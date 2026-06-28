use crate::domain::apps::{AppEntry, ScoredApp};
use crate::ports::app_port::AppRepository;
use crate::state::AppState;
use crate::utils::fuzzy::rank_apps;
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

pub fn search_apps_logic(repo: &dyn AppRepository, query: &str, limit: usize) -> Vec<ScoredApp> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    match repo.list_apps() {
        Ok(apps) => rank_apps(&apps, query, limit),
        Err(_) => Vec::new(),
    }
}

#[tauri::command]
pub async fn search_apps(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<Vec<ScoredApp>, String> {
    Ok(search_apps_logic(&*state.app_repository, &query, limit))
}

#[tauri::command]
pub async fn set_custom_app_dirs(
    state: State<'_, AppState>,
    paths: Vec<PathBuf>,
) -> Result<(), String> {
    state.app_repository.set_custom_paths(paths);
    Ok(())
}

pub fn parse_exec_command(exec_cmd: &str) -> Option<(String, Vec<String>)> {
    let cleaned = exec_cmd
        .replace("%f", "")
        .replace("%F", "")
        .replace("%u", "")
        .replace("%U", "")
        .replace("%i", "")
        .replace("%c", "")
        .replace("%k", "")
        .replace("%d", "")
        .replace("%D", "")
        .replace("%n", "")
        .replace("%N", "")
        .replace("%v", "")
        .replace("%m", "")
        .replace("%M", "");

    let parts = shell_words::split(&cleaned).ok()?;
    if parts.is_empty() {
        return None;
    }

    let cmd = parts[0].clone();
    let args = parts[1..].to_vec();
    Some((cmd, args))
}

#[tauri::command]
pub async fn launch_app(exec_cmd: String) -> Result<(), String> {
    let (cmd, args) = parse_exec_command(&exec_cmd).ok_or_else(|| "Empty command".to_string())?;

    if !cmd.contains('/') && which::which(&cmd).is_err() {
        return Err(format!("Command not found in PATH: {cmd}"));
    }

    std::process::Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::apps::{AppSource, MatchedField};
    use crate::ports::app_port::MockAppRepository;

    fn fixture(name: &str) -> AppEntry {
        AppEntry {
            id: name.to_lowercase(),
            name: name.to_string(),
            generic_name: None,
            description: None,
            keywords: Vec::new(),
            exec: name.to_lowercase(),
            try_exec: None,
            icon: None,
            categories: Vec::new(),
            startup_wm_class: None,
            source: AppSource::Desktop,
            path: format!("/tmp/{name}.desktop"),
        }
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
    fn test_search_apps_logic_ranks_matches() {
        let mut mock = MockAppRepository::new();
        mock.expect_list_apps().times(1).returning(|| {
            Ok(vec![
                fixture("Chrome"),
                fixture("Chromium"),
                fixture("Figma"),
                fixture("Cursor"),
            ])
        });

        let result = search_apps_logic(&mock, "chr", 10);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].app.name, "Chrome");
        assert_eq!(result[1].app.name, "Chromium");
        assert_eq!(result[0].matched_field, MatchedField::NamePrefix);
    }

    #[test]
    fn test_search_apps_logic_respects_limit() {
        let mut mock = MockAppRepository::new();
        mock.expect_list_apps().times(1).returning(|| {
            Ok((0..10).map(|i| fixture(&format!("App{i}"))).collect())
        });

        let result = search_apps_logic(&mock, "app", 3);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_search_apps_logic_empty_query_returns_empty() {
        let mut mock = MockAppRepository::new();
        mock.expect_list_apps().times(0..=1).returning(|| Ok(vec![]));

        let result = search_apps_logic(&mock, "", 10);
        assert!(result.is_empty());

        let result = search_apps_logic(&mock, "   ", 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_search_apps_logic_returns_empty_when_repository_fails() {
        let mut mock = MockAppRepository::new();
        mock.expect_list_apps()
            .times(1)
            .returning(|| Err("boom".to_string()));

        let result = search_apps_logic(&mock, "chr", 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_exec_command() {
        assert_eq!(
            parse_exec_command("firefox"),
            Some(("firefox".to_string(), vec![]))
        );
        assert_eq!(
            parse_exec_command("echo hello world"),
            Some((
                "echo".to_string(),
                vec!["hello".to_string(), "world".to_string()]
            ))
        );
        assert_eq!(
            parse_exec_command("vlc %U"),
            Some(("vlc".to_string(), vec![]))
        );
        assert_eq!(
            parse_exec_command("grep \"hello world\" file.txt"),
            Some((
                "grep".to_string(),
                vec!["hello world".to_string(), "file.txt".to_string()]
            ))
        );
        assert_eq!(parse_exec_command(""), None);
    }
}
