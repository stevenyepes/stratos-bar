use crate::domain::translation::{TranslationRequest, TranslationResult};
use crate::state::AppState;
use tauri::{command, State};

#[command]
pub async fn translate(
    state: State<'_, AppState>,
    text: String,
    source_lang: Option<String>,
    target_lang: String,
) -> Result<TranslationResult, String> {
    let request = TranslationRequest {
        text,
        source_lang,
        target_lang,
    };

    state
        .translation_service
        .translate(request)
        .await
        .map_err(|e| e.to_string())
}
