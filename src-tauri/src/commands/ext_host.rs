//! VS Code extension host sidecar broker.
//!
//! Launches the Node-based extension host (`extension-host/dist/host.js`) as a
//! child process and brokers its line-delimited-JSON RPC over a Tauri event
//! channel: stdout lines are emitted to the renderer (which runs the MainThread
//! side of the protocol), and the renderer sends RPC frames back via
//! [`ext_host_send`], which writes them to the child's stdin.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

use crate::remote::{self, DeployedServer, RemoteTarget};
use crate::state::AppState;

/// Tauri event channel that carries extension-host RPC frames + logs to the UI.
pub const EXT_HOST_EVENT: &str = "agentz:ext-host";
/// Server-pushed control events (`proc.data`, `proc.exit`, ...).
pub const REMOTE_EVENT: &str = "agentz:remote";

type Pending = Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

/// Shared lifecycle state for the (single) extension host process.
#[derive(Default)]
pub struct ExtHostManager {
    inner: Mutex<ExtHostInner>,
    /// Separate from `inner` so a host that stops reading (blocked pipe) can't
    /// freeze status queries and every other caller.
    stdin: Mutex<Option<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
    /// workspace URI -> remote path; re-armed whenever the server restarts.
    remote_watches: std::sync::Mutex<HashMap<String, String>>,
    /// Authority currently being deployed/started, so callers can wait for it.
    connecting: Arc<std::sync::Mutex<Option<String>>>,
}

struct ConnectingGuard(Arc<std::sync::Mutex<Option<String>>>);

impl Drop for ConnectingGuard {
    fn drop(&mut self) {
        *self.0.lock().unwrap() = None;
    }
}

#[derive(Default)]
struct ExtHostInner {
    child: Option<Child>,
    project_dir: Option<String>,
    target: Option<RemoteTarget>,
    server: Option<DeployedServer>,
}

impl ExtHostManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn target(&self) -> Option<RemoteTarget> {
        self.inner.lock().await.target.clone()
    }

    pub async fn server(&self) -> Option<DeployedServer> {
        self.inner.lock().await.server.clone()
    }

    /// Wait until `authority` is the connected target. Waits through an
    /// in-flight connect (deploys can take a minute on slow links) plus a short
    /// grace period for a connect the UI is about to start.
    pub async fn wait_for_authority(&self, authority: &str) -> Result<RemoteTarget, String> {
        let started = std::time::Instant::now();
        loop {
            if let Some(t) = self.target().await {
                // Monaco lowercases authorities when it serializes URIs.
                if t.authority().eq_ignore_ascii_case(authority) && self.has_stdin() {
                    return Ok(t);
                }
            }
            let in_flight = self
                .connecting
                .lock()
                .unwrap()
                .as_deref()
                .is_some_and(|c| c.eq_ignore_ascii_case(authority));
            let elapsed = started.elapsed();
            if (!in_flight && elapsed > Duration::from_secs(3)) || elapsed > Duration::from_secs(120) {
                return Err(format!(
                    "remote '{authority}' is not connected — reopen it from Settings → Extensions → Remote development"
                ));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Call an agentz-server control method on whichever machine the host runs.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let frame = json!({ "$agentz": "req", "id": id, "method": method, "params": params });
        if let Err(e) = self.write_line(frame.to_string()).await {
            self.pending.lock().unwrap().remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(Duration::from_secs(180), rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err("extension host connection closed".into()),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("agentz-server request '{method}' timed out"))
            }
        }
    }

    pub async fn watch_remote(&self, token: &str, remote_path: &str) -> Result<(), String> {
        self.remote_watches
            .lock()
            .unwrap()
            .insert(token.to_string(), remote_path.to_string());
        self.request("fs.watch", json!({ "path": remote_path, "token": token }))
            .await
            .map(|_| ())
    }

    pub async fn unwatch_remote(&self, token: &str) {
        if self.remote_watches.lock().unwrap().remove(token).is_some() {
            let _ = self.request("fs.unwatch", json!({ "token": token })).await;
        }
    }

    async fn rearm_watches(&self) {
        let watches: Vec<(String, String)> = self
            .remote_watches
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (token, path) in watches {
            if let Err(e) = self.request("fs.watch", json!({ "path": path, "token": token })).await {
                warn!("re-arming remote watch {token} failed: {e}");
            }
        }
    }

    /// A held lock means a write is in flight, i.e. the host is attached.
    fn has_stdin(&self) -> bool {
        self.stdin.try_lock().map(|g| g.is_some()).unwrap_or(true)
    }

    async fn write_line(&self, mut line: String) -> Result<(), String> {
        if !line.ends_with('\n') {
            line.push('\n');
        }
        let write = async {
            let mut guard = self.stdin.lock().await;
            let stdin = guard.as_mut().ok_or("extension host is not running")?;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| format!("write to host stdin failed: {e}"))?;
            stdin
                .flush()
                .await
                .map_err(|e| format!("flush host stdin failed: {e}"))
        };
        tokio::time::timeout(Duration::from_secs(30), write)
            .await
            .map_err(|_| "extension host is not reading its input (busy or hung) — reconnect from the status bar".to_string())?
    }

    fn fail_pending(pending: &Pending, reason: &str) {
        for (_, tx) in pending.lock().unwrap().drain() {
            let _ = tx.send(Err(reason.to_string()));
        }
    }
}

