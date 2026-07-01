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
        })
    })
}

fn project_key(root: &Path) -> String {
    root.to_string_lossy().to_string()
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
    enqueue(root, Some(rel));
}

/// Queue a manual rebuild (Wiki menu). Coalesces if already queued/building.
pub fn request_rebuild(root: PathBuf) -> IndexBuildAck {
    enqueue(root, None)
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

fn enqueue(root: PathBuf, rel: Option<String>) -> IndexBuildAck {
    let key = project_key(&root);
    let spawn_worker = {
        let mut st = state().lock().unwrap_or_else(|e| e.into_inner());
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
                st.workers.remove(&key);
                st.phases.insert(key, IndexPhase::Idle);
                break;
            }
            Step::Run(root) => {
                {
                    let mut st = state().lock().unwrap_or_else(|e| e.into_inner());
                    st.phases.insert(key.clone(), IndexPhase::Building);
                }

                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    generate(&root)
                }))
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
                st.workers.remove(&key);
                st.phases.insert(key, IndexPhase::Idle);
                break;
            }
        }
    }
}
