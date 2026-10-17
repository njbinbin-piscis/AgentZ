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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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

fn current_generation(
    generation: &std::sync::Mutex<u64>,
    expected: u64,
) -> Option<std::sync::MutexGuard<'_, u64>> {
    let current = generation.lock().unwrap();
    (*current == expected).then_some(current)
}

/// Shared lifecycle state for the (single) extension host process.
#[derive(Default)]
pub struct ExtHostManager {
    lifecycle: Mutex<()>,
    /// Reader callbacks and lifecycle invalidation share this lock so stale
    /// responses/EOF cannot race into the next connection's pending requests.
    generation: Arc<std::sync::Mutex<u64>>,
    connected: Arc<AtomicBool>,
    watch_operations: Mutex<()>,
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

    pub(crate) async fn deployment_snapshot(
        &self,
    ) -> Result<(RemoteTarget, Option<DeployedServer>), String> {
        let inner = self.inner.lock().await;
        let _generation = self.generation.lock().unwrap();
        if !self.connected.load(Ordering::Acquire) {
            return Err("extension host is not connected".into());
        }
        let target = inner.target.clone().ok_or("extension host has no target")?;
        Ok((target, inner.server.clone()))
    }

    pub(crate) async fn bind_authority(&self, authority: &str) -> Result<u64, String> {
        let generation = *self.generation.lock().unwrap();
        self.wait_for_authority(authority).await?;
        let _current = current_generation(&self.generation, generation)
            .ok_or("remote connection changed while resolving workspace")?;
        if !self.connected.load(Ordering::Acquire) {
            return Err("remote connection closed while resolving workspace".into());
        }
        Ok(generation)
    }

