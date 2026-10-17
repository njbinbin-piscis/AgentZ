//! AI chat command — drives a single agent turn on the shared `piscis-engine`
//! kernel and streams [`AgentEvent`]s back to the frontend.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State, WebviewWindow};

use piscis_core::host::{EventSink, HeadlessCliMode, HeadlessCliRequest};

use crate::commands::chat_turn::run_agentz_turn;
use crate::commands::data_scope::{open_project_kernel_state, require_project_dir};
use crate::commands::session_sources::{default_channel_for, SOURCE_CODEZ};
use crate::state::AppState;

/// Re-export for settings / inline edit (always global config dir).
pub(crate) use crate::commands::data_scope::resolve_global_config_dir as resolve_config_dir;

/// Tauri event channel that carries every streamed kernel event to the UI.
pub const CHAT_EVENT: &str = "agentz:chat-event";

/// Attachment sent from the frontend with a chat message.
#[derive(Debug, Clone, Deserialize)]
pub struct FrontendAttachment {
    pub media_type: String,
    pub path: Option<String>,
    pub data: Option<String>,
    pub filename: Option<String>,
}

/// Bridges the kernel's [`EventSink`] to Tauri events.
struct TauriEventSink {
    app: AppHandle,
}

impl EventSink for TauriEventSink {
    fn emit_session(&self, session_id: &str, event: &str, payload: Value) {
        let _ = self.app.emit(
            CHAT_EVENT,
            json!({ "sessionId": session_id, "channel": event, "payload": payload }),
        );
    }

    fn emit_broadcast(&self, event: &str, payload: Value) {
        let _ = self.app.emit(
            CHAT_EVENT,
            json!({ "sessionId": Value::Null, "channel": event, "payload": payload }),
        );
    }
}

/// Result of a completed chat turn returned to the JS caller.
#[derive(Debug, Serialize)]
pub struct ChatResult {
    pub ok: bool,
    pub session_id: String,
    pub response_text: String,
    /// Journal turn id for this turn — used by the UI Review bar / Undo.
    pub turn_id: Option<String>,
}

// Hold task exclusion until asynchronous cancellation registration cleanup finishes,
// including early-return/aborted future paths.
struct TaskRegistration {
    registration: Option<crate::runtime::chat_cancel::Registration>,
    gate: Option<tokio::sync::OwnedMutexGuard<()>>,
}
impl TaskRegistration {
    async fn release(&mut self) {
        if let Some(registration) = self.registration.as_ref() {
            registration.release().await;
        }
        self.registration.take();
        self.gate.take();
    }
}
impl Drop for TaskRegistration {
    fn drop(&mut self) {
        let registration = self.registration.take();
        let gate = self.gate.take();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if let Some(registration) = registration {
                    registration.release().await;
                }
                drop(gate);
            });
        }
    }
}

/// Reject the legacy clear before stores or queued Run registration.
pub(crate) fn continuation_plan_intent(clear_plan: Option<bool>) -> Result<bool, String> {
    if clear_plan == Some(true) {
        return Err("clear_plan=true is deprecated; continue the existing task plan or use an explicit plan replacement action".into());
    }
    Ok(clear_plan.unwrap_or(false))
}

