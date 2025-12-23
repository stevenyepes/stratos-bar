pub trait IconResolver: Send + Sync {
    fn resolve_icon(&self, icon_name: &str) -> Option<String>;
}
