use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use tauri::{Emitter, Manager};
use tokio::sync::{mpsc, Mutex};
use tracing::Instrument;

use crate::commands::chat::FrontendAttachment;
use crate::commands::data_scope::resolve_global_config_dir;
use crate::commands::session::{first_chat_round_for_title, sanitize_llm_session_title};
use piscis_core::host::{EventSink, HeadlessCliMode, HeadlessCliResponse};
use piscis_kernel::agent::messages::AgentEvent;
use piscis_kernel::agent::plan::PlanStore;
use piscis_kernel::llm::{self, ContentBlock, LlmMessage, LlmRequest, MessageContent};
use piscis_kernel::store::settings::Settings;

use super::tool_approval::{
    resolve_llm_runtime, restore_settings, LlmRuntime, SettingsSnapshot,
};
use super::SessionRunningGuard;

pub fn model_supports_vision(provider: &str, model: &str) -> bool {
    let m = model.to_lowercase();
    let p = provider.to_lowercase();
    if p == "openai" || p.contains("openai") {
        return m.contains("gpt-4o")
            || m.contains("gpt-4-vision")
            || m.contains("gpt-4-turbo")
            || m.contains("o1")
            || m.contains("o3")
            || m.contains("o4");
    }
    if p == "anthropic" || p.contains("claude") || m.contains("claude") {
        return m.contains("claude-3")
            || m.contains("claude-4")
            || m.contains("claude-opus")
            || m.contains("claude-sonnet")
            || m.contains("claude-haiku");
    }
    if p == "qwen" || p == "tongyi" || p.contains("qwen") {
        return m.contains("qwen-vl")
            || m.contains("qwen2-vl")
            || m.contains("qwen2.5-vl")
            || m.contains("qwen3-vl")
            || m.contains("qvq")
            || m.contains("qwen-omni")
            || m.contains("vl")
            || m.contains("vision");
    }
    if p == "kimi" || p == "moonshot" {
        return m.contains("vision") || m.contains("vl");
    }
    if p == "zhipu" {
        return m.contains("vision") || m.contains("vl") || m.contains("glm-4v");
    }
    if p == "minimax" {
        return m.contains("vision") || m.contains("vl");
    }
    false
}

pub fn resolve_attachment(
    prompt: &str,
    attachment: Option<FrontendAttachment>,
    vision_capable: bool,
) -> Result<(String, Option<(String, String)>)> {
    let Some(att) = attachment else {
        return Ok((prompt.to_string(), None));
    };

    if att.media_type.starts_with("image/") {
        if vision_capable {
            let data_b64 = if let Some(data) = att.data.filter(|d| !d.is_empty()) {
                data
            } else if let Some(path) = att.path.filter(|p| !p.is_empty()) {
                let bytes = std::fs::read(&path)
                    .with_context(|| format!("failed to read attachment at {path}"))?;
                base64::engine::general_purpose::STANDARD.encode(bytes)
            } else {
                return Err(anyhow!("image attachment requires data or path"));
            };
            return Ok((prompt.to_string(), Some((att.media_type, data_b64))));
        }

        let path_str = if let Some(p) = att.path.filter(|p| !p.is_empty()) {
            p
        } else if let Some(b64) = att.data.filter(|d| !d.is_empty()) {
            let ext = match att.media_type.as_str() {
                "image/png" => "png",
                "image/gif" => "gif",
                "image/webp" => "webp",
                _ => "jpg",
            };
            let default_fname = format!("attachment.{ext}");
            let fname = att.filename.as_deref().unwrap_or(&default_fname);
            let tmp = std::env::temp_dir().join(fname);
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .context("invalid base64 image data")?;
            std::fs::write(&tmp, &bytes).context("failed to write temp attachment")?;
            tmp.to_string_lossy().to_string()
        } else {
            String::new()
        };

        let effective = if path_str.is_empty() {
            prompt.to_string()
        } else if prompt.trim().is_empty() {
            format!("[Image saved to: {path_str}]")
        } else {
            format!("{prompt}\n[Attached image: {path_str}]")
        };
        return Ok((effective, None));
    }

    let path_str = if let Some(p) = att.path.filter(|p| !p.is_empty()) {
        p
    } else if let Some(b64) = att.data.filter(|d| !d.is_empty()) {
        let fname = att.filename.as_deref().unwrap_or("attachment.bin");
        let tmp = std::env::temp_dir().join(fname);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .context("invalid base64 attachment data")?;
        std::fs::write(&tmp, &bytes).context("failed to write temp attachment")?;
        tmp.to_string_lossy().to_string()
    } else {
        String::new()
    };

    let label = att.filename.as_deref().unwrap_or(&path_str);
    let effective = if path_str.is_empty() {
        prompt.to_string()
    } else if prompt.trim().is_empty() {
        format!("[Attached file: {label} at {path_str}]")
    } else {
        format!("{prompt}\n[Attached file: {label} at {path_str}]")
    };
    Ok((effective, None))
}

