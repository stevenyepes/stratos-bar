use crate::domain::ai::Message;
use crate::state::AppState;
use futures_util::StreamExt;
use tauri::{Emitter, State};

#[tauri::command]
pub async fn ask_ai(
    window: tauri::Window,
    state: State<'_, AppState>,
    messages: Vec<Message>,
) -> Result<(), String> {
    let config = state.config_service.load_config();

    window
        .emit("ai-response-start", ())
        .map_err(|e| e.to_string())?;

    match state.ai_service.stream_completion(&config, messages).await {
        Ok(mut stream) => {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(content) => {
                        window
                            .emit("ai-response-chunk", content)
                            .map_err(|e| e.to_string())?;
                    }
                    Err(e) => {
                        window
                            .emit("ai-response-error", e.clone())
                            .map_err(|e1| e1.to_string())?;
                        return Err(e);
                    }
                }
            }
        }
        Err(e) => {
            window
                .emit("ai-response-error", e.clone())
                .map_err(|e1| e1.to_string())?;
            return Err(e);
        }
    }

    window
        .emit("ai-response-done", ())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn check_ai_connection(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config_service.load_config();
    state.ai_service.check_connection(&config).await
}

#[tauri::command]
pub async fn list_ollama_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let config = state.config_service.load_config();
    match state.ai_service.list_models(&config).await {
        Ok(models) => Ok(models),
        Err(_) => Ok(vec![]), // Graceful fallback
    }
}
