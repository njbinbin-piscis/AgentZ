//! Session-owned Debug Adapter Protocol broker. Old transports cannot publish or stop a replacement.
use crate::state::AppState;
use serde::Serialize;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Child;
#[cfg(test)]
use tokio::process::ChildStdin;
use tokio::sync::Mutex;
use tracing::{info, warn};
#[path = "dap_frame.rs"]
mod dap_frame;

pub const DAP_EVENT: &str = "agentz:dap";
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const SESSION_BUDGET: usize = 4096;
/// Extra connections one TCP adapter may open (child debug targets).
const MAX_CHILD_SESSIONS: usize = 32;
const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
type Writer = Box<dyn AsyncWrite + Send + Unpin>;
fn validate_session_id(session_id: &str) -> Result<(), String> {
    let parsed = uuid::Uuid::parse_str(session_id).map_err(|_| "Invalid DAP session ID")?;
    if parsed.to_string() != session_id {
        return Err("DAP session ID must be a canonical UUID".into());
    }
    Ok(())
}
fn validate_remote_dir(
    remote_dir: Option<&str>,
) -> Result<Option<crate::remote::vfs::RemoteUri>, String> {
    remote_dir
        .map(|raw| {
            crate::remote::vfs::parse(raw).ok_or_else(|| "Invalid remote debug target".to_owned())
        })
        .transpose()
}
#[derive(Default)]
pub struct DapManager {
    inner: Mutex<DapInner>,
}
#[derive(Default)]
struct DapInner {
    // Lifetime tombstones: never evicted. At capacity new sessions fail closed.
    used_or_cancelled: HashSet<String>,
    generation: u64,
    session_id: Option<String>,
    child: Option<Child>,
    stdin: Option<Writer>,
    /// TCP adapters only: the loopback port, and the extra connections opened on it.
    adapter_port: Option<u16>,
    children: HashMap<String, Writer>,
}
#[derive(Clone, Debug)]
struct Lease {
    generation: u64,
    session_id: String,
}
impl Lease {
    fn matches(&self, inner: &DapInner) -> bool {
        inner.generation == self.generation
            && inner.session_id.as_deref() == Some(self.session_id.as_str())
    }
}
impl DapManager {
    pub fn new() -> Self {
        Self::default()
    }
    async fn reserve(&self, session_id: String) -> Result<(Lease, Option<Child>), String> {
        validate_session_id(&session_id)?;
        let mut inner = self.inner.lock().await;
        if inner.used_or_cancelled.contains(&session_id) {
            return Err("DAP session ID was already used or cancelled".into());
        }
        if inner.used_or_cancelled.len() >= SESSION_BUDGET {
            return Err("DAP session budget exhausted; restart the application".into());
        }
        inner.generation = inner
            .generation
            .checked_add(1)
            .ok_or("DAP generation exhausted")?;
        inner.used_or_cancelled.insert(session_id.clone());
        inner.stdin = None;
        inner.children.clear();
        inner.adapter_port = None;
        let old = inner.child.take();
        inner.session_id = Some(session_id.clone());
        Ok((
            Lease {
                generation: inner.generation,
                session_id,
            },
            old,
        ))
    }
    async fn take_owned(&self, session_id: &str, generation: Option<u64>) -> Option<Child> {
        if validate_session_id(session_id).is_err() {
            return None;
        }
        let mut inner = self.inner.lock().await;
        // Even a stop that arrives before start prevents later reservation.
        // When full, all new reservations already fail; no marker eviction.
        if inner.used_or_cancelled.len() < SESSION_BUDGET {
            inner.used_or_cancelled.insert(session_id.to_owned());
        }
        if inner.session_id.as_deref() != Some(session_id)
            || generation.is_some_and(|g| g != inner.generation)
        {
            return None;
        }
        inner.session_id = None;
        inner.stdin = None;
        inner.children.clear();
        inner.adapter_port = None;
        inner.child.take()
    }
    async fn spawn_owned(
        &self,
        lease: &Lease,
        mut command: tokio::process::Command,
    ) -> Result<
        (
            tokio::process::ChildStdout,
            Option<tokio::process::ChildStderr>,
        ),
        String,
    > {
        // No network await under this lock. Validation, synchronous spawn and
        // installation form one ownership boundary; cancelled preparation never spawns.
        let mut inner = self.inner.lock().await;
        if !lease.matches(&inner) {
            return Err("DAP start was superseded or stopped".into());
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|e| format!("failed to spawn debug adapter: {e}"))?;
        let stdin = child.stdin.take().ok_or("failed to take adapter stdin")?;
        let stdout = child.stdout.take().ok_or("failed to take adapter stdout")?;
        let stderr = child.stderr.take();
        inner.child = Some(child);
        inner.stdin = Some(Box::new(stdin));
        Ok((stdout, stderr))
    }
    /// TCP adapters: frames are written to the socket instead of the child's stdin.
    async fn set_writer(&self, lease: &Lease, writer: Writer, port: u16) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        if !lease.matches(&inner) {
            return Err("DAP start was superseded or stopped".into());
        }
        inner.stdin = Some(writer);
        inner.adapter_port = Some(port);
        Ok(())
    }
    #[cfg(test)]
    async fn install(&self, lease: &Lease, child: Child, stdin: ChildStdin) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        if !lease.matches(&inner) {
            return Err("DAP start was superseded or stopped".into());
        }
        inner.child = Some(child);
        inner.stdin = Some(Box::new(stdin));
        Ok(())
    }
}
async fn terminate(mut child: Option<Child>) {
    if let Some(child) = child.as_mut() {
        // kill_on_drop remains the fallback if kill/wait cannot complete.
        let _ = child.start_kill();
        let _ = tokio::time::timeout(IO_TIMEOUT, child.wait()).await;
    }
}
async fn emit_owned(app: &AppHandle, mgr: &DapManager, lease: &Lease, channel: &str, data: String) {
    let inner = mgr.inner.lock().await;
    if lease.matches(&inner) {
        // emit is synchronous; reservation cannot change between check and publication.
        let _ = app.emit(
            DAP_EVENT,
            json!({"session_id": lease.session_id, "channel": channel, "data": data}),
        );
    }
}
async fn emit_child(
    app: &AppHandle,
    mgr: &DapManager,
    lease: &Lease,
    child_id: &str,
    channel: &str,
    data: String,
    remove: bool,
) {
    let mut inner = mgr.inner.lock().await;
    if !lease.matches(&inner) || !inner.children.contains_key(child_id) {
        return;
    }
    if remove {
        inner.children.remove(child_id);
    }
    let _ = app.emit(
        DAP_EVENT,
        json!({"session_id": child_id, "channel": channel, "data": data}),
    );
}
async fn abandon(mgr: &DapManager, lease: &Lease) {
    terminate(
        mgr.take_owned(&lease.session_id, Some(lease.generation))
            .await,
    )
    .await;
}
#[derive(Debug, Serialize)]
pub struct DapStatus {
    pub running: bool,
    pub session_id: String,
    /// Loopback port of a TCP adapter; extra connections (child targets) use it.
    pub port: Option<u16>,
}
fn free_loopback_port() -> Result<u16, String> {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("no free local port for the debug adapter: {e}"))
}
/// Waits for a freshly spawned TCP adapter to start listening.
async fn connect_adapter(mgr: &DapManager, lease: &Lease, port: u16) -> Result<TcpStream, String> {
    let deadline = tokio::time::Instant::now() + TCP_CONNECT_TIMEOUT;
    loop {
        {
            let mut inner = mgr.inner.lock().await;
            if !lease.matches(&inner) {
                return Err("DAP start was superseded or stopped".into());
            }
            if let Some(child) = inner.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    return Err(format!(
                        "debug adapter exited before accepting a connection ({status})"
                    ));
                }
            }
        }
        match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Err(error) => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(format!(
                        "could not connect to the debug adapter on port {port}: {error}"
                    ));
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}
/// Frames read from an adapter connection become `message` events. The main
/// connection owns the session (its end terminates the adapter); child
/// connections only retire themselves.
fn spawn_reader<R>(app: AppHandle, mgr: Arc<DapManager>, lease: Lease, reader: R, child: Option<String>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = reader;
        let mut parser = dap_frame::FrameParser::default();
        let mut chunk = [0u8; 8192];
        let reason = loop {
            match reader.read(&mut chunk).await {
                Ok(0) => {
                    break parser
                        .finish()
                        .err()
                        .unwrap_or_else(|| "adapter stdout closed".into())
                }
                Ok(n) => match parser.push(&chunk[..n]) {
                    Ok(messages) => {
                        for message in messages {
                            match &child {
                                Some(id) => {
                                    emit_child(&app, &mgr, &lease, id, "message", message, false).await
                                }
                                None => emit_owned(&app, &mgr, &lease, "message", message).await,
                            }
                        }
                    }
                    Err(error) => break error,
                },
                Err(error) => break format!("DAP stdout read error: {error}"),
            }
        };
        match child {
            Some(id) => emit_child(&app, &mgr, &lease, &id, "exit", reason, true).await,
            None => {
                emit_owned(&app, &mgr, &lease, "exit", reason).await;
                abandon(&mgr, &lease).await;
            }
        }
    });
}
/// Adapter output that is not DAP traffic (stderr, or stdout of a TCP adapter).
fn spawn_log_pump<R>(app: AppHandle, mgr: Arc<DapManager>, lease: Lease, reader: R)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = reader;
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    emit_owned(
                        &app,
                        &mgr,
                        &lease,
                        "log",
                        String::from_utf8_lossy(&chunk[..n]).to_string(),
                    )
                    .await
                }
            }
        }
    });
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn dap_start(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    remote_dir: Option<String>,
    tcp: Option<bool>,
) -> Result<DapStatus, String> {
    // Invalid remote input must never fall back to local process execution,
    // and must not revoke an existing owner before validation.
    let remote = validate_remote_dir(remote_dir.as_deref())?;
    let tcp = tcp.unwrap_or(false);
    // `{port}` is the port the adapter listens on. Locally that is also where
    // the client connects. On a remote, the adapter listens on the remote
    // loopback and a forward carries that port back to this machine.
    let (args, port) = if tcp {
        let remote_port = free_loopback_port()?;
        let mut connect_port = remote_port;
        if let Some(uri) = &remote {
            let broker = crate::remote::manager().ok_or("remote broker unavailable")?;
            let target = broker.wait_for_authority(&uri.authority).await?;
            let fwd = crate::remote::forward::start(&target, remote_port, Some(remote_port)).await?;
            connect_port = fwd.local_port;
        }
        let shown = remote_port.to_string();
        (
            args.iter().map(|a| a.replace("{port}", &shown)).collect::<Vec<_>>(),
            Some(connect_port),
        )
    } else {
        (args, None)
    };
    let mgr = state.dap.clone();
    let (lease, old) = mgr.reserve(session_id).await?;
    terminate(old).await;
    let spawned = async {
        let cmd = match remote {
            Some(uri) => remote_adapter_command(&uri, &command, &args, cwd.as_deref()).await?,
            None => {
                let mut cmd = piscis_kernel::proc::tokio_command(&command);
                cmd.args(&args);
                if let Some(dir) = cwd.filter(|d| !d.is_empty()) {
                    cmd.current_dir(dir);
                }
                cmd
            }
        };
        Ok::<_, String>(cmd)
    }
    .await;
    let cmd = match spawned {
        Ok(command) => command,
        Err(error) => {
            abandon(&mgr, &lease).await;
            return Err(error);
        }
    };
    let (stdout, stderr) = match mgr.spawn_owned(&lease, cmd).await {
        Ok(pipes) => pipes,
        Err(error) => {
            abandon(&mgr, &lease).await;
            return Err(error);
        }
    };
    let reader: Box<dyn AsyncRead + Send + Unpin> = if let Some(port) = port {
        let stream = match connect_adapter(&mgr, &lease, port).await {
            Ok(stream) => stream,
            Err(error) => {
                abandon(&mgr, &lease).await;
                return Err(error);
            }
        };
        let (read_half, write_half) = stream.into_split();
        if let Err(error) = mgr.set_writer(&lease, Box::new(write_half), port).await {
            abandon(&mgr, &lease).await;
            return Err(error);
        }
        // The adapter's own stdout is plain log output in this mode.
        spawn_log_pump(app.clone(), mgr.clone(), lease.clone(), stdout);
        Box::new(read_half)
    } else {
        Box::new(stdout)
    };
    spawn_reader(app.clone(), mgr.clone(), lease.clone(), reader, None);
    if let Some(stderr) = stderr {
        spawn_log_pump(app.clone(), mgr.clone(), lease.clone(), stderr);
    }
    info!("debug adapter started for session {}", lease.session_id);
    Ok(DapStatus {
        running: true,
        session_id: lease.session_id,
        port,
    })
}