/// Route a control frame from the server; returns false for ordinary RPC lines.
fn route_control(app: &AppHandle, pending: &Pending, line: &str) -> bool {
    if !line.starts_with("{\"$agentz\"") {
        return false;
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    match v.get("$agentz").and_then(|k| k.as_str()) {
        Some("res") => {
            let Some(id) = v.get("id").and_then(|i| i.as_u64()) else { return true };
            if let Some(tx) = pending.lock().unwrap().remove(&id) {
                let res = match v.get("error") {
                    Some(err) if !err.is_null() => Err(err
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("agentz-server error")
                        .to_string()),
                    _ => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                };
                let _ = tx.send(res);
            }
        }
        Some("event") => {
            if v.get("event").and_then(|e| e.as_str()) == Some("fs.change") {
                // The watch token is the workspace URI, so this matches what the
                // local watcher emits and the IDE's reload logic just works.
                let _ = app.emit(
                    "ide-file-changed",
                    json!({
                        "project_dir": v.get("token"),
                        "path": v.get("path"),
                        "kind": v.get("kind"),
                    }),
                );
            } else if !remote::forward::on_server_event(&v) {
                let _ = app.emit(REMOTE_EVENT, v);
            }
        }
        _ => {}
    }
    true
}

#[derive(Debug, Serialize)]
pub struct ExtHostStatus {
    pub running: bool,
    pub project_dir: Option<String>,
    pub host_js: String,
    pub target: Option<RemoteTarget>,
    pub authority: Option<String>,
    pub remote_home: Option<String>,
}

/// Directory holding the pinned Node runtimes (`npm run fetch-node`).
pub fn node_resources_dir(app: &AppHandle) -> Option<PathBuf> {
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/node");
    if dev.is_dir() {
        return Some(dev);
    }
    app.path()
        .resolve("resources/node", BaseDirectory::Resource)
        .ok()
        .filter(|p| p.is_dir())
}

/// Resolve the path to the bundled extension-host entry (`host.js`).
///
/// Precedence: explicit arg → `$CODEZ_EXT_HOST_JS` → Tauri resource bundle
/// (`extension-host/host.js` from `tauri.conf.json`) → dev build path → legacy
/// fallbacks relative to the executable / cwd.
fn resolve_host_js(app: &AppHandle, explicit: Option<String>) -> Result<PathBuf, String> {
    if let Some(p) = explicit.filter(|s| !s.is_empty()) {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
        return Err(format!("extension host js not found: {}", pb.display()));
    }
    if let Ok(env_path) = std::env::var("CODEZ_EXT_HOST_JS") {
        let pb = PathBuf::from(env_path);
        if pb.exists() {
            return Ok(pb);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    // Dev: always prefer a freshly built extension-host/dist over the stale
    // resource copy under target/debug (only refreshed on `cargo build`).
    let dev_host = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../extension-host/dist/host.js");
    candidates.push(dev_host.clone());
    if dev_host.exists() {
        return Ok(dev_host);
    }

    // Production bundle: tauri.conf.json maps dist/host.js → extension-host/host.js
    if let Ok(p) = app
        .path()
        .resolve("extension-host/host.js", BaseDirectory::Resource)
    {
        candidates.push(p.clone());
        if p.exists() {
            return Ok(p);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..6 {
            if let Some(d) = &dir {
                candidates.push(d.join("extension-host/dist/host.js"));
                candidates.push(d.join("resources/extension-host/host.js"));
                candidates.push(d.join("extension-host/host.js"));
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("extension-host/dist/host.js"));
        candidates.push(cwd.join("../extension-host/dist/host.js"));
    }
    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }
    Err(format!(
        "could not locate extension-host/dist/host.js (set CODEZ_EXT_HOST_JS). tried: {}",
        candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Node executable for the local host: `$CODEZ_NODE` → bundled pinned
/// runtime → system `node`.
fn node_bin(app: &AppHandle) -> String {
    if let Ok(n) = std::env::var("CODEZ_NODE") {
        return n;
    }
    let exe = if cfg!(windows) { "node.exe" } else { "node" };
    if let Some(dir) = node_resources_dir(app) {
        let p = dir
            .join(format!("{}-{}", std::env::consts::OS.replace("windows", "win32").replace("macos", "darwin"), node_arch()))
            .join(exe);
        if p.exists() {
            return p.display().to_string();
        }
    }
    "node".to_string()
}

fn node_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    }
}

/// Start the extension host for a project. Idempotent: a second call restarts.
///
/// With a non-local `remote` target the server bundle is deployed to that
/// machine and `project_dir` is interpreted as a path there.
#[tauri::command]
pub async fn ext_host_start(
    app: AppHandle,
    state: State<'_, AppState>,
    project_dir: String,
    host_js: Option<String>,
    remote: Option<RemoteTarget>,
) -> Result<ExtHostStatus, String> {
    let host_js_path = resolve_host_js(&app, host_js)?;
    let mgr = state.ext_host.clone();
    let target = remote.unwrap_or(RemoteTarget::Local);
    *mgr.connecting.lock().unwrap() = Some(target.authority());
    let _connecting = ConnectingGuard(mgr.connecting.clone());

    // Tear down any prior instance first.
    {
        let mut inner = mgr.inner.lock().await;
        if inner.target.as_ref().is_some_and(|t| *t != target) {
            remote::forward::stop_all();
            remote::lsp::stop_all();
        }
        inner.child = None;
        inner.server = None;
        inner.target = None;
    }
    // A writer stuck on the old pipe holds this lock; killing the child above
    // breaks the pipe so it errors out and releases it.
    *mgr.stdin.lock().await = None;
    ExtHostManager::fail_pending(&mgr.pending, "extension host restarted");

    let (mut command, server) = if target.is_local() {
        let node = node_bin(&app);
        let mut c = piscis_kernel::proc::tokio_command(&node);
        c.arg(&host_js_path).current_dir(&project_dir);
        (c, None)
    } else {
        let _ = app.emit(
            EXT_HOST_EVENT,
            json!({ "channel": "log", "data": format!("[remote] deploying agentz-server to {}", target.authority()) }),
        );
        let server = remote::deploy(&target, &host_js_path, node_resources_dir(&app).as_deref()).await?;
        let c = target.sh(&remote::launch_script(&server, &project_dir))?;
        (c, Some(server))
    };

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("failed to spawn extension host on {}: {}", target.authority(), e))?;

    let stdin = child.stdin.take().ok_or("failed to take host stdin")?;
    let stdout = child.stdout.take().ok_or("failed to take host stdout")?;
    let stderr = child.stderr.take();

    // stdout → renderer (RPC frames, one JSON object per line).
    let app_out = app.clone();
    let pending = mgr.pending.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() || route_control(&app_out, &pending, &line) {
                        continue;
                    }
                    let _ = app_out.emit(
                        EXT_HOST_EVENT,
                        json!({ "channel": "message", "data": line }),
                    );
                }
                Ok(None) => {
                    ExtHostManager::fail_pending(&pending, "extension host exited");
                    let _ = app_out.emit(
                        EXT_HOST_EVENT,
                        json!({ "channel": "exit", "data": "stdout closed" }),
                    );
                    break;
                }
                Err(e) => {
                    warn!("ext-host stdout read error: {}", e);
                    break;
                }
            }
        }
    });

    // stderr → renderer log channel (host diagnostics + extension errors).
    if let Some(stderr) = stderr {
        let app_err = app.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = app_err.emit(EXT_HOST_EVENT, json!({ "channel": "log", "data": line }));
            }
        });
    }

    {
        let mut inner = mgr.inner.lock().await;
        inner.child = Some(child);
        inner.project_dir = Some(project_dir.clone());
        inner.target = Some(target.clone());
        inner.server = server.clone();
    }
    *mgr.stdin.lock().await = Some(stdin);

    if !target.is_local() {
        let mgr = mgr.clone();
        tokio::spawn(async move { mgr.rearm_watches().await });
    }

    info!(
        "extension host started for {} on {} ({})",
        project_dir,
        target.authority(),
        host_js_path.display()
    );
    Ok(ExtHostStatus {
        running: true,
        project_dir: Some(project_dir),
        host_js: host_js_path.display().to_string(),
        authority: Some(target.authority()),
        remote_home: server.map(|s| s.home),
        target: Some(target),
    })
}

