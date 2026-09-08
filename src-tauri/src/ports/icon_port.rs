pub trait IconResolver: Send + Sync {
    fn resolve_icon(&self, icon_name: &str, size: u16, scale: u16) -> Option<String>;
}
