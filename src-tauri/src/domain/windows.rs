use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct WindowEntry {
    pub title: String,
    pub class: String,
    pub address: String, // ID or Address
    pub icon: Option<String>,
}
