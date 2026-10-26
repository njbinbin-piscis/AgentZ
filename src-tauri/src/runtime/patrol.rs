//! Background pool patrol for swarm teams.
//!
//! Swarm runs in AgentZ are driven by the main agent turn dispatching member
//! Koi through `pool_org(assign_koi)`; there is no always-on coordinator
//! process. If a Koi turn crashes, times out, or a todo is left pending while
//! its owner is idle, nothing would re-drive it. This patrol mirrors the kernel
//! recovery loop wired by `openpiscis` bootstrap: per project DB it periodically
//!
//! 1. rolls back stale `busy` Koi and `in_progress` todos (watchdog), and
//! 2. re-activates pending todos for each active pool whose owner is idle.
//!
//! Because AgentZ uses per-project DBs (no single global store), patrols are
//! keyed by `project_dir` and deduplicated. A patrol auto-exits once its project
//! has no non-archived pools left, so idle projects cost nothing.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tauri::AppHandle;
use tracing::{info, warn};

use piscis_kernel::pool::coordinator::{self, CoordinatorConfig};
use piscis_kernel::pool::store::PoolStore;

use crate::commands::data_scope::open_project_kernel_state;
use crate::runtime::koi::pool_wiring;

/// How long a `busy` Koi / `in_progress` todo may go without progress before
/// the watchdog rolls it back (steady-state rounds only; startup uses 0).
const STALE_BUSY_SECS: i64 = 600;
/// Interval between patrol rounds for an active project.
const PATROL_INTERVAL: Duration = Duration::from_secs(30);
/// Consecutive empty rounds (no non-archived pools) before a patrol retires.
const EMPTY_ROUNDS_BEFORE_EXIT: u32 = 3;

fn kick() -> &'static tokio::sync::Notify {
    static KICK: OnceLock<tokio::sync::Notify> = OnceLock::new();
    KICK.get_or_init(tokio::sync::Notify::new)
}

/// Run the next patrol round of every live patrol now, e.g. after a todo
/// completes so its dependents start without waiting for the interval.
pub fn kick_pool_patrols() {
    kick().notify_waiters();
}

/// Upper bound on concurrently running member turns per pool, from
/// `AGENTZ_POOL_MAX_CONCURRENCY` (default 3, min 1).
pub fn pool_max_concurrency() -> usize {
    std::env::var("AGENTZ_POOL_MAX_CONCURRENCY")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(3)
}

fn registry() -> &'static Mutex<HashSet<String>> {
    static REG: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Lock the dedup registry, tolerating a poisoned mutex: a patrol round that
/// panicked while holding the lock must not permanently wedge all future
/// patrols, so we recover the inner set instead of unwrapping.
fn lock_registry() -> std::sync::MutexGuard<'static, HashSet<String>> {
    registry().lock().unwrap_or_else(|e| e.into_inner())
}

/// Ensure a background patrol is running for `project_dir`. Idempotent: a second
/// call while a patrol is already active for the same project is a no-op.
pub fn ensure_pool_patrol(app: &AppHandle, project_dir: &str) {
    let dir = project_dir.trim().to_string();
    if dir.is_empty() {
        return;
    }
    {
        let mut reg = lock_registry();
        if !reg.insert(dir.clone()) {
            return;
        }
    }
    let app = app.clone();
    tokio::spawn(async move {
        run_patrol(app, dir.clone()).await;
        lock_registry().remove(&dir);
    });
}

