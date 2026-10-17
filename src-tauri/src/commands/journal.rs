//! Tauri commands for the file journal — Review / Undo of a turn's edits.

use tauri::AppHandle;

use crate::commands::data_scope::require_project_dir;
use crate::journal::{open_project_journal, FileJournal, JournalChange, JournalFileDiff};

async fn task_journal_root(project_dir: Option<&str>, session_id: &str) -> Result<String, String> {
    let project = require_project_dir(project_dir)?;
    super::agent_task::resolve_task_workspace(
        std::path::Path::new(&project),
        Some(session_id),
        None,
        false,
    )
    .await
}

/// Shared by the IPC commands and integration tests: resolve before opening storage.
pub async fn open_task_journal(
    project_dir: Option<&str>,
    session_id: &str,
) -> Result<FileJournal, String> {
    let root = task_journal_root(project_dir, session_id).await?;
    open_project_journal(&root)
}

/// List the files changed by a turn (applied, not yet undone), newest first.
#[tauri::command]
pub async fn journal_list_changes(
    state: tauri::State<'_, crate::state::AppState>,
    _app: AppHandle,
    project_dir: Option<String>,
    session_id: String,
    turn_id: String,
) -> Result<Vec<JournalChange>, String> {
    let _lease = state.environment_access.execution().await;
    let journal = open_task_journal(project_dir.as_deref(), &session_id).await?;
    journal
        .list_changes(&session_id, &turn_id)
        .map_err(|e| e.to_string())
}

/// Before/after contents for inline diff cards in the CodeZ stream.
#[tauri::command]
pub async fn journal_get_turn_diffs(
    state: tauri::State<'_, crate::state::AppState>,
    _app: AppHandle,
    project_dir: Option<String>,
    session_id: String,
    turn_id: String,
) -> Result<Vec<JournalFileDiff>, String> {
    let _lease = state.environment_access.execution().await;
    let journal = open_task_journal(project_dir.as_deref(), &session_id).await?;
    journal
        .get_turn_file_diffs(&session_id, &turn_id)
        .map_err(|e| e.to_string())
}

/// Undo every applied change in a turn, restoring pre-edit file content.
/// Returns the restored relative paths.
#[tauri::command]
pub async fn journal_undo_turn(
    state: tauri::State<'_, crate::state::AppState>,
    _app: AppHandle,
    project_dir: Option<String>,
    session_id: String,
    turn_id: String,
) -> Result<Vec<String>, String> {
    let _lease = state.environment_access.execution().await;
    let journal = open_task_journal(project_dir.as_deref(), &session_id).await?;
    journal
        .undo_turn(&session_id, &turn_id)
        .map_err(|e| e.to_string())
}
