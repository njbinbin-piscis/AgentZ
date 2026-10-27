//! Background process registry: long-running commands that outlive the tool
//! call (and the agent turn) that started them.
//!
//! The registry is execution-agnostic: callers hand it an `exec` closure that
//! runs on a dedicated registry thread and drives the process to completion,
//! streaming output into the entry's shared buffers. The Windows sandbox
//! detached runner and the plain std-process runner both plug in here.
//!
//! Lifecycle rules:
//! - A background task is bound to the session that started it; completion
//!   listeners (the desktop host) use that binding to resume the session.
//! - Killing is cooperative: `kill` sets a per-entry flag and the exec
//!   closure is responsible for terminating its process tree promptly.
//! - Output is kept in capped ring buffers (tail) plus an optional full log
//!   file under `<workspace>/.agentz/logs/bg/` when a workspace is known.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Bytes of each stream retained in memory (tail). Full output goes to the
/// per-task log file when enabled.
pub const STREAM_RING_CAP: usize = 256 * 1024;
const DEFAULT_MAX_PER_WORKSPACE: usize = 4;
const DEFAULT_MAX_RUNTIME_SECS: u64 = 3600;

/// Path-normalized workspace comparison: unify separators, strip a
/// Windows `\\?\` prefix and trailing separators; case-insensitive on
/// Windows where the filesystem is.
fn same_workspace(a: &str, b: &str) -> bool {
    fn norm(p: &str) -> String {
        let mut s = p.replace('\\', "/");
        if let Some(rest) = s.strip_prefix("//?/") {
            s = rest.to_string();
        }
        while s.len() > 1 && s.ends_with('/') {
            s.pop();
        }
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    }
    norm(a) == norm(b)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// What the starter asked to be notified about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyOn {
    /// Notify only when the process reaches a terminal state.
    Exit,
    /// Notify when the readiness condition fires, and again at terminal state.
    Ready,
}

/// Readiness condition for `NotifyOn::Ready` (e.g. dev servers never exit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadySpec {
    None,
    /// A TCP connect to 127.0.0.1:port succeeds.
    Port(u16),
    /// The combined output contains this substring.
    LogSubstring(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgStatus {
    Running,
    Exited,
    Killed,
    Failed,
}

impl BgStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BgStatus::Running => "running",
            BgStatus::Exited => "exited",
            BgStatus::Killed => "killed",
            BgStatus::Failed => "failed",
        }
    }
    pub fn is_terminal(&self) -> bool {
        !matches!(self, BgStatus::Running)
    }
}

/// Shared, capped stream buffer. `total` counts every byte ever seen so
/// readers can use absolute cursors and detect truncation.
#[derive(Debug, Default)]
pub struct SharedStream {
    inner: Mutex<StreamInner>,
}

/// Per-stream on-disk log cap. stdout and stderr share one task log file,
/// so a task's log stays under ~2× this bound no matter how chatty it is;
/// the in-memory ring keeps the tail regardless.
const LOG_FILE_CAP_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Default)]
struct StreamInner {
    bytes: Vec<u8>,
    total: u64,
    log: Option<std::fs::File>,
    log_bytes: u64,
}

impl SharedStream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_log_file(&self, path: PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            self.inner.lock().unwrap().log = Some(file);
        }
    }

    pub fn append(&self, chunk: &[u8]) {
        let mut g = self.inner.lock().unwrap();
        g.total = g.total.saturating_add(chunk.len() as u64);
        let room = LOG_FILE_CAP_BYTES.saturating_sub(g.log_bytes) as usize;
        let mut written = 0usize;
        if room > 0 {
            if let Some(log) = g.log.as_mut() {
                let slice = &chunk[..chunk.len().min(room)];
                if log.write_all(slice).is_ok() {
                    written = slice.len();
                }
            }
        }
        g.log_bytes += written as u64;
        g.bytes.extend_from_slice(chunk);
        if g.bytes.len() > STREAM_RING_CAP {
            let drop = g.bytes.len() - STREAM_RING_CAP;
            g.bytes.drain(..drop);
        }
    }

    /// Bytes starting at absolute `cursor` (clamped to the retained tail).
    pub fn read_from(&self, cursor: u64) -> (Vec<u8>, u64, bool) {
        let g = self.inner.lock().unwrap();
        let base = g.total.saturating_sub(g.bytes.len() as u64);
        let truncated = cursor < base;
        let start = cursor.max(base);
        let offset = (start - base) as usize;
        (g.bytes[offset.min(g.bytes.len())..].to_vec(), g.total, truncated)
    }

    pub fn tail(&self, max: usize) -> Vec<u8> {
        let g = self.inner.lock().unwrap();
        let start = g.bytes.len().saturating_sub(max);
        g.bytes[start..].to_vec()
    }

    pub fn total(&self) -> u64 {
        self.inner.lock().unwrap().total
    }

    pub fn contains(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }
        let g = self.inner.lock().unwrap();
        // Byte-level substring search is fine for readiness probes.
        g.bytes
            .windows(needle.len())
            .any(|w| w == needle.as_bytes())
    }
}