/// Opens another connection to the running TCP adapter (js-debug starts one
/// per debug target). It lives and dies with the parent session.
#[tauri::command]
pub async fn dap_tcp_child(
    app: AppHandle,
    state: State<'_, AppState>,
    parent_session_id: String,
    session_id: String,
    port: u16,
) -> Result<DapStatus, String> {
    validate_session_id(&parent_session_id)?;
    validate_session_id(&session_id)?;
    let mgr = state.dap.clone();
    let lease = {
        let inner = mgr.inner.lock().await;
        if inner.session_id.as_deref() != Some(parent_session_id.as_str()) {
            return Err("DAP session is not current".into());
        }
        if inner.adapter_port != Some(port) {
            return Err("port does not belong to the debug adapter".into());
        }
        if inner.children.len() >= MAX_CHILD_SESSIONS {
            return Err("too many debug target connections".into());
        }
        Lease {
            generation: inner.generation,
            session_id: parent_session_id,
        }
    };
    let stream = tokio::time::timeout(TCP_CONNECT_TIMEOUT, TcpStream::connect(("127.0.0.1", port)))
        .await
        .map_err(|_| "debug adapter connection timed out".to_owned())?
        .map_err(|e| format!("could not connect to the debug adapter: {e}"))?;
    let _ = stream.set_nodelay(true);
    let (read_half, write_half) = stream.into_split();
    {
        let mut inner = mgr.inner.lock().await;
        if !lease.matches(&inner) {
            return Err("DAP session ended while connecting".into());
        }
        if inner.children.contains_key(&session_id) {
            return Err("DAP child session ID already in use".into());
        }
        inner.children.insert(session_id.clone(), Box::new(write_half));
    }
    spawn_reader(app, mgr, lease, read_half, Some(session_id.clone()));
    Ok(DapStatus {
        running: true,
        session_id,
        port: Some(port),
    })
}