async fn run_patrol(app: AppHandle, project_dir: String) {
    info!(target: "pool::patrol", project_dir = %project_dir, "swarm patrol started");
    // Startup sweep recovers anything left stale by a previous crash immediately.
    run_round_isolated(&app, &project_dir, 0).await;

    let mut empty_rounds = 0u32;
    loop {
        tokio::select! {
            _ = tokio::time::sleep(PATROL_INTERVAL) => {}
            _ = kick().notified() => {
                // Let the completing turn finish its bookkeeping first.
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        match run_round_isolated(&app, &project_dir, STALE_BUSY_SECS).await {
            Some(0) => {
                empty_rounds += 1;
                if empty_rounds >= EMPTY_ROUNDS_BEFORE_EXIT {
                    info!(target: "pool::patrol", project_dir = %project_dir, "swarm patrol retiring (no active pools)");
                    return;
                }
            }
            Some(_) => empty_rounds = 0,
            None => {
                // DB could not be opened (project removed?) — retire.
                return;
            }
        }
    }
}

/// Run one round inside its own task so a panic in DB/runtime code is contained
/// to that round (logged + treated as a non-empty round) rather than killing
/// the whole patrol loop.
async fn run_round_isolated(app: &AppHandle, project_dir: &str, max_busy_secs: i64) -> Option<u32> {
    let app = app.clone();
    let dir = project_dir.to_string();
    match tokio::spawn(async move { run_round(&app, &dir, max_busy_secs).await }).await {
        Ok(outcome) => outcome,
        Err(join_err) => {
            warn!(
                target: "pool::patrol",
                project_dir = %project_dir,
                "patrol round panicked, continuing: {join_err}"
            );
            // Treat a panicked round as "work present" so the patrol keeps going
            // instead of prematurely retiring.
            Some(1)
        }
    }
}

/// One patrol round. Returns the number of non-archived pools observed, or
/// `None` if the project DB could not be opened.
async fn run_round(app: &AppHandle, project_dir: &str, max_busy_secs: i64) -> Option<u32> {
    let (db, _settings) = open_project_kernel_state(app, project_dir).ok()?;
    let store = PoolStore::new(db.clone());

    let pools = {
        let guard = db.lock().await;
        guard
            .list_pool_sessions()
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.status != "archived")
            .collect::<Vec<_>>()
    };
    let active_pools = pools.iter().filter(|p| p.status == "active").count() as u32;
    if pools.is_empty() {
        return Some(0);
    }

    let effective_max_busy = if max_busy_secs <= 0 {
        0
    } else {
        pools
            .iter()
            .filter(|p| p.status == "active")
            .map(|p| {
                if p.task_timeout_secs > 0 {
                    ((p.task_timeout_secs as i64 * 12) / 10).max(STALE_BUSY_SECS)
                } else {
                    STALE_BUSY_SECS
                }
            })
            .max()
            .unwrap_or(STALE_BUSY_SECS)
    };

    if effective_max_busy > 0 {
        let (stale_koi, stale_todo) =
            coordinator::watchdog_recover(&store, effective_max_busy).await;
        if stale_koi > 0 || stale_todo > 0 {
            info!(
                target: "pool::patrol",
                project_dir = %project_dir,
                max_busy = effective_max_busy,
                "recovered {stale_koi} stale Koi, {stale_todo} stale todos"
            );
        }
    }

    let (subagent, sink) = pool_wiring(app);
    let cfg = CoordinatorConfig::default();
    for pool in pools.iter().filter(|p| p.status == "active") {
        let max_running = crate::commands::botz::room_max_concurrency(app, &pool.id)
            .unwrap_or_else(pool_max_concurrency);
        let running = {
            let guard = db.lock().await;
            guard
                .list_active_todos_by_pool(&pool.id)
                .map(|v| v.iter().filter(|t| t.status == "in_progress").count())
                .unwrap_or(0)
        };
        if running >= max_running {
            continue;
        }
        match coordinator::activate_pending_todos(
            &store,
            sink.clone(),
            subagent.clone(),
            &cfg,
            Some(&pool.id),
        )
        .await
        {
            Ok(n) if n > 0 => {
                info!(target: "pool::patrol", pool_id = %pool.id, "activated {n} pending todos")
            }
            Err(e) => {
                warn!(target: "pool::patrol", pool_id = %pool.id, "activation error: {e}")
            }
            _ => {}
        }
        wake_finished_parents(&db, &store, &pool.id, sink.clone(), subagent.clone(), &cfg).await;
    }
    Some(active_pools)
}

/// Ask the owner of a parent todo to wrap up once all its subtasks are finished.
async fn wake_finished_parents(
    db: &std::sync::Arc<tokio::sync::Mutex<piscis_kernel::store::db::Database>>,
    store: &PoolStore,
    pool_id: &str,
    sink: std::sync::Arc<dyn piscis_core::host::PoolEventSink>,
    subagent: std::sync::Arc<dyn piscis_core::host::SubagentRuntime>,
    cfg: &CoordinatorConfig,
) {
    use crate::runtime::parent_wake::{finished_parents, mark_woken};
    let (todos, kois) = {
        let guard = db.lock().await;
        let mut all = guard.list_koi_todos(None).unwrap_or_default();
        all.retain(|t| t.pool_session_id.as_deref() == Some(pool_id));
        (all, guard.list_kois().unwrap_or_default())
    };
    for pid in finished_parents(&todos) {
        let Some(parent) = todos.iter().find(|t| t.id == pid) else { continue };
        let Some(owner) = kois.iter().find(|k| k.id == parent.owner_id) else { continue };
        if !mark_woken(&pid) {
            continue;
        }
        let content = format!(
            "@!{} All subtasks of \"{}\" (todo {}) are finished. Read their results in the pool, \
             post a short integrated summary, then call pool_org(action=\"complete_todo\", todo_id=\"{}\").",
            owner.name,
            parent.title,
            pid,
            pid
        );
        let caller = piscis_kernel::pool::model::CallerContext {
            memory_owner_id: "piscis",
            session_id: "patrol",
            session_source: None,
            pool_session_id: Some(pool_id),
            cancel: None,
        };
        let args = piscis_kernel::pool::model::SendPoolMessageArgs {
            pool_id: pool_id.to_string(),
            sender_id: "piscis".into(),
            content,
            reply_to_message_id: None,
        };
        if let Err(e) = piscis_kernel::pool::services::send_pool_message(
            store,
            sink.clone(),
            Some(subagent.clone()),
            cfg,
            &caller,
            args,
        )
        .await
        {
            warn!(target: "pool::patrol", pool_id, "parent wake failed: {e}");
        }
    }
}
