use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub kind: String,    // "app", "script", "window", "file"
    pub content: String, // The executable path, script path, or file path
    pub name: String,    // Display name
    pub icon: Option<String>,
    pub last_accessed: u64,
    pub frequency: u64,
}