#[tauri::command]
pub async fn dap_send(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> Result<(), String> {
    let mgr = state.dap.clone();
    if message.len() > 1024 * 1024 {
        return Err("DAP outgoing message limit exceeded".into());
    }
    let value: serde_json::Value =
        serde_json::from_str(&message).map_err(|_| "DAP message must be JSON")?;
    if !value.is_object() {
        return Err("DAP message must be an object".into());
    }
    let mut inner = mgr.inner.lock().await;
    let is_main = inner.session_id.as_deref() == Some(session_id.as_str());
    if !is_main && !inner.children.contains_key(&session_id) {
        return Err("DAP session is not current".into());
    }
    let writer: &mut Writer = if is_main {
        inner.stdin.as_mut().ok_or("debug adapter is not running")?
    } else {
        inner
            .children
            .get_mut(&session_id)
            .ok_or("debug adapter is not running")?
    };
    let result = tokio::time::timeout(IO_TIMEOUT, async {
        writer
            .write_all(format!("Content-Length: {}\r\n\r\n", message.len()).as_bytes())
            .await?;
        writer.write_all(message.as_bytes()).await?;
        writer.flush().await
    })
    .await;
    let error = match result {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(error)) => format!("DAP write failed: {error}"),
        Err(_) => "DAP write timed out".to_owned(),
    };
    // Partial frames cannot be retried. Revoke only the currently locked owner.
    let _ = app.emit(
        DAP_EVENT,
        json!({ "session_id": session_id, "channel": "exit", "data": error }),
    );
    if !is_main {
        inner.children.remove(&session_id);
        warn!("{}", error);
        return Err(error);
    }
    inner.session_id = None;
    inner.stdin = None;
    inner.children.clear();
    inner.adapter_port = None;
    let child = inner.child.take();
    drop(inner);
    terminate(child).await;
    warn!("{}", error);
    Err(error)
}
#[tauri::command]
pub async fn dap_stop(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    validate_session_id(&session_id)?;
    // A child connection is closed on its own; the parent session keeps running.
    if state.dap.inner.lock().await.children.remove(&session_id).is_some() {
        return Ok(());
    }
    // A stale stop is an idempotent no-op; it must never stop the replacement.
    terminate(state.dap.take_owned(&session_id, None).await).await;
    Ok(())
}