pub(super) fn inject_image_block(messages: &mut [LlmMessage], media_type: &str, data_b64: &str) {
    let image_block = ContentBlock::Image {
        source: llm::ImageSource {
            source_type: "base64".to_string(),
            media_type: media_type.to_string(),
            data: data_b64.to_string(),
        },
    };
    if let Some(last) = messages.last_mut() {
        if last.role == "user" {
            let text = last.content.as_text();
            let blocks = vec![ContentBlock::Text { text }, image_block];
            last.content = MessageContent::Blocks(blocks);
        }
    }
}

const SESSION_TITLE_SUMMARY_SYSTEM: &str =
    "You write very short chat session titles for a sidebar. \
Rules: 4–12 words; same language as the user message; describe the topic; \
no quotes; no trailing period; output ONLY the title on one line.";

fn clip_title_source(text: &str, max_chars: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= max_chars {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(max_chars).collect::<String>())
    }
}

pub(super) async fn maybe_llm_rename_session_title(
    app: &tauri::AppHandle,
    db: &Arc<Mutex<piscis_kernel::store::db::Database>>,
    settings: &Arc<Mutex<Settings>>,
    session_id: &str,
    event_sink: &Arc<dyn EventSink>,
    budget: crate::runtime::task_budget::BudgetScope,
) -> Result<()> {
    let (user_text, assistant_text) = {
        let guard = db.lock().await;
        first_chat_round_for_title(&guard, session_id)
    }
    .ok_or_else(|| anyhow!("session '{session_id}' is not a first chat round"))?;

    let (runtime, read_timeout) = {
        let s = settings.lock().await;
        let flash_id = resolve_global_config_dir(app)
            .ok()
            .and_then(|dir| crate::commands::flash::load_flash_provider_id(&dir));
        let flash = flash_id.filter(|id| s.find_llm_provider(id).is_some());
        let runtime =
            resolve_llm_runtime(&s, flash.as_deref()).or_else(|_| resolve_llm_runtime(&s, None))?;
        (runtime, s.llm_read_timeout_secs.max(30))
    };

    let client = llm::build_client_with_timeout(
        &runtime.provider,
        &runtime.api_key,
        if runtime.base_url.is_empty() {
            None
        } else {
            Some(&runtime.base_url)
        },
        read_timeout,
    );
    let client = budget.guard_with_source(client, "background");

    let user = format!(
        "User message:\n{}\n\nAssistant reply:\n{}\n\nTitle:",
        clip_title_source(&user_text, 600),
        clip_title_source(&assistant_text, 800),
    );
    let req = LlmRequest {
        messages: vec![LlmMessage {
            role: "user".into(),
            content: MessageContent::text(&user),
        }],
        system: Some(SESSION_TITLE_SUMMARY_SYSTEM.to_string()),
        tools: Vec::new(),
        model: runtime.model,
        max_tokens: 64,
        stream: false,
        vision_override: Some(false),
    };

    let resp = client
        .complete(req)
        .await
        .context("flash title summarize failed")?;
    let title = sanitize_llm_session_title(&resp.content);
    if title.is_empty() {
        return Ok(());
    }

    {
        let guard = db.lock().await;
        guard
            .rename_session(session_id, &title)
            .context("rename_session after title summarize failed")?;
    }
    event_sink.emit_session(
        session_id,
        "session_title",
        serde_json::json!({ "title": title }),
    );
    Ok(())
}

pub(super) struct EventCollectorOutput {
    pub streamed_text: String,
    pub stream_error: Option<String>,
    pub tool_calls: usize,
    pub tool_errors: usize,
    pub iterations: u32,
    pub unsaved_tail: String,
    pub durability_failed: bool,
}