/// Specification for a new background task.
#[derive(Debug, Clone)]
pub struct BgSpec {
    pub session_id: String,
    /// Owning project/workspace root (concurrency quota + log location).
    pub workspace_root: String,
    /// Short human/agent label, usually the command line.
    pub label: String,
    pub cwd: String,
    pub notify_on: NotifyOn,
    pub ready: ReadySpec,
    /// Hard runtime cap; `None` uses the registry default (jobs) or no
    /// practical cap (services).
    pub max_runtime: Option<Duration>,
    pub kind: BgKind,
}

/// Intent of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BgKind {
    /// Bounded work (build, test run, long script): killed at `max_runtime`;
    /// its completion is reported back to the agent.
    #[default]
    Job,
    /// Long-lived process (dev server, watcher): no runtime cap; only an
    /// unexpected exit is reported.
    Service,
}

impl BgKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Job => "job",
            Self::Service => "service",
        }
    }
}

/// Effectively unbounded runtime for services (they end with the session/app).
const SERVICE_MAX_RUNTIME: Duration = Duration::from_secs(30 * 24 * 3600);
/// Terminal entries kept in memory for listing / output reads.
const MAX_RETAINED_TERMINAL: usize = 64;

#[derive(Debug)]
struct BgInner {
    status: BgStatus,
    exit_code: Option<i32>,
    error: Option<String>,
    ready_fired: bool,
    finished_unix_ms: Option<u64>,
}

/// One background task.
pub struct BgEntry {
    pub id: String,
    pub session_id: String,
    pub workspace_root: String,
    pub label: String,
    pub cwd: String,
    pub notify_on: NotifyOn,
    pub ready: ReadySpec,
    pub max_runtime: Duration,
    pub kind: BgKind,
    pub started_unix_ms: u64,
    started: Instant,
    pid: Mutex<Option<u32>>,
    inner: Mutex<BgInner>,
    kill: Arc<AtomicBool>,
    pub stdout: Arc<SharedStream>,
    pub stderr: Arc<SharedStream>,
}

impl std::fmt::Debug for BgEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BgEntry")
            .field("id", &self.id)
            .field("session_id", &self.session_id)
            .finish()
    }
}

/// Serializable snapshot for hosts/UIs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BgSnapshot {
    pub id: String,
    pub session_id: String,
    pub workspace_root: String,
    pub label: String,
    pub cwd: String,
    pub pid: Option<u32>,
    pub status: &'static str,
    pub kind: &'static str,
    /// True when stopped by an explicit kill (agent, user or session cleanup).
    pub killed: bool,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub ready: bool,
    pub started_unix_ms: u64,
    pub finished_unix_ms: Option<u64>,
    pub elapsed_ms: u64,
    pub stdout_total: u64,
    pub stderr_total: u64,
}

impl BgEntry {
    pub fn kill_flag(&self) -> Arc<AtomicBool> {
        self.kill.clone()
    }

    pub fn is_killed(&self) -> bool {
        self.kill.load(Ordering::Relaxed)
    }

    pub fn set_pid(&self, pid: u32) {
        *self.pid.lock().unwrap() = Some(pid);
    }

    pub fn status(&self) -> BgStatus {
        self.inner.lock().unwrap().status
    }

    /// Mark the readiness condition satisfied. Returns true only the first time.
    pub fn mark_ready(&self) -> bool {
        let mut g = self.inner.lock().unwrap();
        if g.ready_fired || g.status != BgStatus::Running {
            return false;
        }
        g.ready_fired = true;
        true
    }