/// The adapter runs on the remote host (next to the debuggee), spawned through
/// the target transport with stdio piped.
async fn remote_adapter_command(
    uri: &crate::remote::vfs::RemoteUri,
    command: &str,
    args: &[String],
    cwd: Option<&str>,
) -> Result<tokio::process::Command, String> {
    let mgr = crate::remote::manager().ok_or("remote broker unavailable")?;
    let target = mgr.wait_for_authority(&uri.authority).await?;
    let dir = cwd.filter(|d| d.starts_with('/')).unwrap_or(&uri.path);
    target.sh(&remote_adapter_script(dir, command, args))
}

pub(crate) fn remote_adapter_script(dir: &str, command: &str, args: &[String]) -> String {
    use crate::remote::shell_quote;
    // Many distros ship only `python3`.
    let exe = if command == "python" {
        "\"$(command -v python3 || command -v python)\"".to_string()
    } else {
        shell_quote(command)
    };
    let argv: Vec<String> = args.iter().map(|a| shell_quote(a)).collect();
    format!(
        "cd {} || exit 1; {}; exec {exe} {}",
        shell_quote(dir),
        crate::remote::TOOL_PATH_SETUP,
        argv.join(" ")
    )
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    fn id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
    fn fixture_child() -> (Child, ChildStdin) {
        #[cfg(windows)]
        let mut command = {
            let mut c = piscis_kernel::proc::tokio_command("cmd");
            c.args(["/D", "/C", "set /p fixture_hold="]);
            c
        };
        #[cfg(not(windows))]
        let mut command = {
            let mut c = piscis_kernel::proc::tokio_command("sh");
            c.args(["-c", "read -r fixture_hold"]);
            c
        };
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        (child, stdin)
    }
    #[tokio::test]
    async fn stale_stop_and_start_failure_preserve_replacement() {
        let manager = DapManager::new();
        let (a, _) = manager.reserve(id()).await.unwrap();
        let (b, _) = manager.reserve(id()).await.unwrap();
        assert!(b.generation > a.generation);
        assert!(manager.take_owned(&a.session_id, None).await.is_none());
        assert!(manager
            .take_owned(&a.session_id, Some(a.generation))
            .await
            .is_none());
        assert!(b.matches(&*manager.inner.lock().await));
    }
    #[tokio::test]
    async fn stop_revokes_pending_start_before_install() {
        let manager = DapManager::new();
        let (a, _) = manager.reserve(id()).await.unwrap();
        assert!(manager.take_owned(&a.session_id, None).await.is_none());
        let (child, stdin) = fixture_child();
        assert!(manager.install(&a, child, stdin).await.is_err());
        assert!(manager.inner.lock().await.child.is_none());
        assert!(manager.inner.lock().await.session_id.is_none());
    }
    #[tokio::test]
    async fn stale_install_cannot_replace_owned_child() {
        let manager = DapManager::new();
        let (a, _) = manager.reserve(id()).await.unwrap();
        let (b, _) = manager.reserve(id()).await.unwrap();
        let (child, stdin) = fixture_child();
        manager.install(&b, child, stdin).await.unwrap();
        let current_pid = manager.inner.lock().await.child.as_ref().unwrap().id();
        let (child, stdin) = fixture_child();
        assert!(manager.install(&a, child, stdin).await.is_err());
        assert!(manager.take_owned(&a.session_id, None).await.is_none());
        assert_eq!(
            manager.inner.lock().await.child.as_ref().unwrap().id(),
            current_pid
        );
        terminate(manager.take_owned(&b.session_id, None).await).await;
        assert!(manager.inner.lock().await.child.is_none());
    }
    #[tokio::test]
    async fn invalid_and_duplicate_reservations_fail_without_revocation() {
        let manager = DapManager::new();
        let (a, _) = manager.reserve(id()).await.unwrap();
        assert!(manager.reserve("not-a-uuid".into()).await.is_err());
        assert!(manager.reserve(a.session_id.clone()).await.is_err());
        assert!(a.matches(&*manager.inner.lock().await));
    }
    #[tokio::test]
    async fn stop_before_start_and_uuid_replay_are_rejected() {
        let manager = DapManager::new();
        let cancelled = id();
        assert!(manager.take_owned(&cancelled, None).await.is_none());
        assert!(manager.reserve(cancelled).await.is_err());
        let (a, _) = manager.reserve(id()).await.unwrap();
        manager.take_owned(&a.session_id, None).await;
        assert!(manager.reserve(a.session_id).await.is_err());
        assert!(manager
            .reserve("A0000000-0000-0000-0000-000000000000".into())
            .await
            .is_err());
    }
    #[tokio::test]
    async fn session_budget_exhaustion_never_evicts_cancellations_or_owner() {
        let manager = DapManager::new();
        let (owner, _) = manager.reserve(id()).await.unwrap();
        for n in 1..SESSION_BUDGET {
            manager
                .take_owned(&uuid::Uuid::from_u128(n as u128).to_string(), None)
                .await;
        }
        assert_eq!(
            manager.inner.lock().await.used_or_cancelled.len(),
            SESSION_BUDGET
        );
        let last_cancel = id();
        manager.take_owned(&last_cancel, None).await;
        assert!(manager.reserve(last_cancel).await.is_err());
        assert!(manager.reserve(id()).await.is_err());
        assert!(owner.matches(&*manager.inner.lock().await));
        assert_eq!(
            manager.inner.lock().await.used_or_cancelled.len(),
            SESSION_BUDGET
        );
    }
    #[tokio::test]
    async fn cancelled_preparation_does_not_attempt_spawn() {
        let manager = DapManager::new();
        let (lease, _) = manager.reserve(id()).await.unwrap();
        manager.take_owned(&lease.session_id, None).await;
        // This nonexistent executable would report a spawn error if attempted.
        let command = piscis_kernel::proc::tokio_command("agentz-owned-fixture-never-spawn-this");
        let error = manager.spawn_owned(&lease, command).await.err().unwrap();
        assert_eq!(error, "DAP start was superseded or stopped");
    }
    #[tokio::test]
    async fn tcp_adapter_connect_waits_for_listener_and_respects_revocation() {
        let manager = DapManager::new();
        let (lease, _) = manager.reserve(id()).await.unwrap();
        let port = free_loopback_port().unwrap();
        // The adapter starts listening only after the first attempts have failed.
        let listener_task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
            let _ = listener.accept().await;
        });
        assert!(connect_adapter(&manager, &lease, port).await.is_ok());
        listener_task.abort();
        manager.take_owned(&lease.session_id, None).await;
        let error = connect_adapter(&manager, &lease, port).await.err().unwrap();
        assert_eq!(error, "DAP start was superseded or stopped");
    }
    #[tokio::test]
    async fn revoking_the_parent_drops_child_connections() {
        let manager = DapManager::new();
        let (lease, _) = manager.reserve(id()).await.unwrap();
        {
            let mut inner = manager.inner.lock().await;
            inner.adapter_port = Some(1);
            inner.children.insert(id(), Box::new(tokio::io::sink()));
        }
        manager.take_owned(&lease.session_id, None).await;
        let inner = manager.inner.lock().await;
        assert!(inner.children.is_empty());
        assert!(inner.adapter_port.is_none());
    }
    #[test]
    fn malformed_remote_never_becomes_local() {
        assert!(validate_remote_dir(None).unwrap().is_none());
        assert_eq!(
            validate_remote_dir(Some("agentz-remote://target/Work"))
                .unwrap()
                .unwrap()
                .path,
            "/Work"
        );
        for input in [
            "",
            "C:/Work",
            "ssh://host/Work",
            "agentz-remote:///Work",
            "agentz-remote:bad",
            "agentz-remote://target%00/Work",
        ] {
            assert!(validate_remote_dir(Some(input)).is_err(), "{input}");
        }
    }
}
