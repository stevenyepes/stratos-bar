use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableKind {
    App,
    Script,
    AiTool,
    RecentFile,
    Shortcut,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiscoverableLaunch {
    App { exec: String },
    Script { path: String, args: Option<String> },
    AiTool { tool_id: String },
    RecentFile { path: String },
    Shortcut { target: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscoverableItem {
    pub kind: DiscoverableKind,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub keywords: Vec<String>,
    pub score: f32,
    pub source: String,
    pub launch: DiscoverableLaunch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_serializes_as_snake_case() {
        let json = serde_json::to_string(&DiscoverableKind::RecentFile).unwrap();
        assert_eq!(json, "\"recent_file\"");
        let json = serde_json::to_string(&DiscoverableKind::AiTool).unwrap();
        assert_eq!(json, "\"ai_tool\"");
    }

    #[test]
    fn launch_serializes_with_tag_and_snake_case_fields() {
        let launch = DiscoverableLaunch::App {
            exec: "firefox".to_string(),
        };
        let json = serde_json::to_string(&launch).unwrap();
        assert!(json.contains("\"type\":\"app\""));
        assert!(json.contains("\"exec\":\"firefox\""));

        let launch = DiscoverableLaunch::RecentFile {
            path: "/home/me/note.txt".to_string(),
        };
        let json = serde_json::to_string(&launch).unwrap();
        assert!(json.contains("\"type\":\"recent_file\""));
        assert!(json.contains("\"path\":\"/home/me/note.txt\""));
    }

    #[test]
    fn discoverable_item_round_trips_via_json() {
        let item = DiscoverableItem {
            kind: DiscoverableKind::Script,
            id: "deploy".to_string(),
            name: "deploy".to_string(),
            description: Some("deploy to staging".to_string()),
            icon: None,
            keywords: vec!["deploy".to_string()],
            score: 1.23,
            source: "config.script".to_string(),
            launch: DiscoverableLaunch::Script {
                path: "/srv/scripts/deploy.sh".to_string(),
                args: Some("--staging".to_string()),
            },
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: DiscoverableItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.kind, DiscoverableKind::Script);
        assert_eq!(parsed.id, "deploy");
        match parsed.launch {
            DiscoverableLaunch::Script { path, args } => {
                assert_eq!(path, "/srv/scripts/deploy.sh");
                assert_eq!(args.as_deref(), Some("--staging"));
            }
            _ => panic!("wrong variant"),
        }
    }
}