pub(super) fn spawn_turn_event_collector(
    mut rx: mpsc::Receiver<AgentEvent>,
    mut durable_stream: Option<crate::runtime::durable_stream::DurableStreamWriter>,
    collector_cancel: Arc<AtomicBool>,
    collector_sink: Arc<dyn EventSink>,
    collector_session: String,
    collector_app: tauri::AppHandle,
    collector_workspace: String,
) -> tokio::task::JoinHandle<EventCollectorOutput> {
    tokio::spawn(
        async move {
            let mut text = String::new();
            let mut tail = String::new();
            let mut errored: Option<String> = None;
            let mut tool_calls = 0usize;
            let mut tool_errors = 0usize;
            let mut iterations = 0u32;
            let mut waited_for_permission = false;
            let mut durability_failed = false;
            let mut tool_inputs: HashMap<String, serde_json::Value> = HashMap::new();
            while let Some(event) = rx.recv().await {
                let payload = match serde_json::to_value(&event) {
                    Ok(payload) => payload,
                    Err(error) => {
                        durability_failed = true;
                        collector_cancel.store(true, Ordering::SeqCst);
                        let message = format!("Cannot record agent event: {error}");
                        collector_sink.emit_session(
                            &collector_session,
                            "agent_lifecycle",
                            serde_json::json!({"state":"failed","error":message}),
                        );
                        errored = Some(message);
                        break;
                    }
                };
                let mut published_durably = false;
                if let Some(writer) = durable_stream.as_mut() {
                    let recorded = writer.record_before_publish(&payload, |committed| {
                        collector_sink.emit_session(
                            &collector_session,
                            "agent_event",
                            committed.clone(),
                        );
                    });
                    let committed_event = match recorded {
                        Ok(event) => event,
                        Err(error) => {
                            durability_failed = true;
                            collector_cancel.store(true, Ordering::SeqCst);
                            let message = format!(
                                "Execution history could not be saved; execution stopped: {error}"
                            );
                            collector_sink.emit_session(
                                &collector_session,
                                "agent_lifecycle",
                                serde_json::json!({"state":"failed","error":message}),
                            );
                            errored = Some(message);
                            break;
                        }
                    };
                    collector_sink.emit_session(
                        &collector_session,
                        "agent_event_committed",
                        serde_json::json!(committed_event),
                    );
                    published_durably = true;
                }
                if let AgentEvent::ToolStart {
                    ref id, ref input, ..
                } = event
                {
                    tool_calls += 1;
                    tool_inputs.insert(id.clone(), input.clone());
                }
                if let AgentEvent::ToolEnd {
                    ref id,
                    ref name,
                    is_error,
                    ..
                } = event
                {
                    if is_error {
                        tool_errors += 1;
                        let safe_tool_name = if name.len() <= 64
                            && name
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
                        {
                            name.as_str()
                        } else {
                            "unknown"
                        };
                        tracing::warn!(tool_name = safe_tool_name, "tool execution failed");
                    }
                    if matches!(name.as_str(), "file_write" | "file_edit") {
                        if let Some(input) = tool_inputs.remove(id) {
                            if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                                let path_norm = path.replace('\\', "/");
                                if !path_norm.is_empty()
                                    && path_norm != "."
                                    && crate::path_filter::should_watch_path(&path_norm)
                                {
                                    let _ = collector_app.emit(
                                        "ide-file-changed",
                                        serde_json::json!({
                                            "project_dir": collector_workspace,
                                            "path": path_norm,
                                            "kind": "modified",
                                        }),
                                    );
                                }
                            }
                        }
                    } else if name.as_str() == "browser" && !is_error {
                        if let Some(input) = tool_inputs.remove(id) {
                            let state = collector_app.state::<crate::state::AppState>();
                            state.browser_activity.mark_browser_tool().await;
                            crate::browser::events::emit_browser_changed(
                                &collector_app,
                                &input,
                                Some(collector_session.as_str()),
                            );
                        }
                    } else {
                        tool_inputs.remove(id);
                    }
                }
                if matches!(event, AgentEvent::TextSegmentStart { .. }) {
                    iterations += 1;
                }
                if matches!(event, AgentEvent::PermissionRequest { .. }) {
                    waited_for_permission = true;
                    collector_sink.emit_session(
                        &collector_session,
                        "agent_lifecycle",
                        serde_json::json!({"state": "waiting_permission"}),
                    );
                } else if waited_for_permission && matches!(event, AgentEvent::ToolEnd { .. }) {
                    collector_sink.emit_session(
                        &collector_session,
                        "agent_lifecycle",
                        serde_json::json!({"state": "executing"}),
                    );
                }
                if !published_durably {
                    collector_sink.emit_session(&collector_session, "agent_event", payload);
                }
                match event {
                    AgentEvent::TextDelta { delta } => {
                        text.push_str(&delta);
                        tail.push_str(&delta);
                    }
                    AgentEvent::ToolStart { .. } | AgentEvent::TextSegmentStart { .. } => {
                        tail.clear();
                    }
                    AgentEvent::Error { message, .. } => {
                        errored = Some(message);
                        break;
                    }
                    AgentEvent::Done { .. } => break,
                    _ => {}
                }
            }
            EventCollectorOutput {
                streamed_text: text,
                stream_error: errored,
                tool_calls,
                tool_errors,
                iterations,
                unsaved_tail: tail,
                durability_failed,
            }
        }
        .instrument(tracing::Span::current()),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn finalize_turn_and_spawn_background(
    app: tauri::AppHandle,
    db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    settings: Arc<Mutex<Settings>>,
    settings_snapshot: Option<SettingsSnapshot>,
    event_sink: Arc<dyn EventSink>,
    plan_store: PlanStore,
    session_id: String,
    run_id: String,
    attempt_id: String,
    workspace_root: String,
    runtime: LlmRuntime,
    budget_scope: crate::runtime::task_budget::BudgetScope,
    max_iterations: u32,
    cancel: Arc<AtomicBool>,
    early_binding: agentz_task_state::approval::Binding,
    cancellation_completion: crate::runtime::cancel_completion::CancellationCompletion,
    durable_run: Option<crate::runtime::durable_run::ManagedRun>,
    plan_attempt_guard: Option<crate::tools::durable_plan_todo::ActiveAttemptGuard>,
    status_guard: SessionRunningGuard,
    ok: bool,
    new_messages: Vec<LlmMessage>,
    error_msg: Option<String>,
    turn_timeout_secs: Option<u64>,
    collector_out: EventCollectorOutput,
) -> Result<HeadlessCliResponse> {
    let EventCollectorOutput {
        streamed_text,
        stream_error,
        tool_calls,
        tool_errors,
        iterations,
        unsaved_tail,
        durability_failed,
    } = collector_out;

    let open_todo_count = {
        let plans = plan_store.lock().await;
        plans
            .get(&session_id)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.status == "pending" || item.status == "in_progress")
                    .count()
            })
            .unwrap_or(0)
    };
    let mut durable_run = durable_run;
    let cancellation_requested = cancel.load(Ordering::SeqCst);
    let mut terminal_err: Option<anyhow::Error> = None;
    let cancellation_unconfirmed = if cancellation_requested {
        if let Some(run) = durable_run.as_mut() {
            if let Err(e) = run.request_cancelling() {
                terminal_err = Some(anyhow::Error::msg(e));
            }
        }
        if terminal_err.is_some() {
            true
        } else {
            match cancellation_completion
                .wait_requested(&early_binding, std::time::Duration::from_secs(10))
                .await
            {
                Ok(decision) => !decision.may_finalize_cancelled(),
                Err(e) => {
                    terminal_err = Some(anyhow::Error::msg(e));
                    true
                }
            }
        }
    } else {
        false
    };
    let turn_failed = error_msg.is_some()
        || stream_error.is_some()
        || cancellation_unconfirmed
        || terminal_err.is_some();
    let effective_timeout = turn_timeout_secs.filter(|_| !durability_failed);

    if (turn_failed || cancel.load(Ordering::SeqCst)) && !unsaved_tail.trim().is_empty() {
        let db = db.lock().await;
        let tail = unsaved_tail.trim();
        let already_saved = db
            .get_messages_latest(&session_id, 4)
            .map(|rows| {
                rows.iter()
                    .any(|r| r.role == "assistant" && r.content.contains(tail))
            })
            .unwrap_or(false);
        if !already_saved {
            if let Err(e) = db.append_message(&session_id, "assistant", tail) {
                tracing::warn!(session_id = %session_id, "failed to persist interrupted reply: {e}");
            }
        }
    }
    drop(plan_attempt_guard);
    if let Some(run) = durable_run {
        if let Err(e) = run.finish(
            cancellation_requested && !durability_failed && !cancellation_unconfirmed,
            turn_failed,
            effective_timeout.is_some() || cancellation_unconfirmed,
        ) {
            let err = anyhow!("failed to persist run terminal state: {e}");
            if terminal_err.is_none() {
                terminal_err = Some(err);
            } else {
                tracing::warn!(session_id = %session_id, "{err}");
            }
        }
    }
    let lifecycle_state = if cancellation_unconfirmed {
        "interrupted"
    } else if effective_timeout.is_some() {
        "timed_out"
    } else if cancellation_requested && !durability_failed {
        "cancelled"
    } else if turn_failed {
        "failed"
    } else if tool_calls == 0 {
        "completed_text_only"
    } else {
        "completed"
    };
    tracing::info!(
        session_id = %session_id,
        lifecycle_state,
        run_id = %run_id,
        attempt_id = %attempt_id,
        tool_calls,
        tool_errors,
        iterations,
        "agent turn finished"
    );
    event_sink.emit_session(
        &session_id,
        "agent_lifecycle",
        serde_json::json!({
            "state": lifecycle_state,
            "tool_calls": tool_calls,
            "tool_errors": tool_errors,
            "iterations": iterations,
            "max_iterations": max_iterations,
            "open_todos": open_todo_count,
        }),
    );
    let final_error = if durability_failed {
        stream_error.as_deref().or(error_msg.as_deref())
    } else {
        error_msg.as_deref().or(stream_error.as_deref())
    };
    if let Some(err) = final_error {
        let message = match effective_timeout {
            Some(secs) => format!("Turn exceeded its {secs}s limit."),
            None => err.to_string(),
        };
        event_sink.emit_session(
            &session_id,
            "agent_final",
            serde_json::json!({
                "ok": false,
                "error": message,
                "timed_out": effective_timeout.is_some(),
                "open_todos": open_todo_count,
            }),
        );
    } else {
        event_sink.emit_session(&session_id, "agent_final", serde_json::json!({"ok": true}));
    }

    status_guard
        .finish(turn_failed || terminal_err.is_some())
        .await;
    if let Some(err) = terminal_err {
        return Err(err);
    }

    if !turn_failed {
        let db_bg = db.clone();
        let settings_bg = settings.clone();
        let app_bg = app.clone();
        let session_bg = session_id.clone();
        let sink_bg = event_sink.clone();
        let budget_title = budget_scope.clone();
        tokio::spawn(async move {
            if let Err(e) = maybe_llm_rename_session_title(
                &app_bg,
                &db_bg,
                &settings_bg,
                &session_bg,
                &sink_bg,
                budget_title,
            )
            .await
            {
                tracing::debug!("session title summarize skipped: {e}");
            }
        });

        let project_db_ev = db.clone();
        let app_ev = app.clone();
        let session_ev = session_id.clone();
        let msgs_ev = new_messages.clone();
        let provider_ev = runtime.provider.clone();
        let api_key_ev = runtime.api_key.clone();
        let base_url_ev = if runtime.base_url.is_empty() {
            None
        } else {
            Some(runtime.base_url.clone())
        };
        let model_ev = runtime.model.clone();
        let max_tokens_ev = runtime.max_tokens;
        let budget_post = budget_scope.clone();
        let ws_root_ev = workspace_root.clone();
        tokio::spawn(async move {
            crate::commands::post_turn::auto_extract_repo_notes(
                ws_root_ev,
                msgs_ev.clone(),
                provider_ev.clone(),
                api_key_ev.clone(),
                base_url_ev.clone(),
                model_ev.clone(),
                max_tokens_ev,
                budget_post.clone(),
            )
            .await;
            crate::commands::post_turn::run_post_turn_hooks(
                &app_ev,
                project_db_ev,
                session_ev,
                msgs_ev,
                provider_ev,
                api_key_ev,
                base_url_ev,
                model_ev,
                max_tokens_ev,
                budget_post,
            )
            .await;
        });
    }

    if let Some(snap) = settings_snapshot {
        let mut s = settings.lock().await;
        restore_settings(&mut s, snap);
    }

    let response_text = if !streamed_text.is_empty() {
        streamed_text
    } else {
        new_messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .map(|m| m.content.as_text())
            .unwrap_or_default()
    };

    let response_error = if durability_failed {
        stream_error.or(error_msg)
    } else {
        error_msg
    };
    if let Some(err) = response_error {
        if effective_timeout.is_some() {
            tracing::warn!(session_id = %session_id, %err, "turn timed out; returning partial result");
            return Ok(HeadlessCliResponse {
                ok: false,
                mode: HeadlessCliMode::Piscis.as_str().to_string(),
                session_id,
                pool_id: None,
                response_text,
                disabled_tools: Vec::new(),
                pool_wait: None,
            });
        }
        return Err(anyhow!(err));
    }

    Ok(HeadlessCliResponse {
        ok: ok && !turn_failed,
        mode: HeadlessCliMode::Piscis.as_str().to_string(),
        session_id,
        pool_id: None,
        response_text,
        disabled_tools: Vec::new(),
        pool_wait: None,
    })
}
