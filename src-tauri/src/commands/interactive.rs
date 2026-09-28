//! Interactive UI responses from the `chat_ui` tool.

use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn respond_interactive_ui(
    state: State<'_, AppState>,
    request_id: String,
    values: serde_json::Value,
) -> Result<(), String> {
    let mut map = state.interactive_responses.lock().await;
    if let Some(tx) = map.remove(&request_id) {
        let _ = tx.send(values);
        Ok(())
    } else {
        Err("Interactive UI request not found or expired".into())
    }
}

/// Resolve a one-shot permission request emitted by the Agent harness.
#[tauri::command]
pub async fn respond_permission_request(
    state: State<'_, AppState>,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    let mut map = state.permission_responses.lock().await;
    if let Some(tx) = map.remove(&request_id) {
        let _ = tx.send(approved);
        Ok(())
    } else {
        Err("Permission request not found or expired".into())
    }
}
