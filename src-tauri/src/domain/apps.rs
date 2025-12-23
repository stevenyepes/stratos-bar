use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
}
