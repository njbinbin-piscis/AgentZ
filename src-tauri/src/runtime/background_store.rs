//! Persistence + startup reconciliation for background tasks.
//!
//! Live state is owned by the kernel `BackgroundRegistry`; this module only
//! mirrors ready/terminal snapshots and parked continuations into the
//! project's kernel DB so an app restart can (a) mark stale `running` rows
//! `interrupted` and (b) re-dispatch continuations that were parked when
//! the app closed.

use std::collections::HashSet;
use std::sync::Mutex as StdMutex;

use piscis_kernel::background::BgSnapshot;
use tauri::AppHandle;

use crate::commands::data_scope::open_project_kernel_state;
use crate::state::AppState;

pub struct StoredContinuation {
    pub bg_id: String,
    pub session_id: String,
    pub prompt: String,
    pub display_prompt: String,
    pub session_source: Option<String>,
    pub prefer_zh: bool,
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Mirror a ready/terminal snapshot into the project DB (best-effort).
pub async fn record_snapshot(app: &AppHandle, snap: &BgSnapshot, kind: &str) {
    if snap.workspace_root.is_empty() {
        return;
    }
    let Ok((db, _settings)) = open_project_kernel_state(app, &snap.workspace_root) else {
        return;
    };
    let guard = db.lock().await;
    let _ = guard.upsert_background_task(
        &snap.id,
        &snap.session_id,
        &snap.label,
        snap.status,
        snap.exit_code.map(|c| c as i64),
        kind,
        now_unix_ms(),
    );
}

/// Persist a parked continuation so it survives an app restart.
pub async fn save_pending(
    app: &AppHandle,
    project_dir: &str,
    bg_id: &str,
    session_id: &str,
    prompt: &str,
    display_prompt: &str,
    session_source: Option<&str>,
    prefer_zh: bool,
) {
    let Ok((db, _settings)) = open_project_kernel_state(app, project_dir) else {
        return;
    };
    let guard = db.lock().await;
    let _ = guard.upsert_pending_bg_continuation(
        bg_id,
        session_id,
        prompt,
        display_prompt,
        session_source,
        prefer_zh,
        now_unix_ms(),
    );
}

pub async fn delete_pending(app: &AppHandle, project_dir: &str, bg_id: &str) {
    let Ok((db, _settings)) = open_project_kernel_state(app, project_dir) else {
        return;
    };
    let guard = db.lock().await;
    let _ = guard.delete_pending_bg_continuation(bg_id);
}

/// Load and remove all persisted continuations for a project.
pub async fn take_pending_continuations(
    app: &AppHandle,
    project_dir: &str,
) -> Vec<StoredContinuation> {
    let Ok((db, _settings)) = open_project_kernel_state(app, project_dir) else {
        return Vec::new();
    };
    let guard = db.lock().await;
    let rows = guard.list_pending_bg_continuations().unwrap_or_default();
    let mut out = Vec::new();
    for (bg_id, session_id, prompt, display_prompt, session_source, prefer_zh) in rows {
        let _ = guard.delete_pending_bg_continuation(&bg_id);
        out.push(StoredContinuation {
            bg_id,
            session_id,
            prompt,
            display_prompt,
            session_source,
            prefer_zh,
        });
    }
    out
}

/// Projects already reconciled in this app run.
static RECONCILED: StdMutex<Option<HashSet<String>>> = StdMutex::new(None);

/// Once per project per app run: stale `running` rows (from tasks whose
/// processes died with the previous app instance) become `interrupted`,
/// and persisted continuations are re-dispatched through the hub.
pub async fn reconcile_project(app: &AppHandle, state: &AppState, project_dir: &str) {
    {
        let mut guard = RECONCILED.lock().unwrap();
        let set = guard.get_or_insert_with(HashSet::new);
        if !set.insert(project_dir.to_string()) {
            return;
        }
    }
    if let Ok((db, _settings)) = open_project_kernel_state(app, project_dir) {
        let guard = db.lock().await;
        if let Ok(ids) = guard.list_running_background_task_ids() {
            for id in ids {
                let live = state
                    .background_runs
                    .get(&id)
                    .is_some_and(|e| e.snapshot().status == "running");
                if !live {
                    let _ = guard.mark_background_task_status(&id, "interrupted");
                }
            }
        }
    }
    state
        .bg_hub
        .restore_pending(app.clone(), state.clone(), project_dir)
        .await;
}
