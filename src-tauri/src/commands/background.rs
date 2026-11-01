//! Tauri commands for registered background tasks (list / output / kill).
//!
//! Live state comes from the kernel `BackgroundRegistry` in `AppState`;
//! listing a project also triggers its once-per-run reconciliation
//! (stale rows → `interrupted`, persisted continuations re-dispatched).

use std::collections::HashMap;
use std::path::Path;

use piscis_kernel::background::BgSnapshot;
use piscis_kernel::store::db::Database;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::state::AppState;

/// One background task plus the chat mode that started it.
#[derive(Serialize)]
pub struct BackgroundTaskView {
    #[serde(flatten)]
    pub task: BgSnapshot,
    /// Session source (`codez`, `workz`, `workz-team`, …) when the session row exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_source: Option<String>,
}

#[derive(Serialize)]
pub struct BackgroundOutput {
    pub task: BgSnapshot,
    pub stdout: String,
    pub stderr: String,
    pub next_cursor_out: u64,
    pub next_cursor_err: u64,
    pub truncated: bool,
}

/// List background tasks, newest first. Filters by session when given;
/// always scoped to tasks whose workspace matches `project_dir` when given.
#[tauri::command]
pub async fn background_list(
    app: AppHandle,
    state: State<'_, AppState>,
    project_dir: Option<String>,
    session_id: Option<String>,
) -> Result<Vec<BackgroundTaskView>, String> {
    if let Some(dir) = project_dir.as_deref().filter(|d| !d.is_empty()) {
        crate::runtime::background_store::reconcile_project(&app, state.inner(), dir).await;
    }
    let tasks = state
        .background_runs
        .list(session_id.as_deref(), project_dir.as_deref());
    let sources = session_sources(project_dir.as_deref(), &tasks);
    Ok(tasks
        .into_iter()
        .map(|task| {
            let session_source = sources.get(&task.session_id).cloned();
            BackgroundTaskView {
                task,
                session_source,
            }
        })
        .collect())
}

fn session_sources(project_dir: Option<&str>, tasks: &[BgSnapshot]) -> HashMap<String, String> {
    let Some(dir) = project_dir.filter(|d| !d.is_empty() && !crate::remote::is_remote(d)) else {
        return HashMap::new();
    };
    let db_path = Path::new(dir).join(".agentz").join("piscis.db");
    if !db_path.is_file() {
        return HashMap::new();
    }
    let Ok(db) = Database::open(&db_path) else {
        return HashMap::new();
    };
    let mut sources = HashMap::new();
    for task in tasks {
        if sources.contains_key(&task.session_id) {
            continue;
        }
        if let Ok(Some(session)) = db.get_session(&task.session_id) {
            let source = session.source.trim();
            if !source.is_empty() {
                sources.insert(task.session_id.clone(), source.to_string());
            }
        }
    }
    sources
}

/// Read one task's status plus output from absolute cursors (0 = from the
/// retained tail start).
#[tauri::command]
pub async fn background_output(
    state: State<'_, AppState>,
    bg_id: String,
    cursor_out: Option<u64>,
    cursor_err: Option<u64>,
) -> Result<BackgroundOutput, String> {
    let entry = state
        .background_runs
        .get(&bg_id)
        .ok_or_else(|| format!("unknown background task: {bg_id}"))?;
    let (out, next_out, trunc_out) = entry.stdout.read_from(cursor_out.unwrap_or(0));
    let (err, next_err, trunc_err) = entry.stderr.read_from(cursor_err.unwrap_or(0));
    Ok(BackgroundOutput {
        task: entry.snapshot(),
        stdout: String::from_utf8_lossy(&out).into_owned(),
        stderr: String::from_utf8_lossy(&err).into_owned(),
        next_cursor_out: next_out,
        next_cursor_err: next_err,
        truncated: trunc_out || trunc_err,
    })
}

/// Terminate a background task's process tree and discard any continuation
/// parked for it.
#[tauri::command]
pub async fn background_kill(
    state: State<'_, AppState>,
    bg_id: String,
) -> Result<bool, String> {
    state.bg_hub.cancel_pending_for_bg(&bg_id);
    Ok(state.background_runs.kill(&bg_id))
}

/// Number of currently running background tasks (optionally per workspace).
#[tauri::command]
pub async fn background_running_count(
    state: State<'_, AppState>,
    project_dir: Option<String>,
) -> Result<usize, String> {
    Ok(state
        .background_runs
        .running_count(project_dir.as_deref()))
}
