//! Interactive UI responses from the `chat_ui` tool.

use crate::state::AppState;
use tauri::{Manager, State};

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
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    request_id: String,
    approved: bool,
    expected_digest: Option<String>,
    always_allow: Option<bool>,
) -> Result<(), String> {
    require_approval_surface(&window)?;
    let grant_always = approved && always_allow.unwrap_or(false);
    if request_id.starts_with("approval:") {
        let digest = expected_digest
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or("Approval preview digest is required")?;
        let pending_action = if grant_always {
            state.approval_broker.pending_action(&request_id).await
        } else {
            None
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        let now = i64::try_from(now).map_err(|e| e.to_string())?;
        state
            .approval_broker
            .respond(&request_id, digest, approved, now)
            .await?;
        if let Some(tool_name) = pending_action {
            state.approval_broker.grant_always_allow(&tool_name);
            crate::commands::settings::persist_always_allow_tool(window.app_handle(), &tool_name)
                .await;
        }
        return Ok(());
    }
    agentz_task_state::approval::deliver_response(
        &state.permission_responses,
        &request_id,
        approved,
    )
    .await
}

fn require_approval_surface(window: &tauri::WebviewWindow) -> Result<(), String> {
    let source = window.url().map_err(|e| e.to_string())?;
    // URL treats custom schemes as opaque; preserve the verified local Tauri origin.
    let origin = if source.scheme() == "tauri"
        && source.host_str() == Some("localhost")
        && source.port().is_none()
        && source.username().is_empty()
        && source.password().is_none()
    {
        "tauri://localhost".to_string()
    } else {
        source.origin().ascii_serialization()
    };
    agentz_task_state::approval_broker::validate_surface(
        window.label(),
        &origin,
        cfg!(debug_assertions),
    )
}

#[tauri::command]
pub async fn get_permission_preview(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    request_id: String,
) -> Result<serde_json::Value, String> {
    require_approval_surface(&window)?;
    state.approval_broker.preview(&request_id).await
}
