//! Tauri commands for registered background tasks (list / output / kill).
//!
//! Live state comes from the kernel `BackgroundRegistry` in `AppState`;
//! listing a project also triggers its once-per-run reconciliation
//! (stale rows → `interrupted`, persisted continuations re-dispatched).

use piscis_kernel::background::BgSnapshot;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::state::AppState;

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
) -> Result<Vec<BgSnapshot>, String> {
    if let Some(dir) = project_dir.as_deref().filter(|d| !d.is_empty()) {
        crate::runtime::background_store::reconcile_project(&app, state.inner(), dir).await;
    }
    Ok(state.background_runs.list(
        session_id.as_deref(),
        project_dir.as_deref(),
    ))
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