    pub fn ready_fired(&self) -> bool {
        self.inner.lock().unwrap().ready_fired
    }

    fn finish(&self, status: BgStatus, exit_code: Option<i32>, error: Option<String>) {
        let mut g = self.inner.lock().unwrap();
        if g.status.is_terminal() {
            return;
        }
        g.status = status;
        g.exit_code = exit_code;
        g.error = error;
        g.finished_unix_ms = Some(now_unix_ms());
    }

    pub fn snapshot(&self) -> BgSnapshot {
        let g = self.inner.lock().unwrap();
        BgSnapshot {
            id: self.id.clone(),
            session_id: self.session_id.clone(),
            workspace_root: self.workspace_root.clone(),
            label: self.label.clone(),
            cwd: self.cwd.clone(),
            pid: *self.pid.lock().unwrap(),
            status: g.status.as_str(),
            kind: self.kind.as_str(),
            killed: self.is_killed(),
            exit_code: g.exit_code,
            error: g.error.clone(),
            ready: g.ready_fired,
            started_unix_ms: self.started_unix_ms,
            finished_unix_ms: g.finished_unix_ms,
            elapsed_ms: self.started.elapsed().as_millis() as u64,
            stdout_total: self.stdout.total(),
            stderr_total: self.stderr.total(),
        }
    }

    /// Combined output tail for synthetic continuation prompts.
    pub fn output_tail(&self, max_bytes: usize) -> String {
        let mut out = String::from_utf8_lossy(&self.stdout.tail(max_bytes)).into_owned();
        let err = String::from_utf8_lossy(&self.stderr.tail(max_bytes)).into_owned();
        if !err.trim().is_empty() {
            if !out.trim().is_empty() {
                out.push_str("\n--- stderr ---\n");
            }
            out.push_str(&err);
        }
        if out.len() > max_bytes {
            out = out[out.len() - max_bytes..].to_string();
        }
        out
    }
}

/// Outcome an exec closure reports when its process reaches a terminal state.
#[derive(Debug, Clone)]
pub enum BgOutcome {
    Exited(i32),
    Failed(String),
}

/// Event delivered to the registry's completion listener.
#[derive(Debug, Clone)]
pub enum BgEvent {
    Ready(BgSnapshot),
    Terminal(BgSnapshot),
}

pub type BgListener = Arc<dyn Fn(BgEvent) + Send + Sync>;

/// Process-wide fallback registry for hosts that build tool registries per
/// turn without threading their own instance through (headless CLI, Koi).
/// The desktop host uses its own `AppState` instance so it can attach the
/// completion listener that resumes sessions.
pub fn shared_default_registry() -> Arc<BackgroundRegistry> {
    static REGISTRY: std::sync::OnceLock<Arc<BackgroundRegistry>> = std::sync::OnceLock::new();
    REGISTRY
        .get_or_init(|| Arc::new(BackgroundRegistry::new()))
        .clone()
}

/// Thread-safe registry of background tasks. One instance per host process,
/// shared into tool structs at registration time.
pub struct BackgroundRegistry {
    entries: Mutex<HashMap<String, Arc<BgEntry>>>,
    order: Mutex<Vec<String>>,
    listener: RwLock<Option<BgListener>>,
    max_per_workspace: usize,
    default_max_runtime: Duration,
    log_files: bool,
}

impl std::fmt::Debug for BackgroundRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundRegistry").finish()
    }
}

