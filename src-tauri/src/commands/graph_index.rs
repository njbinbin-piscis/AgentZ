//! Background knowledge-graph index (CodeGraph-style auto-sync).
//!
//! One debounced worker per project; watcher bursts and manual rebuilds coalesce.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::graph::generate;
use super::graph_db::{clear_pending_sync, graph_db_status, mark_pending_sync};

const DEBOUNCE_MS: u64 = 2500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexPhase {
    Idle,
    Queued,
    Building,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexBuildStatus {
    pub phase: IndexPhase,
    pub pending_files: usize,
    pub last_built_at: Option<String>,
    pub last_error: Option<String>,
    pub nodes: usize,
    pub edges: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexBuildAck {
    pub accepted: bool,
    pub phase: IndexPhase,
    pub message: String,
}

/// Aggregate in-memory graph-index ownership without exposing project names.
#[derive(Debug, Clone, Serialize)]
pub struct IndexResourceSnapshot {
    pub queued_projects: usize,
    pub pending_files: usize,
    pub worker_projects: usize,
    pub phase_entries: usize,
    pub last_built_entries: usize,
    pub last_error_entries: usize,
    pub retiring_projects: usize,
}

#[tauri::command]
pub fn graph_index_resource_snapshot() -> IndexResourceSnapshot {
    let st = state().lock().unwrap_or_else(|e| e.into_inner());
    IndexResourceSnapshot {
        queued_projects: st.queues.len(),
        pending_files: st
            .queues
            .values()
            .map(|entry| entry.pending_files.len())
            .sum(),
        worker_projects: st.workers.len(),
        phase_entries: st.phases.len(),
        last_built_entries: st.last_built_at.len(),
        last_error_entries: st.last_errors.len(),
        retiring_projects: st.retired.len(),
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn closing_project_releases_idle_cached_status() {
        let dir = PathBuf::from(format!("graph-census-test-{}", uuid::Uuid::new_v4()));
        let key = project_key(&dir);
        {
            let mut st = state().lock().unwrap();
            st.phases.insert(key.clone(), IndexPhase::Idle);
            st.last_built_at.insert(key.clone(), "prior".into());
            st.last_errors.insert(key.clone(), "prior".into());
        }
        forget_project(&dir);
        let st = state().lock().unwrap();
        assert!(!st.phases.contains_key(&key));
        assert!(!st.last_built_at.contains_key(&key));
        assert!(!st.last_errors.contains_key(&key));
    }

    #[test]
    fn closing_during_build_discards_cached_status_when_worker_finishes() {
        let dir = PathBuf::from(format!("graph-retire-test-{}", uuid::Uuid::new_v4()));
        let key = project_key(&dir);
        {
            let mut st = state().lock().unwrap();
            st.workers.insert(key.clone());
            st.queues.insert(
                key.clone(),
                QueueEntry {
                    root: dir.clone(),
                    touched_at: Instant::now(),
                    pending_files: HashSet::new(),
                    rerun_after_current: false,
                },
            );
            st.phases.insert(key.clone(), IndexPhase::Building);
            st.last_errors.insert(key.clone(), "prior".into());
        }
        forget_project(&dir);
        {
            let st = state().lock().unwrap();
            assert!(!st.queues.contains_key(&key));
            assert!(st.retired.contains(&key));
        }
        let mut st = state().lock().unwrap();
        finish_worker(&mut st, &key);
        assert!(!st.workers.contains(&key));
        assert!(!st.retired.contains(&key));
        assert!(!st.phases.contains_key(&key));
        assert!(!st.last_errors.contains_key(&key));
    }

    #[test]
    fn late_watcher_callback_cannot_requeue_closed_project() {
        use std::sync::atomic::AtomicBool;

        let dir = PathBuf::from(format!("graph-late-test-{}", uuid::Uuid::new_v4()));
        let key = project_key(&dir);
        let active = AtomicBool::new(false);
        forget_project(&dir);
        let ack = request_rebuild_if_active(dir, &active);
        assert!(!ack.accepted);
        let st = state().lock().unwrap();
        assert!(!st.queues.contains_key(&key));
        assert!(!st.workers.contains(&key));
    }

    #[test]
    fn late_incremental_event_does_not_start_graph_work() {
        use std::sync::atomic::AtomicBool;

        let dir = PathBuf::from(format!("graph-late-patch-{}", uuid::Uuid::new_v4()));
        let key = project_key(&dir);
        let active = AtomicBool::new(false);
        notify_file_change_if_active(dir, "src/main.rs".into(), &active);
        let st = state().lock().unwrap();
        assert!(!st.queues.contains_key(&key));
        assert!(!st.workers.contains(&key));
    }

    #[test]
    fn queued_worker_exits_after_project_close() {
        let dir = PathBuf::from(format!("graph-close-worker-{}", uuid::Uuid::new_v4()));
        let key = project_key(&dir);
        assert!(request_rebuild(dir.clone()).accepted);
        forget_project(&dir);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let st = state().lock().unwrap();
            if !st.workers.contains(&key) {
                assert!(!st.queues.contains_key(&key));
                assert!(!st.phases.contains_key(&key));
                assert!(!st.retired.contains(&key));
                break;
            }
            drop(st);
            assert!(
                Instant::now() < deadline,
                "retired graph worker did not exit"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    #[test]
    fn in_progress_build_drops_result_after_project_close() {
        use std::sync::mpsc;

        let dir = PathBuf::from(format!("graph-building-close-{}", uuid::Uuid::new_v4()));
        let key = project_key(&dir);
        {
            let mut st = state().lock().unwrap();
            st.queues.insert(
                key.clone(),
                QueueEntry {
                    root: dir.clone(),
                    touched_at: Instant::now() - Duration::from_millis(DEBOUNCE_MS + 1),
                    pending_files: HashSet::new(),
                    rerun_after_current: true,
                },
            );
            st.workers.insert(key.clone());
            st.phases.insert(key.clone(), IndexPhase::Queued);
        }
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            worker_loop_with(key, |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Err("controlled build failure".into())
            });
        });
        entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        forget_project(&dir);
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        let key = project_key(&dir);
        let st = state().lock().unwrap();
        assert!(!st.queues.contains_key(&key));
        assert!(!st.workers.contains(&key));
        assert!(!st.retired.contains(&key));
        assert!(!st.phases.contains_key(&key));
        assert!(!st.last_errors.contains_key(&key));
    }

    #[test]
    fn completed_real_graph_build_does_not_retain_status_after_close() {
        use std::sync::mpsc;

        let dir = std::env::temp_dir().join(format!("agentz-graph-close-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/main.rs"),
            "fn main() { println!(\"ready\"); }\n",
        )
        .unwrap();
        let key = project_key(&dir);
        {
            let mut st = state().lock().unwrap();
            st.queues.insert(
                key.clone(),
                QueueEntry {
                    root: dir.clone(),
                    touched_at: Instant::now() - Duration::from_millis(DEBOUNCE_MS + 1),
                    pending_files: HashSet::new(),
                    rerun_after_current: false,
                },
            );
            st.workers.insert(key.clone());
            st.phases.insert(key.clone(), IndexPhase::Queued);
        }
        let (built_tx, built_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_key = key.clone();
        let worker = std::thread::spawn(move || {
            worker_loop_with(worker_key, |root| {
                let doc = generate(root)?;
                built_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(doc)
            });
        });
        built_rx.recv_timeout(Duration::from_secs(15)).unwrap();
        assert!(dir.join(".agentz/graph.json").is_file());
        forget_project(&dir);
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        {
            let st = state().lock().unwrap();
            assert!(!st.queues.contains_key(&key));
            assert!(!st.workers.contains(&key));
            assert!(!st.retired.contains(&key));
            assert!(!st.phases.contains_key(&key));
            assert!(!st.last_built_at.contains_key(&key));
            assert!(!st.last_errors.contains_key(&key));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

struct QueueEntry {
    root: PathBuf,
    touched_at: Instant,
    pending_files: HashSet<String>,
    rerun_after_current: bool,
}

struct IndexState {
    queues: HashMap<String, QueueEntry>,
    phases: HashMap<String, IndexPhase>,
    last_built_at: HashMap<String, String>,
    last_errors: HashMap<String, String>,
    workers: HashSet<String>,
    retired: HashSet<String>,
}

fn state() -> &'static Mutex<IndexState> {
    static S: OnceLock<Mutex<IndexState>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(IndexState {
            queues: HashMap::new(),
            phases: HashMap::new(),
            last_built_at: HashMap::new(),
            last_errors: HashMap::new(),
            workers: HashSet::new(),
            retired: HashSet::new(),
        })
    })
}

fn project_key(root: &Path) -> String {
    root.to_string_lossy().to_string()
}

/// Release volatile per-project index state when its local watcher closes.
/// A build already running may finish, but its cached status is discarded.
pub fn forget_project(root: &Path) {
    let key = project_key(root);
    let mut st = state().lock().unwrap_or_else(|e| e.into_inner());
    st.queues.remove(&key);
    if st.workers.contains(&key) {
        st.retired.insert(key);
    } else {
        clear_cached_status(&mut st, &key);
    }
}

fn clear_cached_status(st: &mut IndexState, key: &str) {
    st.phases.remove(key);
    st.last_built_at.remove(key);
    st.last_errors.remove(key);
}

fn finish_worker(st: &mut IndexState, key: &str) {
    st.workers.remove(key);
    if st.retired.remove(key) {
        clear_cached_status(st, key);
    } else {
        st.phases.insert(key.to_owned(), IndexPhase::Idle);
    }
}

pub fn status(root: &Path) -> IndexBuildStatus {
    let key = project_key(root);
    let st = state().lock().unwrap_or_else(|e| e.into_inner());
    let phase = st.phases.get(&key).copied().unwrap_or(IndexPhase::Idle);
    let pending_files = st
        .queues
        .get(&key)
        .map(|q| q.pending_files.len())
        .unwrap_or(0);
    let db = graph_db_status(root).ok().flatten();
    IndexBuildStatus {
        phase,
        pending_files,
        last_built_at: st
            .last_built_at
            .get(&key)
            .cloned()
            .or_else(|| db.as_ref().and_then(|d| d.generated_at.clone())),
        last_error: st.last_errors.get(&key).cloned(),
        nodes: db.as_ref().map(|d| d.nodes).unwrap_or(0),
        edges: db.as_ref().map(|d| d.edges).unwrap_or(0),
    }
}

/// Queue rebuild after a watched file change.
pub fn notify_file_change(root: PathBuf, rel: String) {
    let _ = mark_pending_sync(&root, &rel);
    enqueue(root, Some(rel), None);
}

/// A file event from the owned local watcher. A closed watcher cannot recreate
/// volatile graph work even when its index worker finishes after unmount.
pub fn notify_file_change_if_active(
    root: PathBuf,
    rel: String,
    active: &std::sync::atomic::AtomicBool,
) {
    if !active.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let _ = mark_pending_sync(&root, &rel);
    enqueue(root, Some(rel), Some(active));
}

/// Queue a manual rebuild (Wiki menu). Coalesces if already queued/building.
pub fn request_rebuild(root: PathBuf) -> IndexBuildAck {
    enqueue(root, None, None)
}

/// Only accept a callback from a watcher that is still owned by the host.
/// The activity check shares the queue lock with `forget_project`, so a late
/// callback cannot resurrect a retired project's queue.
pub fn request_rebuild_if_active(
    root: PathBuf,
    active: &std::sync::atomic::AtomicBool,
) -> IndexBuildAck {
    enqueue(root, None, Some(active))
}

/// Start background index when opening a project with no graph.db yet.
pub fn ensure_started(root: &Path) {
    let empty = graph_db_status(root)
        .ok()
        .flatten()
        .map(|s| s.nodes == 0)
        .unwrap_or(true);
    if empty {
        let _ = request_rebuild(root.to_path_buf());
    }
}

/// When the graph index is not usable yet, make sure a build is running and
/// return an explanation the agent can act on (instead of a raw error that
/// teaches it to avoid the tool). `None` means the index is ready.
pub fn not_ready_message(root: &Path, tool: &str) -> Option<String> {
    let ready = graph_db_status(root)
        .ok()
        .flatten()
        .map(|s| s.nodes > 0)
        .unwrap_or(false);
    if ready {
        return None;
    }
    let _ = request_rebuild(root.to_path_buf());
    let st = status(root);
    let phase = match st.phase {
        IndexPhase::Building => "building now",
        _ => "queued",
    };
    Some(format!(
        "{tool}: the repository graph index is not ready yet ({phase}; it is built automatically in the background, usually within a minute). \
         For this question use `codebase_search` or `file_search` now, then retry {tool} later."
    ))
}

fn enqueue(
    root: PathBuf,
    rel: Option<String>,
    active: Option<&std::sync::atomic::AtomicBool>,
) -> IndexBuildAck {
    let key = project_key(&root);
    let spawn_worker = {
        let mut st = state().lock().unwrap_or_else(|e| e.into_inner());
        if active.is_some_and(|flag| !flag.load(std::sync::atomic::Ordering::Acquire)) {
            return IndexBuildAck {
                accepted: false,
                phase: IndexPhase::Idle,
                message: "Watcher already closed; rebuild discarded.".into(),
            };
        }
        let entry = st.queues.entry(key.clone()).or_insert_with(|| QueueEntry {
            root: root.clone(),
            touched_at: Instant::now(),
            pending_files: HashSet::new(),
            rerun_after_current: false,
        });
        entry.touched_at = Instant::now();
        entry.root = root.clone();
        if let Some(r) = rel {
            entry.pending_files.insert(r);
        } else {
            entry.rerun_after_current = true;
        }
        st.phases.insert(key.clone(), IndexPhase::Queued);
        st.retired.remove(&key);
        if st.workers.contains(&key) {
            return IndexBuildAck {
                accepted: true,
                phase: IndexPhase::Queued,
                message: "Index already in progress; request coalesced.".into(),
            };
        }
        st.workers.insert(key.clone());
        true
    };

    if spawn_worker {
        std::thread::spawn(move || worker_loop(key));
    }

    IndexBuildAck {
        accepted: true,
        phase: IndexPhase::Queued,
        message: "Background graph index queued.".into(),
    }
}

fn worker_loop(key: String) {
    worker_loop_with(key, generate);
}

fn worker_loop_with(
    key: String,
    build: impl Fn(&Path) -> Result<crate::commands::graph::GraphDoc, String>,
) {
    loop {
        std::thread::sleep(Duration::from_millis(250));
        enum Step {
            Wait,
            Stop,
            Run(PathBuf),
        }
        let step = {
            let st = state().lock().unwrap_or_else(|e| e.into_inner());
            match st.queues.get(&key) {
                None => Step::Stop,
                Some(entry) if entry.touched_at.elapsed() < Duration::from_millis(DEBOUNCE_MS) => {
                    Step::Wait
                }
                Some(entry) => Step::Run(entry.root.clone()),
            }
        };

        match step {
            Step::Wait => continue,
            Step::Stop => {
                let mut st = state().lock().unwrap_or_else(|e| e.into_inner());
                finish_worker(&mut st, &key);
                break;
            }
            Step::Run(root) => {
                {
                    let mut st = state().lock().unwrap_or_else(|e| e.into_inner());
                    // This build covers everything queued so far; only requests
                    // arriving while it runs should trigger another pass.
                    if let Some(entry) = st.queues.get_mut(&key) {
                        entry.rerun_after_current = false;
                        entry.pending_files.clear();
                    }
                    st.phases.insert(key.clone(), IndexPhase::Building);
                }

                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(&root)))
                        .unwrap_or_else(|_| Err("graph index worker panicked".into()));
                let rerun = {
                    let mut st = state().lock().unwrap_or_else(|e| e.into_inner());
                    match result {
                        Ok(doc) => {
                            st.last_built_at
                                .insert(key.clone(), doc.generated_at.clone());
                            st.last_errors.remove(&key);
                            let _ = clear_pending_sync(&root);
                        }
                        Err(e) => {
                            st.last_errors.insert(key.clone(), e);
                        }
                    }
                    st.queues
                        .get(&key)
                        .map(|e| e.rerun_after_current || !e.pending_files.is_empty())
                        .unwrap_or(false)
                };

                if rerun {
                    let mut st = state().lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(entry) = st.queues.get_mut(&key) {
                        entry.rerun_after_current = false;
                        entry.pending_files.clear();
                        entry.touched_at = Instant::now();
                    }
                    st.phases.insert(key.clone(), IndexPhase::Queued);
                    continue;
                }

                let mut st = state().lock().unwrap_or_else(|e| e.into_inner());
                st.queues.remove(&key);
                finish_worker(&mut st, &key);
                break;
            }
        }
    }
}