#[cfg(test)]
mod continuation_intent_tests {
    use super::*;
    #[tokio::test]
    async fn task_registration_drop_keeps_gate_until_cancel_map_cleanup() {
        let root =
            std::env::temp_dir().join(format!("agentz-registration-drop-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let gate = crate::runtime::task_activity::TaskActivity::default();
        let single = Arc::new(tokio::sync::Mutex::new(None));
        let tasks = Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
        let flag = Arc::new(AtomicBool::new(false));
        let registration = crate::runtime::chat_cancel::Registration::new(
            single,
            tasks.clone(),
            Some("run-key".into()),
            flag,
        )
        .await
        .unwrap();
        let owned = TaskRegistration {
            registration: Some(registration),
            gate: Some(gate.try_enter(&root, "task").unwrap()),
        };
        let blocked = tasks.lock().await;
        drop(owned);
        tokio::task::yield_now().await;
        assert!(gate.try_enter(&root, "task").is_err());
        assert!(blocked.contains_key("run-key"));
        drop(blocked);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if let Ok(entered) = gate.try_enter(&root, "task") {
                    assert!(!tasks.lock().await.contains_key("run-key"));
                    drop(entered);
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn task_registration_normal_release_keeps_gate_until_cancel_map_cleanup() {
        let root = std::env::temp_dir().join(format!(
            "agentz-registration-release-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let gate = crate::runtime::task_activity::TaskActivity::default();
        let single = Arc::new(tokio::sync::Mutex::new(None));
        let tasks = Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
        let flag = Arc::new(AtomicBool::new(false));
        let registration = crate::runtime::chat_cancel::Registration::new(
            single,
            tasks.clone(),
            Some("run-key".into()),
            flag,
        )
        .await
        .unwrap();
        let mut owned = TaskRegistration {
            registration: Some(registration),
            gate: Some(gate.try_enter(&root, "task").unwrap()),
        };
        let blocked = tasks.lock().await;
        let release = tokio::spawn(async move {
            owned.release().await;
        });
        tokio::task::yield_now().await;
        assert!(gate.try_enter(&root, "task").is_err());
        assert!(blocked.contains_key("run-key"));
        drop(blocked);
        tokio::time::timeout(std::time::Duration::from_secs(2), release)
            .await
            .unwrap()
            .unwrap();
        assert!(!tasks.lock().await.contains_key("run-key"));
        drop(gate.try_enter(&root, "task").unwrap());
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn omitted_and_explicit_continue_preserve_plan_but_legacy_clear_is_rejected() {
        assert!(!continuation_plan_intent(None).unwrap());
        assert!(!continuation_plan_intent(Some(false)).unwrap());
        let error = continuation_plan_intent(Some(true)).unwrap_err();
        assert!(error.contains("deprecated"));
        assert!(error.contains("explicit plan replacement"));
    }
}

/// Run one agent turn. Streams `AgentEvent`s via `agentz:chat-event`.
#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    mut prompt: String,
    session_id: Option<String>,
    workspace: Option<String>,
    attachment: Option<FrontendAttachment>,
    chat_mode: Option<String>,
    model_id: Option<String>,
    clear_plan: Option<bool>,
    display_prompt: Option<String>,
    project_dir: Option<String>,
    workspace_dir: Option<String>,
    require_task_environment: Option<bool>,
    task_key: Option<String>,
    enabled_skills: Option<Vec<String>>,
    enabled_connectors: Option<Vec<String>>,
    agent_id: Option<String>,
    session_source: Option<String>,
    team_id: Option<String>,
    pool_id: Option<String>,
    prefer_zh: Option<bool>,
    artifact_scope_handle: Option<String>,
    artifact_ref: Option<super::task_artifacts::ArtifactContinuationRef>,
) -> Result<ChatResult, String> {
    let clear_plan = continuation_plan_intent(clear_plan)?;
    // Held before queueing and through the entire turn; RAII also releases on errors.
    let _environment_lease = state.environment_access.execution().await;
    let project = require_project_dir(project_dir.as_deref().or(workspace.as_deref()))?;
    let session_id = Some(
        session_id
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
    );
    let task_activity = state.task_activity.try_enter(
        std::path::Path::new(&project),
        session_id.as_deref().expect("host assigned session"),
    )?;
    // When an isolated worktree is provided, the agent works inside it (and the
    // journal tracks its files) while sessions stay in the main project's DB.
    let agent_workspace = super::agent_task::resolve_task_workspace(
        std::path::Path::new(&project),
        session_id.as_deref(),
        workspace_dir.as_deref(),
        require_task_environment.unwrap_or(false),
    )
    .await?;
    let kernel = open_project_kernel_state(&app, &project)?;
    let config_dir = resolve_config_dir(&app)?;
    let run_id = uuid::Uuid::new_v4().to_string();
    let mut binding = crate::runtime::approval::resolve_binding(
        std::path::Path::new(&project),
        std::path::Path::new(&agent_workspace),
        session_id.as_deref().expect("host assigned session"),
        &run_id,
        require_task_environment.unwrap_or(false),
    )
    .await?;
    binding.attempt_id = uuid::Uuid::new_v4().to_string();

    // Queue behind the concurrency limit. For single (IDE) chats this is a
    // no-op; for a burst of parallel Agent tasks it bounds resource use.
    let cancel = Arc::new(AtomicBool::new(false));
    let task_key = task_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned);
    let registration = crate::runtime::chat_cancel::Registration::new(
        state.chat_cancel.clone(),
        state.task_cancel.clone(),
        task_key,
        cancel.clone(),
    )
    .await?;
    let mut registration = TaskRegistration {
        registration: Some(registration),
        gate: Some(task_activity),
    };
    // Declared after registration so early-drop persists the Run terminal state
    // before releasing registration/task exclusion.
    let queued_run = Some(crate::runtime::durable_run::ManagedRun::queue_bound(
        std::path::Path::new(&project),
        &binding,
    )?);
    let _permit =
        match crate::runtime::chat_cancel::acquire(state.agent_slots.clone(), &cancel).await {
            Ok(permit) => permit,
            Err(error) => {
                if cancel.load(Ordering::SeqCst) {
                    if let Some(run) = queued_run {
                        run.finish(true, false, false)?;
                    }
                } else {
                    // Scheduler failure still owns a queued Run: terminalize it
                    // synchronously before releasing task exclusion.
                    drop(queued_run);
                }
                registration.release().await;
                return Err(error);
            }
        };

    // A queued task may outlive changes to the filesystem or Git checkout.
    // Validate at actual start as well, and reject any intervening retarget.
    let agent_workspace = super::agent_task::resolve_task_workspace(
        std::path::Path::new(&project),
        session_id.as_deref(),
        Some(&agent_workspace),
        require_task_environment.unwrap_or(false),
    )
    .await?;
    let journal = Arc::new(crate::journal::open_project_journal(&agent_workspace)?);

    let sink = Arc::new(TauriEventSink { app: app.clone() });

    let channel = session_source
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_channel_for(SOURCE_CODEZ).to_string());
    if let Some(reference) = artifact_ref.as_ref() {
        if channel != default_channel_for(SOURCE_CODEZ) {
            return Err("artifact continuation requires a CodeZ session".into());
        }
        let handle = artifact_scope_handle
            .as_deref()
            .ok_or("artifact continuation requires a project scope")?;
        let context = super::task_artifacts::resolve_continuation_text(
            &app,
            &window,
            &state,
            handle,
            std::path::Path::new(&project),
            session_id.as_deref().expect("host assigned session"),
            reference,
        )
        .await?;
        prompt.push_str("\n\n");
        prompt.push_str(&context);
    } else if artifact_scope_handle.is_some() {
        return Err("artifact scope supplied without continuation reference".into());
    }
    // CodeZ sessions get a title from the first user message; WorkZ keeps a
    // generic placeholder until maybe_autotitle_session_from_first_prompt runs.
    let session_title = match channel.as_str() {
        "workz-team" => Some("WorkZ team task".to_string()),
        "workz" => Some("WorkZ task".to_string()),
        _ => None,
    };

    let request = HeadlessCliRequest {
        prompt,
        workspace: Some(agent_workspace.clone()),
        mode: HeadlessCliMode::Piscis,
        session_id,
        session_title,
        channel: Some(channel),
        ..Default::default()
    };

    state.browser_activity.set_chat_turn_active(true);

    let result = run_agentz_turn(
        app.clone(),
        request,
        kernel,
        sink,
        state.plan_state.clone(),
        Some(state.permission_responses.clone()),
        project.clone(),
        cancel,
        model_id,
        chat_mode.unwrap_or_else(|| "agent".to_string()),
        attachment,
        clear_plan,
        display_prompt,
        state.lsp_manager.clone(),
        state.browser.clone(),
        journal.clone(),
        config_dir,
        enabled_skills.unwrap_or_default(),
        enabled_connectors,
        agent_id.filter(|a| !a.trim().is_empty()),
        team_id.filter(|s| !s.trim().is_empty()),
        pool_id.filter(|s| !s.trim().is_empty()),
        prefer_zh.unwrap_or(false),
        queued_run,
    )
    .await;

    state.browser_activity.set_chat_turn_active(false);

    registration.release().await;

    let response = result.map_err(|e| format!("agent turn failed: {e}"))?;
    Ok(ChatResult {
        ok: response.ok,
        session_id: response.session_id,
        response_text: response.response_text,
        turn_id: journal.current_turn_id(),
    })
}

/// Number of chat turns currently running (IDE chat + parallel Agent tasks).
#[tauri::command]
pub async fn chat_running_count(state: State<'_, AppState>) -> Result<u32, String> {
    let single = state.chat_cancel.lock().await.is_some() as u32;
    let parallel = state.task_cancel.lock().await.len() as u32;
    Ok(single + parallel)
}

/// Cancel every running turn and wait (bounded) for each to wind down, so the
/// streamed tail is persisted and session status is reset before the app exits.
#[tauri::command]
pub async fn chat_cancel_all(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(flag) = state.chat_cancel.lock().await.as_ref() {
        flag.store(true, Ordering::SeqCst);
    }
    for flag in state.task_cancel.lock().await.values() {
        flag.store(true, Ordering::SeqCst);
    }
    for _ in 0..50 {
        let idle =
            state.chat_cancel.lock().await.is_none() && state.task_cancel.lock().await.is_empty();
        if idle {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
}

/// Stop an in-flight chat turn. With `task_key` it stops just that parallel
/// Agent task; without one it stops the sequential IDE chat turn.
#[tauri::command]
pub async fn chat_cancel(
    state: State<'_, AppState>,
    task_key: Option<String>,
) -> Result<(), String> {
    match task_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        Some(key) => {
            if let Some(flag) = state.task_cancel.lock().await.get(key) {
                flag.store(true, Ordering::SeqCst);
            }
        }
        None => {
            let slot = state.chat_cancel.lock().await;
            if let Some(flag) = slot.as_ref() {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }
    Ok(())
}
