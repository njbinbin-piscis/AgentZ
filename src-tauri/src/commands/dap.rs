//! Session-owned Debug Adapter Protocol broker. Old transports cannot publish or stop a replacement.
use crate::state::AppState;
use serde::Serialize;
use serde_json::json;
use std::{collections::HashSet, process::Stdio, time::Duration};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin};
use tokio::sync::Mutex;
use tracing::{info, warn};
#[path = "dap_frame.rs"]
mod dap_frame;

pub const DAP_EVENT: &str = "agentz:dap";
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const SESSION_BUDGET: usize = 4096;
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
    stdin: Option<ChildStdin>,
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
        inner.stdin = Some(stdin);
        Ok((stdout, stderr))
    }
    #[cfg(test)]
    async fn install(&self, lease: &Lease, child: Child, stdin: ChildStdin) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        if !lease.matches(&inner) {
            return Err("DAP start was superseded or stopped".into());
        }
        inner.child = Some(child);
        inner.stdin = Some(stdin);
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
#[derive(Debug, Serialize)]
pub struct DapStatus {
    pub running: bool,
    pub session_id: String,
}

#[tauri::command]
pub async fn dap_start(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    remote_dir: Option<String>,
) -> Result<DapStatus, String> {
    // Invalid remote input must never fall back to local process execution,
    // and must not revoke an existing owner before validation.
    let remote = validate_remote_dir(remote_dir.as_deref())?;
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
            terminate(
                mgr.take_owned(&lease.session_id, Some(lease.generation))
                    .await,
            )
            .await;
            return Err(error);
        }
    };
    let (stdout, stderr) = match mgr.spawn_owned(&lease, cmd).await {
        Ok(pipes) => pipes,
        Err(error) => {
            terminate(
                mgr.take_owned(&lease.session_id, Some(lease.generation))
                    .await,
            )
            .await;
            return Err(error);
        }
    };
    let app_out = app.clone();
    let out_mgr = mgr.clone();
    let out_lease = lease.clone();
    tokio::spawn(async move {
        let mut reader = stdout;
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
                            emit_owned(&app_out, &out_mgr, &out_lease, "message", message).await;
                        }
                    }
                    Err(error) => break error,
                },
                Err(error) => break format!("DAP stdout read error: {error}"),
            }
        };
        emit_owned(&app_out, &out_mgr, &out_lease, "exit", reason).await;
        terminate(
            out_mgr
                .take_owned(&out_lease.session_id, Some(out_lease.generation))
                .await,
        )
        .await;
    });
    if let Some(mut stderr) = stderr {
        let app_err = app.clone();
        let err_mgr = mgr.clone();
        let err_lease = lease.clone();
        tokio::spawn(async move {
            let mut chunk = [0u8; 4096];
            loop {
                match stderr.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        emit_owned(
                            &app_err,
                            &err_mgr,
                            &err_lease,
                            "log",
                            String::from_utf8_lossy(&chunk[..n]).to_string(),
                        )
                        .await
                    }
                }
            }
        });
    }
    info!("debug adapter started for session {}", lease.session_id);
    Ok(DapStatus {
        running: true,
        session_id: lease.session_id,
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
    if inner.session_id.as_deref() != Some(session_id.as_str()) {
        return Err("DAP session is not current".into());
    }
    let stdin = inner.stdin.as_mut().ok_or("debug adapter is not running")?;
    let result = tokio::time::timeout(IO_TIMEOUT, async {
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n", message.len()).as_bytes())
            .await?;
        stdin.write_all(message.as_bytes()).await?;
        stdin.flush().await
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
    inner.session_id = None;
    inner.stdin = None;
    let child = inner.child.take();
    drop(inner);
    terminate(child).await;
    warn!("{}", error);
    Err(error)
}
#[tauri::command]
pub async fn dap_stop(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    validate_session_id(&session_id)?;
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
