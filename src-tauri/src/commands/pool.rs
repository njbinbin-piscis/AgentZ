//! Pool (team collaboration) read + lifecycle commands (Phase 3).
//!
//! The kernel owns Pool orchestration; these commands expose the project DB's
//! `pool_sessions` / `pool_messages` / `koi_todos` / `kois` rows to the
//! collaboration board UI, plus light lifecycle controls (pause/resume/archive/
//! delete). Dispatch itself happens through the main agent's `pool_org` /
//! `pool_chat` tools, so there is no per-task dispatch command here.

use serde::Serialize;
use tauri::AppHandle;

use piscis_core::models::{KoiDefinition, KoiTodo, PoolMessage, PoolSession};
use piscis_kernel::store::db::Database;

use crate::commands::data_scope::open_project_kernel_state;

#[derive(Debug, Serialize)]
pub struct PoolMember {
    pub koi_id: String,
    pub name: String,
    pub role: String,
    pub icon: String,
    pub color: String,
    pub status: String,
}

#[tauri::command]
pub async fn pool_list(app: AppHandle, project_dir: String) -> Result<Vec<PoolSession>, String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let db = db.lock().await;
    db.list_pool_sessions().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pool_get(
    app: AppHandle,
    project_dir: String,
    pool_id: String,
) -> Result<Option<PoolSession>, String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let db = db.lock().await;
    db.get_pool_session(&pool_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pool_members(
    app: AppHandle,
    project_dir: String,
    pool_id: String,
) -> Result<Vec<PoolMember>, String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let db = db.lock().await;
    let ids = db
        .list_pool_member_ids(&pool_id)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for koi_id in ids {
        if let Ok(Some(k)) = db.get_koi(&koi_id) {
            let status = reconcile_member_status(&db, &k.id, &k.status);
            out.push(PoolMember {
                koi_id: k.id,
                name: k.name,
                role: k.role,
                icon: k.icon,
                color: k.color,
                status,
            });
        }
    }
    Ok(out)
}

/// Persist and return a consistent member badge. A `busy` Koi with no
/// `in_progress` todo on the board is stale (crashed turn, killed process,
/// watchdog reset) and healed to `idle` — same board-derived rule the kernel
/// coordinator uses when restoring status after a turn (coordinator.rs).
fn reconcile_member_status(db: &Database, koi_id: &str, status: &str) -> String {
    if status != "busy" {
        return status.to_string();
    }
    match db.koi_has_in_progress_todo(koi_id) {
        Ok(true) => return "busy".into(),
        Ok(false) => {}
        Err(e) => {
            tracing::warn!(koi_id, "failed to check koi board state: {e}");
            return status.to_string();
        }
    }
    match db.update_koi_status(koi_id, "idle") {
        Ok(()) => {
            tracing::info!(koi_id, "healed stale koi status busy -> idle (no board work)");
        }
        Err(e) => {
            tracing::warn!(koi_id, "failed to heal stale busy koi status: {e}");
        }
    }
    "idle".into()
}

#[tauri::command]
pub async fn pool_messages(
    app: AppHandle,
    project_dir: String,
    pool_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<PoolMessage>, String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let db = db.lock().await;
    db.get_pool_messages(&pool_id, limit.unwrap_or(200), offset.unwrap_or(0))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pool_todos(
    app: AppHandle,
    project_dir: String,
    pool_id: String,
) -> Result<Vec<KoiTodo>, String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let db = db.lock().await;
    db.list_active_todos_by_pool(&pool_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pool_kois(app: AppHandle, project_dir: String) -> Result<Vec<KoiDefinition>, String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let db = db.lock().await;
    db.list_kois().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pool_set_status(
    app: AppHandle,
    project_dir: String,
    pool_id: String,
    status: String,
) -> Result<(), String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    {
        let db = db.lock().await;
        db.update_pool_session_status(&pool_id, &status)
            .map_err(|e| e.to_string())?;
    }
    // A resumed pool must be re-driven even if its patrol retired while the
    // pool was paused (or the app restarted in between).
    if status == "active" {
        crate::runtime::patrol::ensure_pool_patrol(&app, &project_dir);
        crate::runtime::patrol::kick_pool_patrols();
    }
    Ok(())
}

#[tauri::command]
pub async fn pool_delete(
    app: AppHandle,
    project_dir: String,
    pool_id: String,
) -> Result<(), String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let db = db.lock().await;
    db.delete_pool_session(&pool_id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod member_status_tests {
    use super::reconcile_member_status;
    use piscis_kernel::store::db::Database;

    #[test]
    fn heals_busy_without_board_work_and_keeps_live_busy() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_koi_with_id("koi-live", "Live").unwrap();
        db.upsert_koi_with_id("koi-stale", "Stale").unwrap();
        db.update_koi_status("koi-live", "busy").unwrap();
        db.update_koi_status("koi-stale", "busy").unwrap();

        let live_todo = db
            .create_koi_todo("koi-live", "work", "", "medium", "piscis", None, "pool", None, 600)
            .unwrap();
        db.claim_koi_todo(&live_todo.id, "koi-live").unwrap();

        assert_eq!(reconcile_member_status(&db, "koi-live", "busy"), "busy");
        assert_eq!(reconcile_member_status(&db, "koi-stale", "busy"), "idle");
        // The heal must be persisted, not just projected.
        assert_eq!(db.get_koi("koi-stale").unwrap().unwrap().status, "idle");
        // Non-busy rows pass through untouched.
        assert_eq!(reconcile_member_status(&db, "koi-live", "idle"), "idle");
    }
}
