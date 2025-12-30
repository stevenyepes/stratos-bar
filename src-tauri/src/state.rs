use crate::ports::ai_port::AiService;
use crate::ports::app_port::AppRepository;
use crate::ports::config_port::ConfigService;
use crate::ports::history::HistoryRepository;
use crate::ports::icon_port::IconResolver;
use crate::ports::window_port::WindowService;
use std::sync::Arc;

pub struct AppState {
    pub app_repository: Arc<dyn AppRepository>,
    pub window_service: Arc<dyn WindowService>,
    pub config_service: Arc<dyn ConfigService>,
    pub icon_resolver: Arc<dyn IconResolver>,
    pub ai_service: Arc<dyn AiService>,
    pub history_repository: Arc<dyn HistoryRepository>,
}