impl Default for BackgroundRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundRegistry {
    pub fn new() -> Self {
        let max_per_workspace = std::env::var("AGENTZ_BG_MAX_PER_WORKSPACE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n >= 1)
            .unwrap_or(DEFAULT_MAX_PER_WORKSPACE);
        let default_max_runtime = std::env::var("AGENTZ_BG_MAX_RUNTIME_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|n| *n >= 1)
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(DEFAULT_MAX_RUNTIME_SECS));
        Self {
            entries: Mutex::new(HashMap::new()),
            order: Mutex::new(Vec::new()),
            listener: RwLock::new(None),
            max_per_workspace,
            default_max_runtime,
            log_files: true,
        }
    }

    pub fn set_listener(&self, listener: BgListener) {
        *self.listener.write().unwrap() = Some(listener);
    }

    fn fire(&self, event: BgEvent) {
        if let Some(listener) = self.listener.read().unwrap().clone() {
            listener(event);
        }
    }

    /// Create an entry with caller-provided streams (used when output
    /// capture started before the entry existed, e.g. foreground transfer).
    pub fn create_with_streams(
        self: &Arc<Self>,
        spec: BgSpec,
        streams: Option<(Arc<SharedStream>, Arc<SharedStream>)>,
    ) -> Result<Arc<BgEntry>, String> {
        let (stdout, stderr) = streams
            .unwrap_or_else(|| (Arc::new(SharedStream::new()), Arc::new(SharedStream::new())));
        self.create_inner(spec, stdout, stderr)
    }

    /// Create an entry (does not start anything). Fails when the workspace
    /// already runs the maximum number of background tasks.
    pub fn create(self: &Arc<Self>, spec: BgSpec) -> Result<Arc<BgEntry>, String> {
        self.create_inner(
            spec,
            Arc::new(SharedStream::new()),
            Arc::new(SharedStream::new()),
        )
    }

    fn create_inner(
        self: &Arc<Self>,
        spec: BgSpec,
        stdout: Arc<SharedStream>,
        stderr: Arc<SharedStream>,
    ) -> Result<Arc<BgEntry>, String> {
        {
            let entries = self.entries.lock().unwrap();
            let running = entries
                .values()
                .filter(|e| {
                    e.workspace_root == spec.workspace_root
                        && e.status() == BgStatus::Running
                })
                .count();
            if running >= self.max_per_workspace {
                return Err(format!(
                    "background task limit reached for this workspace ({running}/{}); wait for one to finish or kill it first",
                    self.max_per_workspace
                ));
            }
        }
        let id = format!("bg-{}", uuid::Uuid::new_v4().simple().to_string()[..12].to_string());
        let entry = Arc::new(BgEntry {
            id: id.clone(),
            session_id: spec.session_id,
            workspace_root: spec.workspace_root.clone(),
            label: spec.label,
            cwd: spec.cwd,
            notify_on: spec.notify_on,
            ready: spec.ready,
            max_runtime: spec.max_runtime.unwrap_or(match spec.kind {
                BgKind::Job => self.default_max_runtime,
                BgKind::Service => SERVICE_MAX_RUNTIME,
            }),
            kind: spec.kind,
            started_unix_ms: now_unix_ms(),
            started: Instant::now(),
            pid: Mutex::new(None),
            inner: Mutex::new(BgInner {
                status: BgStatus::Running,
                exit_code: None,
                error: None,
                ready_fired: false,
                finished_unix_ms: None,
            }),
            kill: Arc::new(AtomicBool::new(false)),
            stdout,
            stderr,
        });
        if self.log_files && !spec.workspace_root.is_empty() {
            let log_path = PathBuf::from(&spec.workspace_root)
                .join(".agentz")
                .join("logs")
                .join("bg")
                .join(format!("{id}.log"));
            entry.stdout.set_log_file(log_path.clone());
            entry.stderr.set_log_file(log_path);
        }
        {
            // Lock order must match `list`: order, then entries.
            let mut order = self.order.lock().unwrap();
            let mut entries = self.entries.lock().unwrap();
            entries.insert(id.clone(), entry.clone());
            order.push(id);
            // Bound memory: drop the oldest finished entries beyond the cap.
            let terminal: Vec<String> = order
                .iter()
                .filter(|k| entries.get(*k).is_some_and(|e| e.status().is_terminal()))
                .cloned()
                .collect();
            if terminal.len() > MAX_RETAINED_TERMINAL {
                let drop_n = terminal.len() - MAX_RETAINED_TERMINAL;
                for k in &terminal[..drop_n] {
                    entries.remove(k);
                }
                order.retain(|k| entries.contains_key(k));
            }
        }
        Ok(entry)
    }

    /// Run `exec` on a registry thread; it must drive the process to a
    /// terminal state, honour `entry.kill_flag()`, stream output into
    /// `entry.stdout/stderr`, and call `entry.mark_ready()` (via the probe
    /// helper) when its readiness condition fires.
    pub fn spawn_exec<F>(self: &Arc<Self>, entry: Arc<BgEntry>, exec: F)
    where
        F: FnOnce(Arc<BgEntry>) -> BgOutcome + Send + 'static,
    {
        let registry = self.clone();
        let thread_entry = entry.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("bg-{}", entry.id))
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    exec(thread_entry.clone())
                }))
                .unwrap_or_else(|_| BgOutcome::Failed("background worker panicked".into()));
                let killed = thread_entry.is_killed();
                match outcome {
                    BgOutcome::Exited(code) => {
                        if killed {
                            thread_entry.finish(BgStatus::Killed, Some(code), None);
                        } else {
                            thread_entry.finish(BgStatus::Exited, Some(code), None);
                        }
                    }
                    BgOutcome::Failed(error) => {
                        if killed {
                            thread_entry.finish(BgStatus::Killed, None, Some(error));
                        } else {
                            thread_entry.finish(BgStatus::Failed, None, Some(error));
                        }
                    }
                }
                registry.fire(BgEvent::Terminal(thread_entry.snapshot()));
            });
        if let Err(error) = spawned {
            entry.finish(
                BgStatus::Failed,
                None,
                Some(format!("failed to spawn background worker: {error}")),
            );
            self.fire(BgEvent::Terminal(entry.snapshot()));
        }
    }

    /// Probe the readiness condition once; fires the Ready event on first hit.
    pub fn probe_ready(self: &Arc<Self>, entry: &BgEntry) -> bool {
        if entry.ready_fired() || entry.status() != BgStatus::Running {
            return entry.ready_fired();
        }
        let hit = match &entry.ready {
            ReadySpec::None => false,
            ReadySpec::Port(port) => std::net::TcpStream::connect_timeout(
                &std::net::SocketAddr::from(([127, 0, 0, 1], *port)),
                Duration::from_millis(200),
            )
            .is_ok(),
            ReadySpec::LogSubstring(needle) => {
                entry.stdout.contains(needle) || entry.stderr.contains(needle)
            }
        };
        if hit && entry.mark_ready() {
            self.fire(BgEvent::Ready(entry.snapshot()));
        }
        hit
    }

    pub fn get(&self, id: &str) -> Option<Arc<BgEntry>> {
        self.entries.lock().unwrap().get(id).cloned()
    }

    /// Newest first. `session_id`/`workspace_root` filter when given.
    /// Workspace comparison is path-normalized (separator/case tolerant) so
    /// `C:\proj`, `C:/proj/` and a canonicalized `\\?\C:\proj` all match.
    pub fn list(
        &self,
        session_id: Option<&str>,
        workspace_root: Option<&str>,
    ) -> Vec<BgSnapshot> {
        let order = self.order.lock().unwrap();
        let entries = self.entries.lock().unwrap();
        order
            .iter()
            .rev()
            .filter_map(|id| entries.get(id))
            .filter(|e| session_id.is_none_or(|s| e.session_id == s))
            .filter(|e| {
                workspace_root.is_none_or(|w| same_workspace(&e.workspace_root, w))
            })
            .map(|e| e.snapshot())
            .collect()
    }

    pub fn running_count(&self, workspace_root: Option<&str>) -> usize {
        self.entries
            .lock()
            .unwrap()
            .values()
            .filter(|e| e.status() == BgStatus::Running)
            .filter(|e| {
                workspace_root.is_none_or(|w| same_workspace(&e.workspace_root, w))
            })
            .count()
    }

    /// Request termination. The exec closure observes the flag and performs
    /// the actual process-tree teardown.
    pub fn kill(&self, id: &str) -> bool {
        match self.get(id) {
            Some(entry) if entry.status() == BgStatus::Running => {
                entry.kill.store(true, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    /// Kill every running task of a session (session deletion / shutdown).
    pub fn kill_session(&self, session_id: &str) -> usize {
        let mut n = 0;
        for entry in self.entries.lock().unwrap().values() {
            if entry.session_id == session_id && entry.status() == BgStatus::Running {
                entry.kill.store(true, Ordering::Relaxed);
                n += 1;
            }
        }
        n
    }

    /// Kill every running task (host shutdown).
    pub fn kill_all(&self) -> usize {
        let mut n = 0;
        for entry in self.entries.lock().unwrap().values() {
            if entry.status() == BgStatus::Running {
                entry.kill.store(true, Ordering::Relaxed);
                n += 1;
            }
        }
        n
    }

    /// Block (poll) until the entry is terminal, its readiness fires, or the
    /// timeout elapses. Used by `process_wait`; runs on the caller's thread.
    pub fn wait_for(
        self: &Arc<Self>,
        entry: &BgEntry,
        timeout: Duration,
        want_ready: bool,
    ) -> BgSnapshot {
        let deadline = Instant::now() + timeout;
        loop {
            let snap = entry.snapshot();
            if snap.status != "running" {
                return snap;
            }
            if want_ready {
                self.probe_ready(entry);
                if entry.ready_fired() {
                    return entry.snapshot();
                }
            }
            if Instant::now() >= deadline {
                return entry.snapshot();
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(session: &str) -> BgSpec {
        BgSpec {
            session_id: session.into(),
            workspace_root: String::new(),
            label: "echo test".into(),
            cwd: String::new(),
            notify_on: NotifyOn::Exit,
            ready: ReadySpec::None,
            max_runtime: None,
            kind: BgKind::Job,
        }
    }

    #[test]
    fn concurrent_create_and_list_do_not_deadlock() {
        let registry = Arc::new(BackgroundRegistry::new());
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let r1 = registry.clone();
        let creator = std::thread::spawn(move || {
            for _ in 0..2000 {
                if let Ok(e) = r1.create(spec("s")) {
                    e.finish(BgStatus::Exited, Some(0), None);
                }
            }
        });
        let r2 = registry.clone();
        let lister = std::thread::spawn(move || {
            for _ in 0..2000 {
                let _ = r2.list(None, None);
            }
        });
        std::thread::spawn(move || {
            creator.join().unwrap();
            lister.join().unwrap();
            let _ = done_tx.send(());
        });
        done_rx
            .recv_timeout(Duration::from_secs(20))
            .expect("create/list deadlocked");
    }

    #[test]
    fn registry_tracks_lifecycle_and_kill() {
        let registry = Arc::new(BackgroundRegistry::new());
        let entry = registry.create(spec("s1")).unwrap();
        let e2 = entry.clone();
        registry.spawn_exec(entry.clone(), move |e| {
            e.set_pid(4242);
            e.stdout.append(b"hello background\n");
            // Honour the kill flag like a real runner.
            for _ in 0..50 {
                if e.is_killed() {
                    return BgOutcome::Exited(130);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let _ = e2;
            BgOutcome::Exited(0)
        });
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(registry.running_count(None), 1);
        assert!(registry.kill(&entry.id));
        let snap = registry.wait_for(&entry, Duration::from_secs(5), false);
        assert_eq!(snap.status, "killed");
        assert_eq!(snap.pid, Some(4242));
        assert_eq!(registry.running_count(None), 0);
        let (bytes, total, truncated) = entry.stdout.read_from(0);
        assert_eq!(String::from_utf8_lossy(&bytes), "hello background\n");
        assert_eq!(total, 17);
        assert!(!truncated);
    }

    #[test]
    fn registry_ready_probe_fires_once() {
        let registry = Arc::new(BackgroundRegistry::new());
        let mut s = spec("s1");
        s.ready = ReadySpec::LogSubstring("listening".into());
        s.notify_on = NotifyOn::Ready;
        let entry = registry.create(s).unwrap();
        entry.stdout.append(b"server listening on 3000\n");
        assert!(registry.probe_ready(&entry));
        assert!(entry.ready_fired());
        let snap = registry.wait_for(&entry, Duration::from_millis(50), true);
        assert!(snap.ready);
        assert_eq!(snap.status, "running");
    }

    #[test]
    fn registry_enforces_per_workspace_limit() {
        let registry = Arc::new(BackgroundRegistry::new());
        let mut s = spec("s1");
        s.workspace_root = "C:/proj".into();
        for _ in 0..4 {
            registry.create(s.clone()).unwrap();
        }
        assert!(registry.create(s.clone()).is_err());
        let other = {
            let mut o = spec("s2");
            o.workspace_root = "C:/other".into();
            o
        };
        assert!(registry.create(other).is_ok());
    }
}