    pub(crate) async fn target_for_generation(
        &self,
        generation: u64,
    ) -> Result<RemoteTarget, String> {
        let target = self
            .target()
            .await
            .ok_or("remote connection has no target")?;
        let _current = current_generation(&self.generation, generation)
            .ok_or("remote connection changed while capturing target")?;
        if !self.connected.load(Ordering::Acquire) {
            return Err("remote connection closed while capturing target".into());
        }
        Ok(target)
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
            if (!in_flight && elapsed > Duration::from_secs(3))
                || elapsed > Duration::from_secs(120)
            {
                return Err(format!(
                    "remote '{authority}' is not connected — reopen it from Settings → Extensions → Remote development"
                ));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Call an agentz-server control method on whichever machine the host runs.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let generation = *self.generation.lock().unwrap();
        self.request_generation(method, params, generation).await
    }

    pub(crate) async fn request_generation(
        &self,
        method: &str,
        params: Value,
        generation: u64,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let frame = json!({ "$agentz": "req", "id": id, "method": method, "params": params });
        if let Err(e) = self
            .write_line_generation(frame.to_string(), generation)
            .await
        {
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
        let _operation = self.watch_operations.lock().await;
        let uri = remote::vfs::parse(token).ok_or("invalid remote watch URI")?;
        let generation = self.bind_authority(&uri.authority).await?;
        self.remote_watches
            .lock()
            .unwrap()
            .insert(token.to_string(), remote_path.to_string());
        self.request_generation(
            "fs.watch",
            json!({ "path": remote_path, "token": token }),
            generation,
        )
        .await
        .map(|_| ())
    }

    pub async fn unwatch_remote(&self, token: &str) {
        let _operation = self.watch_operations.lock().await;
        if self.remote_watches.lock().unwrap().remove(token).is_some() {
            if let Some(uri) = remote::vfs::parse(token) {
                if let Ok(generation) = self.bind_authority(&uri.authority).await {
                    let _ = self
                        .request_generation("fs.unwatch", json!({ "token": token }), generation)
                        .await;
                }
            }
        }
    }

    async fn rearm_watches(&self, generation: u64, authority: String) {
        let watches: Vec<(String, String)> = self
            .remote_watches
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (token, path) in watches {
            let _operation = self.watch_operations.lock().await;
            if self.remote_watches.lock().unwrap().get(&token) != Some(&path) {
                continue;
            }
            if !remote::vfs::parse(&token)
                .is_some_and(|uri| uri.authority.eq_ignore_ascii_case(&authority))
            {
                continue;
            }
            if let Err(e) = self
                .request_generation(
                    "fs.watch",
                    json!({ "path": path, "token": token }),
                    generation,
                )
                .await
            {
                warn!("re-arming remote watch {token} failed: {e}");
            }
        }
    }

    /// A held lock means a write is in flight, i.e. the host is attached.
    fn has_stdin(&self) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        self.stdin.try_lock().map(|g| g.is_some()).unwrap_or(true)
    }

    async fn write_line(&self, line: String) -> Result<(), String> {
        let generation = *self.generation.lock().unwrap();
        self.write_line_generation(line, generation).await
    }

    async fn write_line_generation(&self, mut line: String, generation: u64) -> Result<(), String> {
        if !line.ends_with('\n') {
            line.push('\n');
        }
        let write = async {
            let mut guard = self.stdin.lock().await;
            if current_generation(&self.generation, generation).is_none() {
                return Err(
                    "extension host connection changed; request was not replayed".to_string(),
                );
            }
            if !self.connected.load(Ordering::Acquire) {
                return Err("extension host connection closed".to_string());
            }
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
fn route_control(app: &AppHandle, pending: &Pending, line: &str, generation: u64) -> bool {
    if !line.starts_with("{\"$agentz\"") {
        return false;
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    match v.get("$agentz").and_then(|k| k.as_str()) {
        Some("res") => {
            let Some(id) = v.get("id").and_then(|i| i.as_u64()) else {
                return true;
            };
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
            } else if !remote::forward::on_server_event_generation(&v, generation) {
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

/// Counts only; no extension, project, or remote resource identifiers leave the host.
#[derive(Debug, Serialize)]
pub struct ExtHostResourceSnapshot {
    pub child_present: bool,
    pub connected: bool,
    pub pending_requests: usize,
    pub remote_watches: usize,
}

#[tauri::command]
pub async fn ext_host_resource_snapshot(
    state: State<'_, AppState>,
) -> Result<ExtHostResourceSnapshot, String> {
    let mgr = &state.ext_host;
    let child_present = mgr.inner.lock().await.child.is_some();
    let connected = mgr.connected.load(Ordering::Acquire);
    let pending_requests = mgr.pending.lock().unwrap().len();
    let remote_watches = mgr.remote_watches.lock().unwrap().len();
    Ok(ExtHostResourceSnapshot {
        child_present,
        connected,
        pending_requests,
        remote_watches,
    })
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
            .join(format!(
                "{}-{}",
                std::env::consts::OS
                    .replace("windows", "win32")
                    .replace("macos", "darwin"),
                node_arch()
            ))
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
    let _lifecycle = mgr.lifecycle.lock().await;
    let generation = {
        let mut current = mgr.generation.lock().unwrap();
        *current += 1;
        mgr.connected.store(false, Ordering::Release);
        *current
    };
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
    remote::forward::begin_generation(generation);

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
        let server =
            remote::deploy(&target, &host_js_path, node_resources_dir(&app).as_deref()).await?;
        let c = target.sh(&remote::launch_script(&server, &project_dir))?;
        (c, Some(server))
    };

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            format!(
                "failed to spawn extension host on {}: {}",
                target.authority(),
                e
            )
        })?;

    let stdin = child.stdin.take().ok_or("failed to take host stdin")?;
    let stdout = child.stdout.take().ok_or("failed to take host stdout")?;
    let stderr = child.stderr.take();

    // stdout → renderer (RPC frames, one JSON object per line).
    let app_out = app.clone();
    let pending = mgr.pending.clone();
    let reader_generation = mgr.generation.clone();
    let connected = mgr.connected.clone();
    connected.store(true, Ordering::Release);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        loop {
            let result = reader.next_line().await;
            let Some(_current) = current_generation(&reader_generation, generation) else {
                break;
            };
            match result {
                Ok(Some(line)) => {
                    if line.trim().is_empty()
                        || route_control(&app_out, &pending, &line, generation)
                    {
                        continue;
                    }
                    let _ = app_out.emit(
                        EXT_HOST_EVENT,
                        json!({ "channel": "message", "data": line }),
                    );
                }
                Ok(None) => {
                    connected.store(false, Ordering::Release);
                    remote::forward::end_generation(generation);
                    ExtHostManager::fail_pending(&pending, "extension host exited");
                    let _ = app_out.emit(
                        EXT_HOST_EVENT,
                        json!({ "channel": "exit", "data": "stdout closed" }),
                    );
                    break;
                }
                Err(e) => {
                    connected.store(false, Ordering::Release);
                    remote::forward::end_generation(generation);
                    ExtHostManager::fail_pending(&pending, "extension host stdout read failed");
                    let _ = app_out.emit(
                        EXT_HOST_EVENT,
                        json!({ "channel": "exit", "data": "stdout read failed" }),
                    );
                    warn!("ext-host stdout read error: {}", e);
                    break;
                }
            }
        }
    });

    // stderr → renderer log channel (host diagnostics + extension errors).
    if let Some(stderr) = stderr {
        let app_err = app.clone();
        let reader_generation = mgr.generation.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let Some(_current) = current_generation(&reader_generation, generation) else {
                    break;
                };
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
        let authority = target.authority();
        tokio::spawn(async move { mgr.rearm_watches(generation, authority).await });
    }

    info!(
        "extension host started for {} on {} ({})",
        project_dir,
        target.authority(),
        host_js_path.display()
    );
    Ok(ExtHostStatus {
        running: mgr.connected.load(Ordering::Acquire),
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
    let _lifecycle = mgr.lifecycle.lock().await;
    let generation = {
        let mut current = mgr.generation.lock().unwrap();
        *current += 1;
        *current
    };
    mgr.connected.store(false, Ordering::Release);
    let mut inner = mgr.inner.lock().await;
    inner.child = None;
    inner.project_dir = None;
    inner.target = None;
    inner.server = None;
    drop(inner);
    *mgr.stdin.lock().await = None;
    remote::forward::stop_all();
    remote::forward::begin_generation(generation);
    remote::forward::end_generation(generation);
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
        running: inner.child.is_some() && mgr.connected.load(Ordering::Acquire),
        project_dir: inner.project_dir.clone(),
        host_js: resolve_host_js(&app, None)
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        target: inner.target.clone(),
        authority: inner.target.as_ref().map(|t| t.authority()),
        remote_home: inner.server.as_ref().map(|s| s.home.clone()),
    })
}

#[cfg(test)]
mod connection_generation_tests {
    use super::*;

    #[tokio::test]
    async fn stale_session_sends_no_frame_to_replacement_child() {
        #[cfg(windows)]
        let mut command = {
            let mut cmd = tokio::process::Command::new("powershell");
            cmd.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "while ($null -ne ($line = [Console]::ReadLine())) { [Console]::WriteLine($line) }",
            ]);
            cmd
        };
        #[cfg(not(windows))]
        let mut command = {
            let mut cmd = tokio::process::Command::new("sh");
            cmd.args([
                "-c",
                "while IFS= read -r line; do printf '%s\\n' \"$line\"; done",
            ]);
            cmd
        };
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mgr = Arc::new(ExtHostManager::new());
        *mgr.generation.lock().unwrap() = 2;
        mgr.connected.store(true, Ordering::Release);
        *mgr.stdin.lock().await = child.stdin.take();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        mgr.write_line_generation("replacement-ready".into(), 2)
            .await
            .unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(15), lines.next_line())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            "replacement-ready"
        );
        let session = remote::RemoteSession {
            target: RemoteTarget::Ssh { host: "a".into() },
            authority: "ssh-remote+a".into(),
            path: "/workspace".into(),
            generation: 1,
            manager: mgr.clone(),
        };
        let result = session
            .call("fs.writeFile", json!({"path":"/workspace/file"}))
            .await;
        assert!(result.unwrap_err().contains("request was not replayed"));
        assert!(
            tokio::time::timeout(Duration::from_millis(200), lines.next_line())
                .await
                .is_err()
        );
        assert!(mgr.pending.lock().unwrap().is_empty());
        child.kill().await.unwrap();
        let _ = child.wait().await;
    }

    #[tokio::test]
    async fn bound_remote_call_rejects_connection_switch() {
        let mgr = Arc::new(ExtHostManager::new());
        *mgr.generation.lock().unwrap() = 1;
        let session = remote::RemoteSession {
            target: RemoteTarget::Ssh { host: "a".into() },
            authority: "ssh-remote+a".into(),
            path: "/workspace".into(),
            generation: 1,
            manager: mgr.clone(),
        };
        *mgr.generation.lock().unwrap() = 2;
        mgr.connected.store(true, Ordering::Release);
        let result = remote::with_session(
            session,
            remote::call(
                "fs.writeFile",
                json!({"path":"/workspace/file", "base64":""}),
            ),
        )
        .await;
        assert!(result.unwrap_err().contains("request was not replayed"));
        assert!(mgr.pending.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn bound_resolution_rejects_other_authority() {
        let session = remote::RemoteSession {
            target: RemoteTarget::Ssh { host: "a".into() },
            authority: "ssh-remote+a".into(),
            path: "/workspace".into(),
            generation: 1,
            manager: Arc::new(ExtHostManager::new()),
        };
        remote::with_session(session, async {
            assert_eq!(
                remote::resolve("agentz-remote://ssh-remote+a/workspace")
                    .await
                    .unwrap(),
                Some("/workspace".into())
            );
            assert!(remote::resolve("agentz-remote://ssh-remote+b/workspace")
                .await
                .unwrap_err()
                .contains("authority differs"));
        })
        .await;
    }

    #[tokio::test]
    async fn stale_write_is_rejected_before_touching_new_pipe() {
        let mgr = ExtHostManager::new();
        *mgr.generation.lock().unwrap() = 2;
        mgr.connected.store(true, Ordering::Release);
        let error = mgr
            .write_line_generation("frame".into(), 1)
            .await
            .unwrap_err();
        assert!(error.contains("request was not replayed"));
        assert!(mgr.pending.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn disconnected_host_rejects_writes_and_readiness() {
        let mgr = ExtHostManager::new();
        assert!(!mgr.has_stdin());
        assert_eq!(
            mgr.write_line("frame".into()).await.unwrap_err(),
            "extension host connection closed"
        );
    }

    #[test]
    fn stale_exit_preserves_new_connection_request() {
        let generation = std::sync::Mutex::new(2);
        let pending: Pending = Default::default();
        let (tx, mut rx) = oneshot::channel();
        pending.lock().unwrap().insert(7, tx);
        if let Some(_current) = current_generation(&generation, 1) {
            ExtHostManager::fail_pending(&pending, "old host exited");
        }
        assert_eq!(pending.lock().unwrap().len(), 1);
        assert!(matches!(
            rx.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        if let Some(_current) = current_generation(&generation, 2) {
            ExtHostManager::fail_pending(&pending, "current host exited");
        }
        assert_eq!(rx.try_recv().unwrap().unwrap_err(), "current host exited");
        assert!(pending.lock().unwrap().is_empty());
    }

    #[test]
    fn callback_guard_excludes_lifecycle_invalidation() {
        let generation = std::sync::Mutex::new(1);
        let guard = current_generation(&generation, 1).unwrap();
        assert!(generation.try_lock().is_err());
        drop(guard);
        *generation.lock().unwrap() = 2;
        assert!(current_generation(&generation, 1).is_none());
        assert!(current_generation(&generation, 2).is_some());
    }
}