/// Send one RPC frame (a single JSON line) to the extension host's stdin.
#[tauri::command]
pub async fn ext_host_send(state: State<'_, AppState>, message: String) -> Result<(), String> {
    state.ext_host.write_line(message).await
}

/// Stop the extension host (kills the child via kill_on_drop).
#[tauri::command]
pub async fn ext_host_stop(state: State<'_, AppState>) -> Result<(), String> {
    let mgr = state.ext_host.clone();
    let mut inner = mgr.inner.lock().await;
    inner.child = None;
    inner.project_dir = None;
    inner.target = None;
    inner.server = None;
    drop(inner);
    *mgr.stdin.lock().await = None;
    remote::forward::stop_all();
    remote::lsp::stop_all();
    ExtHostManager::fail_pending(&mgr.pending, "extension host stopped");
    info!("extension host stopped");
    Ok(())
}

/// Report whether the host is running.
#[tauri::command]
pub async fn ext_host_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ExtHostStatus, String> {
    let mgr = state.ext_host.clone();
    let inner = mgr.inner.lock().await;
    Ok(ExtHostStatus {
        running: inner.child.is_some(),
        project_dir: inner.project_dir.clone(),
        host_js: resolve_host_js(&app, None)
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        target: inner.target.clone(),
        authority: inner.target.as_ref().map(|t| t.authority()),
        remote_home: inner.server.as_ref().map(|s| s.home.clone()),
    })
}
