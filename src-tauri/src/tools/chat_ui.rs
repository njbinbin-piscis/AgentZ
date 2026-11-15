use async_trait::async_trait;
use piscis_kernel::agent::messages::AgentEvent;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::chat::CHAT_EVENT;
use crate::state::AppState;

use super::chat_ui_schema;

pub struct ChatUiTool {
    pub app: AppHandle,
}

pub fn render_interactive_response_result(values: &Value) -> String {
    let json = serde_json::to_string_pretty(values).unwrap_or_else(|_| format!("{:?}", values));
    let action_type = values
        .get("__action_type__")
        .and_then(|v| v.as_str())
        .unwrap_or("submit");
    let hint = if action_type == "action" {
        "This is a non-terminal action. You may call chat_ui_patch to update the card and chat_ui_listen before expecting final submit."
    } else {
        "This is the user's final structured input from the interactive card."
    };
    format!(
        "USER_INTERACTIVE_RESPONSE_JSON:\n{}\n\n\
{hint} \
You MUST treat field ids, __data_model__, and __action__ as authoritative. \
Override any prior defaults, assumptions, examples, or tentative plans that conflict with this response. \
If a field is present here, use this submitted value exactly unless the user later changes it.",
        json
    )
}

/// The definition to show for a `chat_ui` call: repaired so every option can be
/// selected, or an error for the model when the form could not be answered.
pub fn prepare_ui_definition(input: &Value) -> Result<Value, String> {
    let mut ui_def = input.get("ui_definition").cloned().unwrap_or(Value::Null);
    if ui_def.is_null() || ui_def.get("blocks").is_none() {
        return Err("ui_definition must contain a 'blocks' array.".to_string());
    }
    let unanswerable = chat_ui_schema::normalize_ui_definition(&mut ui_def);
    if !unanswerable.is_empty() {
        return Err(chat_ui_schema::unanswerable_fields_error(&unanswerable));
    }
    Ok(ui_def)
}

pub fn emit_interactive_event(app: &AppHandle, session_id: &str, event: AgentEvent) {
    if let Ok(payload) = serde_json::to_value(&event) {
        let _ = app.emit(
            CHAT_EVENT,
            json!({ "sessionId": session_id, "channel": "agent_event", "payload": payload }),
        );
    }
}

#[async_trait]
impl Tool for ChatUiTool {
    fn name(&self) -> &str {
        "chat_ui"
    }

    fn description(&self) -> &str {
        "Display an interactive UI card (Chat UI Protocol v2; v1 compatible) for structured user input. \
         Supports layout (row/column/card), image/code_preview/progress/link_list, wizard steps, file_picker, \
         text/number/date/time/slider/switch, select/radio/checkbox/tags, koi/project pickers, show_when, validation. \
         Buttons: emit=submit (terminal) or emit=action (non-terminal; then use chat_ui_patch + chat_ui_listen). \
         Include data{} for v2 data model. NOT for simple yes/no. Blocks until submit (or action+listen flow)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["ui_definition"],
            "properties": {
                "ui_definition": chat_ui_schema::ui_definition_schema()
            }
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    /// Waits for the user up to its own 5-minute deadline; the loop's default
    /// 150s tool cap would end the call early and leave the card orphaned.
    fn manages_own_timeout(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let ui_def = match prepare_ui_definition(&input) {
            Ok(def) => def,
            Err(message) => return Ok(ToolResult::err(message)),
        };

        let request_id = ctx
            .tool_use_id
            .clone()
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();

        {
            let state = self.app.state::<AppState>();
            let mut map = state.interactive_responses.lock().await;
            map.insert(request_id.clone(), resp_tx);
        }

        emit_interactive_event(
            &self.app,
            &ctx.session_id,
            AgentEvent::InteractiveUi {
                request_id: request_id.clone(),
                ui_definition: ui_def.clone(),
            },
        );

        match tokio::time::timeout(std::time::Duration::from_secs(300), resp_rx).await {
            Ok(Ok(values)) => Ok(ToolResult::ok(render_interactive_response_result(&values))),
            Ok(Err(_)) => Ok(ToolResult::err(
                "Interactive UI response channel was dropped (user may have navigated away).",
            )),
            Err(_) => {
                let state = self.app.state::<AppState>();
                let mut map = state.interactive_responses.lock().await;
                map.remove(&request_id);
                Ok(ToolResult::err(
                    "Interactive UI timed out after 5 minutes with no user response.",
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn interactive_response_result_is_authoritative_and_machine_readable() {
        let result = render_interactive_response_result(&json!({
            "game_type": "puzzle",
            "project_name": "timy",
            "__data_model__": { "game_type": "puzzle" },
            "__action_type__": "submit"
        }));

        assert!(result.contains("USER_INTERACTIVE_RESPONSE_JSON"));
        assert!(result.contains("\"game_type\": \"puzzle\""));
        assert!(result.contains("authoritative"));
    }

    #[test]
    fn action_response_hints_patch_listen() {
        let result = render_interactive_response_result(&json!({
            "__action_type__": "action",
            "__action__": "preview"
        }));
        assert!(result.contains("chat_ui_patch"));
        assert!(result.contains("chat_ui_listen"));
    }

    #[test]
    fn malformed_options_are_repaired_before_the_card_is_shown() {
        // The shapes seen in the field: numeric values, a label with no value.
        let def = prepare_ui_definition(&json!({
            "ui_definition": {
                "blocks": [{
                    "type": "radio",
                    "id": "plan",
                    "required": true,
                    "default": 2,
                    "options": [
                        { "value": 1, "label": "Basic" },
                        { "value": 2, "label": "Pro" },
                        { "label": "Enterprise" }
                    ]
                }]
            }
        }))
        .expect("a repairable form is shown, not rejected");
        let options = def["blocks"][0]["options"].as_array().unwrap();
        let values: Vec<&str> = options.iter().map(|o| o["value"].as_str().unwrap()).collect();
        assert_eq!(values, ["1", "2", "Enterprise"]);
        assert_eq!(def["blocks"][0]["default"], json!("2"));
    }

    #[test]
    fn choice_field_without_options_is_reported_to_the_model() {
        for options in [json!([]), json!(null), json!([{}, { "value": "  " }])] {
            let err = prepare_ui_definition(&json!({
                "ui_definition": {
                    "blocks": [
                        { "type": "text_input", "id": "name" },
                        { "type": "select", "id": "region", "required": true, "options": options }
                    ]
                }
            }))
            .expect_err("a select nobody can answer must not be shown");
            assert!(err.contains("region (select)"), "{err}");
            assert!(err.contains("options"), "{err}");
        }
    }

    #[test]
    fn missing_blocks_is_still_rejected() {
        let err = prepare_ui_definition(&json!({ "ui_definition": { "title": "x" } })).unwrap_err();
        assert!(err.contains("'blocks'"));
        assert!(prepare_ui_definition(&json!({})).is_err());
    }
}
