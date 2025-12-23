use crate::domain::windows::WindowEntry;

pub trait WindowService: Send + Sync {
    fn list_windows(&self) -> Result<Vec<WindowEntry>, String>;
    fn focus_window(&self, id: &str) -> Result<(), String>;
}
