//! Port forwarding from the remote workspace to `127.0.0.1` on this machine.
//!
//! SSH targets use the client's native `-L` forwarding. Docker / WSL targets
//! have no SSH, so each accepted local connection is tunnelled through the
//! agentz-server stdio (`tcp.connect` / `tcp.write` / `tcp.data` events).

use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use super::{manager, RemoteTarget};

#[derive(Debug, Clone, Serialize)]
pub struct Forward {
    pub remote_port: u16,
    pub local_port: u16,
    pub via: &'static str,
}

enum Handle {
    Ssh(Box<SshHandle>),
    Tunnel(JoinHandle<()>),
}

struct SshHandle {
    child: tokio::process::Child,
    stderr: Option<JoinHandle<()>>,
}

impl Drop for SshHandle {
    fn drop(&mut self) {
        if let Some(stderr) = &self.stderr {
            stderr.abort();
        }
    }
}

const STDERR_TAIL_BYTES: usize = 8 * 1024;

#[derive(Default)]
struct DiagnosticTail(VecDeque<u8>);

impl DiagnosticTail {
    fn append(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if self.0.len() == STDERR_TAIL_BYTES {
                self.0.pop_front();
            }
            self.0.push_back(*byte);
        }
    }

    fn message(&self) -> String {
        String::from_utf8_lossy(&self.0.iter().copied().collect::<Vec<_>>())
            .trim()
            .into()
    }
}

async fn drain_diagnostics(
    mut stderr: impl tokio::io::AsyncRead + Unpin,
    tail: Arc<Mutex<DiagnosticTail>>,
) {
    let mut bytes = [0u8; 4096];
    loop {
        match stderr.read(&mut bytes).await {
            Ok(0) | Err(_) => break,
            Ok(n) => tail.lock().unwrap().append(&bytes[..n]),
        }
    }
}

async fn wait_for_listener(
    child: &mut tokio::process::Child,
    port: u16,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("inspect SSH forward: {e}"))?
        {
            return Err(format!("SSH forward exited: {status}"));
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("SSH forward timed out waiting for local listener".into());
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if matches!(
            tokio::time::timeout(
                remaining.min(Duration::from_millis(100)),
                tokio::net::TcpStream::connect(("127.0.0.1", port))
            )
            .await,
            Ok(Ok(_))
        ) {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err("SSH forward exited while its listener became ready".into());
            }
            return Ok(());
        }
        tokio::time::sleep_until(
            deadline.min(tokio::time::Instant::now() + Duration::from_millis(25)),
        )
        .await;
    }
}

struct Entry {
    authority: String,
    info: Forward,
    _handle: Handle,
}

impl Entry {
    fn reusable_for(&mut self, authority: &str) -> bool {
        self.authority.eq_ignore_ascii_case(authority) && self.is_alive()
    }

    fn is_alive(&mut self) -> bool {
        match &mut self._handle {
            Handle::Ssh(handle) => matches!(handle.child.try_wait(), Ok(None)),
            Handle::Tunnel(task) => !task.is_finished(),
        }
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        if let Handle::Tunnel(h) = &self._handle {
            h.abort();
        }
    }
}

fn forwards() -> &'static Mutex<HashMap<u16, Entry>> {
    static F: OnceLock<Mutex<HashMap<u16, Entry>>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stop_epoch() -> &'static std::sync::atomic::AtomicU64 {
    static EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    &EPOCH
}

/// Advance the broker generation even when the remote authority is unchanged.
/// SSH forwards are independent; broker-backed listeners must be recreated.
pub fn begin_generation(generation: u64) {
    let mut forwards = forwards().lock().unwrap();
    let mut map = tunnels().lock().unwrap();
    let mut pending = early().lock().unwrap();
    if !pending.advance(generation) {
        return;
    }
    stop_epoch().fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    forwards.retain(|_, entry| matches!(entry._handle, Handle::Ssh(_)));
    for sink in map.values() {
        sink.closed.send_replace(true);
    }
    map.clear();
}

/// EOF from a superseded reader must not close a replacement connection.
pub fn end_generation(generation: u64) {
    let mut forwards = forwards().lock().unwrap();
    let mut map = tunnels().lock().unwrap();
    let mut pending = early().lock().unwrap();
    if pending.generation != Some(generation) {
        return;
    }
    stop_epoch().fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    forwards.retain(|_, entry| matches!(entry._handle, Handle::Ssh(_)));
    fail_tunnels(&mut map, &mut pending, generation);
}

/// A chunk from the remote socket; `None` = closed.
type Frame = Option<Vec<u8>>;
type TunnelKey = (u64, u64); // connection generation, server-assigned id
const MAX_EARLY_CONNECTIONS: usize = 128;
const MAX_EARLY_BYTES: usize = 4 * 1024 * 1024;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const MAX_EARLY_FRAMES_PER_CONNECTION: usize = 128;
const REGISTERED_QUEUE_FRAMES: usize = 8;
const MAX_REGISTERED_CONNECTIONS: usize = 128;

#[derive(Default)]
struct EarlyFrames {
    frames: HashMap<TunnelKey, Vec<Frame>>,
    bytes: usize,
    generation: Option<u64>,
    failed: bool,
}

impl EarlyFrames {
    fn advance(&mut self, generation: u64) -> bool {
        if self.generation.is_some_and(|current| generation <= current) {
            return false;
        }
        *self = Self {
            generation: Some(generation),
            ..Self::default()
        };
        true
    }

    fn accepts(&self, generation: u64) -> bool {
        self.generation == Some(generation) && !self.failed
    }

    fn push(&mut self, key: TunnelKey, frame: Frame) -> Result<(), String> {
        let bytes = frame.as_ref().map_or(0, Vec::len);
        if !self.accepts(key.0) {
            return Err("tunnel generation closed after buffer budget exhaustion".into());
        }
        if bytes > MAX_FRAME_BYTES
            || self.bytes + bytes > MAX_EARLY_BYTES
            || (!self.frames.contains_key(&key) && self.frames.len() >= MAX_EARLY_CONNECTIONS)
            || self
                .frames
                .get(&key)
                .is_some_and(|frames| frames.len() >= MAX_EARLY_FRAMES_PER_CONNECTION)
        {
            self.fail(key.0);
            return Err("tunnel early-frame budget exhausted; connection generation closed".into());
        }
        self.bytes += bytes;
        self.frames.entry(key).or_default().push(frame);
        Ok(())
    }

    fn remove(&mut self, key: &TunnelKey) -> Vec<Frame> {
        let frames = self.frames.remove(key).unwrap_or_default();
        self.bytes -= frames.iter().flatten().map(Vec::len).sum::<usize>();
        frames
    }

    fn fail(&mut self, generation: u64) {
        if self.generation == Some(generation) {
            self.failed = true;
            self.frames.clear();
            self.bytes = 0;
        }
    }
}

#[derive(Clone)]
struct TunnelSink {
    tx: mpsc::Sender<Frame>,
    closed: watch::Sender<bool>,
}

fn tunnel_channel() -> (TunnelSink, mpsc::Receiver<Frame>, watch::Receiver<bool>) {
    let (tx, rx) = mpsc::channel(REGISTERED_QUEUE_FRAMES);
    let (closed, signal) = watch::channel(false);
    (TunnelSink { tx, closed }, rx, signal)
}

fn fail_tunnels(
    map: &mut HashMap<TunnelKey, TunnelSink>,
    pending: &mut EarlyFrames,
    generation: u64,
) {
    pending.fail(generation);
    map.retain(|key, sink| {
        if key.0 == generation {
            sink.closed.send_replace(true);
            false
        } else {
            true
        }
    });
}

/// Tunnel id -> sink for bytes arriving from the remote socket.
fn tunnels() -> &'static Mutex<HashMap<TunnelKey, TunnelSink>> {
    static T: OnceLock<Mutex<HashMap<TunnelKey, TunnelSink>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Route connection-bound `tcp.data` / `tcp.close`; returns true if consumed.
pub fn on_server_event_generation(v: &Value, generation: u64) -> bool {
    let event = v.get("event").and_then(|e| e.as_str()).unwrap_or_default();
    if event != "tcp.data" && event != "tcp.close" {
        return false;
    }
    let Some(id) = v.get("id").and_then(|i| i.as_u64()) else {
        return true;
    };
    let frame = if event == "tcp.data" {
        match v.get("base64").and_then(|b| b.as_str()).and_then(|b| {
            // Reject oversized encoded input before allocating decoded data.
            if b.len() > MAX_FRAME_BYTES.div_ceil(3) * 4 {
                Some(vec![0; MAX_FRAME_BYTES + 1])
            } else {
                STANDARD.decode(b).ok()
            }
        }) {
            Some(bytes) => Some(bytes),
            None => return true,
        }
    } else {
        None
    };
    let mut map = tunnels().lock().unwrap();
    let mut pending = early().lock().unwrap();
    if !pending.accepts(generation) {
        return true;
    }
    if frame
        .as_ref()
        .is_some_and(|bytes| bytes.len() > MAX_FRAME_BYTES)
    {
        fail_tunnels(&mut map, &mut pending, generation);
        tracing::warn!(
            generation,
            "tunnel frame exceeds byte budget; generation closed"
        );
        return true;
    }
    let key = (generation, id);
    match map.get(&key) {
        Some(sink) => {
            if let Err(error) = sink.tx.try_send(frame) {
                tracing::warn!(
                    generation,
                    "tunnel bounded queue failed: {error}; generation closed"
                );
                fail_tunnels(&mut map, &mut pending, generation);
            }
        }
        // The reply to tcp.connect and the first data can race the
        // registration in `pipe`; keep frames until it registers.
        None => {
            if let Err(error) = pending.push(key, frame) {
                tracing::warn!(generation, "{error}");
                fail_tunnels(&mut map, &mut pending, generation);
            }
        }
    }
    true
}

fn early() -> &'static Mutex<EarlyFrames> {
    static E: OnceLock<Mutex<EarlyFrames>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(EarlyFrames::default()))
}

fn register_tunnel(key: TunnelKey, sink: TunnelSink) -> Result<TunnelRegistration, String> {
    let mut map = tunnels().lock().unwrap();
    let mut pending = early().lock().unwrap();
    if !pending.accepts(key.0) {
        return Err("tunnel generation closed after early-frame budget exhaustion".into());
    }
    if !map.contains_key(&key) && map.len() >= MAX_REGISTERED_CONNECTIONS {
        fail_tunnels(&mut map, &mut pending, key.0);
        return Err("registered tunnel connection budget exhausted; generation closed".into());
    }
    for frame in pending.remove(&key) {
        if sink.tx.try_send(frame).is_err() {
            sink.closed.send_replace(true);
            fail_tunnels(&mut map, &mut pending, key.0);
            return Err(
                "early tunnel frames exceed registered queue capacity; generation closed".into(),
            );
        }
    }
    if let Some(previous) = map.insert(key, sink.clone()) {
        previous.closed.send_replace(true);
    }
    Ok(TunnelRegistration {
        key,
        sink,
        down: None,
    })
}

struct TunnelRegistration {
    key: TunnelKey,
    sink: TunnelSink,
    down: Option<JoinHandle<()>>,
}

impl Drop for TunnelRegistration {
    fn drop(&mut self) {
        let mut map = tunnels().lock().unwrap();
        if map
            .get(&self.key)
            .is_some_and(|sink| sink.tx.same_channel(&self.sink.tx))
        {
            map.remove(&self.key);
            early().lock().unwrap().remove(&self.key);
        }
        self.sink.closed.send_replace(true);
        if let Some(down) = &self.down {
            down.abort();
        }
    }
}

async fn bind_local(preferred: u16) -> Result<TcpListener, String> {
    for port in [preferred, 0] {
        if let Ok(l) = TcpListener::bind(("127.0.0.1", port)).await {
            return Ok(l);
        }
    }
    Err("no free local port".into())
}

async fn free_local_port(preferred: u16) -> Result<u16, String> {
    let l = bind_local(preferred).await?;
    l.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

fn cached_forward(
    map: &mut HashMap<u16, Entry>,
    authority: &str,
    remote_port: u16,
    generation: Option<u64>,
) -> Result<Option<Forward>, String> {
    let pending = early().lock().unwrap();
    if generation.is_some_and(|generation| !pending.accepts(generation)) {
        return Err(
            "tunnel connection generation unavailable; reconnect the remote broker before retrying"
                .into(),
        );
    }
    if let Some(entry) = map.get_mut(&remote_port) {
        if entry.reusable_for(authority) {
            return Ok(Some(entry.info.clone()));
        }
    }
    Ok(None)
}

pub async fn start(
    target: &RemoteTarget,
    remote_port: u16,
    local_port: Option<u16>,
) -> Result<Forward, String> {
    static START: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    let _start = START
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let authority = target.authority();
    let broker_generation = match target {
        RemoteTarget::Local => return Err("local workspace needs no forwarding".into()),
        RemoteTarget::Ssh { .. } => None,
        _ => Some(
            manager()
                .ok_or("remote broker unavailable")?
                .bind_authority(&authority)
                .await?,
        ),
    };
    let epoch;
    {
        let mut map = forwards().lock().unwrap();
        epoch = stop_epoch().load(std::sync::atomic::Ordering::Acquire);
        if let Some(existing) =
            cached_forward(&mut map, &authority, remote_port, broker_generation)?
        {
            return Ok(existing);
        }
        map.remove(&remote_port);
    }
    let preferred = local_port.unwrap_or(remote_port);
    let (info, handle) = match target {
        RemoteTarget::Ssh { host } => {
            let local = free_local_port(preferred).await?;
            let mut child = piscis_kernel::proc::tokio_command("ssh")
                .args([
                    "-N",
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "StrictHostKeyChecking=accept-new",
                    "-o",
                    "ConnectTimeout=15",
                    "-o",
                    "ExitOnForwardFailure=yes",
                    "-o",
                    "ServerAliveInterval=15",
                    "-L",
                    &format!("127.0.0.1:{local}:localhost:{remote_port}"),
                    host,
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| format!("ssh: {e}"))?;
            let tail = Arc::new(Mutex::new(DiagnosticTail::default()));
            let stderr = child.stderr.take().ok_or("SSH forward has no stderr")?;
            let mut handle = SshHandle {
                child,
                stderr: Some(tokio::spawn(drain_diagnostics(stderr, tail.clone()))),
            };
            if let Err(error) =
                wait_for_listener(&mut handle.child, local, Duration::from_secs(20)).await
            {
                // Give an already-closed stderr pipe one scheduling turn to drain.
                tokio::task::yield_now().await;
                return Err(format!(
                    "port forward failed: {error}; {}",
                    tail.lock().unwrap().message()
                ));
            }
            (
                Forward {
                    remote_port,
                    local_port: local,
                    via: "ssh",
                },
                Handle::Ssh(Box::new(handle)),
            )
        }
        RemoteTarget::Local => return Err("local workspace needs no forwarding".into()),
        _ => {
            let listener = bind_local(preferred).await?;
            let local = listener.local_addr().map_err(|e| e.to_string())?.port();
            let generation = broker_generation.ok_or("missing broker generation")?;
            let task = tokio::spawn(accept_loop(listener, remote_port, generation));
            (
                Forward {
                    remote_port,
                    local_port: local,
                    via: "tunnel",
                },
                Handle::Tunnel(task),
            )
        }
    };
    let mut map = forwards().lock().unwrap();
    if stop_epoch().load(std::sync::atomic::Ordering::Acquire) != epoch {
        drop(Entry {
            authority,
            info,
            _handle: handle,
        });
        return Err("port forward stopped while starting".into());
    }
    // A budget failure can occur without advancing stop_epoch during creation.
    if broker_generation.is_some_and(|generation| !early().lock().unwrap().accepts(generation)) {
        drop(Entry {
            authority,
            info,
            _handle: handle,
        });
        return Err(
            "tunnel connection generation failed while starting; reconnect the remote broker"
                .into(),
        );
    }
    map.insert(
        remote_port,
        Entry {
            authority,
            info: info.clone(),
            _handle: handle,
        },
    );
    Ok(info)
}

async fn accept_loop(listener: TcpListener, remote_port: u16, generation: u64) {
    // JoinSet aborts accepted connection tasks when the forward is stopped.
    let mut connections = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((sock, _)) = accepted else { break };
                if connections.len() >= MAX_REGISTERED_CONNECTIONS {
                    tracing::warn!(generation, "forward accepted-connection budget exhausted; socket closed");
                    drop(sock);
                    continue;
                }
                connections.spawn(async move {
                    if let Err(e) = pipe(sock, remote_port, generation).await {
                        tracing::debug!("tunnel to remote :{remote_port} ended: {e}");
                    }
                });
            }
            _ = connections.join_next(), if !connections.is_empty() => {}
        }
    }
}

async fn pipe(
    sock: tokio::net::TcpStream,
    remote_port: u16,
    generation: u64,
) -> Result<(), String> {
    let mgr = manager().ok_or("remote broker unavailable")?;
    let opened = mgr
        .request_generation(
            "tcp.connect",
            json!({ "host": "127.0.0.1", "port": remote_port }),
            generation,
        )
        .await?;
    let id = opened
        .get("id")
        .and_then(|i| i.as_u64())
        .ok_or("bad tcp.connect reply")?;
    let (sink, rx, signal) = tunnel_channel();
    let mut registration = register_tunnel((generation, id), sink)?;

    let (mut rd, wr) = sock.into_split();
    registration.down = Some(tokio::spawn(downstream(wr, rx, signal)));

    let mut buf = vec![0u8; 32 * 1024];
    loop {
        let read = tokio::select! {
            result = rd.read(&mut buf) => result,
            _ = registration.down.as_mut().expect("downstream task exists") => break,
        };
        match read {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                mgr.request_generation(
                    "tcp.write",
                    json!({ "id": id, "base64": STANDARD.encode(&buf[..n]) }),
                    generation,
                )
                .await?;
            }
        }
    }
    drop(registration);
    let _ = mgr
        .request_generation("tcp.close", json!({ "id": id }), generation)
        .await;
    Ok(())
}

async fn closed_signal(signal: &mut watch::Receiver<bool>) {
    loop {
        if *signal.borrow_and_update() {
            break;
        }
        if signal.changed().await.is_err() {
            break;
        }
    }
}

async fn downstream(
    mut writer: impl tokio::io::AsyncWrite + Unpin,
    mut rx: mpsc::Receiver<Frame>,
    mut signal: watch::Receiver<bool>,
) {
    loop {
        let frame = tokio::select! {
            biased;
            _ = closed_signal(&mut signal) => break,
            frame = rx.recv() => frame,
        };
        let Some(Some(bytes)) = frame else { break };
        tokio::select! {
            biased;
            _ = closed_signal(&mut signal) => break,
            result = writer.write_all(&bytes) => { if result.is_err() { break } },
        }
    }
    // Dropping the owned socket closes it without waiting for a blocked flush.
}

pub fn stop(remote_port: u16) {
    let mut map = forwards().lock().unwrap();
    stop_epoch().fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    map.remove(&remote_port);
}

pub fn stop_all() {
    let mut map = forwards().lock().unwrap();
    stop_epoch().fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    map.clear();
    let mut tunnels = tunnels().lock().unwrap();
    let mut pending = early().lock().unwrap();
    if let Some(generation) = pending.generation {
        fail_tunnels(&mut tunnels, &mut pending, generation);
    }
}

pub fn list() -> Vec<Forward> {
    let mut map = forwards().lock().unwrap();
    let pending = early().lock().unwrap();
    let broker_ready = pending
        .generation
        .is_some_and(|generation| pending.accepts(generation));
    map.retain(|_, entry| {
        (matches!(entry._handle, Handle::Ssh(_)) || broker_ready) && entry.is_alive()
    });
    let mut v: Vec<Forward> = map.values().map(|e| e.info.clone()).collect();
    v.sort_by_key(|f| f.remote_port);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned_sleeping_child() -> tokio::process::Child {
        #[cfg(windows)]
        let mut command = piscis_kernel::proc::tokio_command("powershell");
        #[cfg(windows)]
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 10",
        ]);
        #[cfg(not(windows))]
        let mut command = piscis_kernel::proc::tokio_command("sh");
        #[cfg(not(windows))]
        command.args(["-c", "exec sleep 10"]);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap()
    }

    #[tokio::test]
    async fn delayed_owned_listener_becomes_ready_before_deadline() {
        let port = free_local_port(0).await.unwrap();
        let listener = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
            listener.accept().await.unwrap();
        });
        let mut child = owned_sleeping_child();
        let result = wait_for_listener(&mut child, port, Duration::from_secs(3)).await;
        child.kill().await.unwrap();
        assert!(result.is_ok(), "{result:?}");
        tokio::time::timeout(Duration::from_secs(1), listener)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn unready_owned_listener_times_out_without_success() {
        let port = free_local_port(0).await.unwrap();
        let mut child = owned_sleeping_child();
        let result = wait_for_listener(&mut child, port, Duration::from_millis(60)).await;
        child.kill().await.unwrap();
        assert!(result.unwrap_err().contains("timed out"));
    }

    #[tokio::test]
    async fn diagnostic_drain_consumes_more_than_tail_capacity() {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let tail = Arc::new(Mutex::new(DiagnosticTail::default()));
        let drain = tokio::spawn(drain_diagnostics(reader, tail.clone()));
        let bytes = vec![b'x'; STDERR_TAIL_BYTES * 3];
        tokio::time::timeout(Duration::from_secs(2), writer.write_all(&bytes))
            .await
            .unwrap()
            .unwrap();
        writer.write_all(b"FINAL").await.unwrap();
        drop(writer);
        drain.await.unwrap();
        let tail = tail.lock().unwrap();
        assert_eq!(tail.0.len(), STDERR_TAIL_BYTES);
        assert!(tail.message().ends_with("FINAL"));
    }

    #[test]
    fn early_total_byte_budget_fails_closed_and_new_generation_recovers() {
        let mut frames = EarlyFrames::default();
        frames.advance(1);
        for id in 0..(MAX_EARLY_BYTES / MAX_FRAME_BYTES) as u64 {
            frames
                .push((1, id), Some(vec![0; MAX_FRAME_BYTES]))
                .unwrap();
        }
        assert_eq!(frames.bytes, MAX_EARLY_BYTES);
        assert!(frames.push((1, 1000), Some(vec![1])).is_err());
        assert_eq!(frames.bytes, 0);
        assert!(frames.failed);
        assert!(frames.push((1, 0), Some(vec![1])).is_err());
        frames.advance(2);
        frames.push((2, 0), Some(vec![1])).unwrap();
        assert_eq!(frames.bytes, 1);
    }

    #[test]
    fn early_connection_and_zero_byte_frame_budgets_are_bounded() {
        let mut frames = EarlyFrames::default();
        frames.advance(1);
        for id in 0..MAX_EARLY_CONNECTIONS as u64 {
            frames.push((1, id), None).unwrap();
        }
        assert!(frames
            .push((1, MAX_EARLY_CONNECTIONS as u64), None)
            .is_err());
        frames.advance(2);
        for _ in 0..MAX_EARLY_FRAMES_PER_CONNECTION {
            frames.push((2, 0), Some(Vec::new())).unwrap();
        }
        assert!(frames.push((2, 0), Some(Vec::new())).is_err());
        assert!(frames.frames.is_empty());
    }

    #[tokio::test]
    async fn oversized_frame_closes_registered_tunnels_and_rejects_further_registration() {
        let _serial = test_serial().lock().await;
        let key = test_key();
        begin_generation(key.0);
        let (tx, _rx, signal) = tunnel_channel();
        let guard = register_tunnel(key, tx).unwrap();
        on_server_event_generation(
            &json!({"event":"tcp.data", "id":key.1, "base64": STANDARD.encode(vec![0; MAX_FRAME_BYTES + 1])}),
            key.0,
        );
        assert!(*signal.borrow());
        let (next, _rx, _signal) = tunnel_channel();
        assert!(register_tunnel((key.0, key.1 + 1), next).is_err());
        drop(guard);
    }

    fn test_key() -> TunnelKey {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX / 2);
        (NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), 1)
    }

    fn test_serial() -> &'static tokio::sync::Mutex<()> {
        static SERIAL: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        SERIAL.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    #[tokio::test]
    async fn full_registered_queue_uses_independent_close_signal() {
        let _serial = test_serial().lock().await;
        let key = test_key();
        begin_generation(key.0);
        let (sink, _rx, signal) = tunnel_channel();
        let guard = register_tunnel(key, sink).unwrap();
        for _ in 0..=REGISTERED_QUEUE_FRAMES {
            on_server_event_generation(
                &json!({"event":"tcp.data", "id":key.1, "base64":"eA=="}),
                key.0,
            );
        }
        assert!(*signal.borrow(), "close must bypass the full queue");
        assert!(early().lock().unwrap().failed);
        assert!(tunnels().lock().unwrap().is_empty());
        drop(guard);
    }

    #[tokio::test]
    async fn close_signal_interrupts_a_blocked_local_write() {
        let (writer, _unread_peer) = tokio::io::duplex(1);
        let (sink, rx, signal) = tunnel_channel();
        sink.tx.try_send(Some(vec![0; MAX_FRAME_BYTES])).unwrap();
        let task = tokio::spawn(downstream(writer, rx, signal));
        tokio::task::yield_now().await;
        sink.closed.send_replace(true);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
    }

    #[test]
    fn repeated_generations_keep_failure_metadata_constant() {
        let mut state = EarlyFrames::default();
        for generation in 1..=10_000 {
            assert!(state.advance(generation));
            state.push((generation, 1), Some(vec![1])).unwrap();
            state.fail(generation);
            assert!(state.frames.is_empty());
            assert_eq!(state.bytes, 0);
        }
        assert_eq!(state.generation, Some(10_000));
        assert!(state.failed);
        assert!(!state.advance(9_999));
        assert!(!state.advance(10_000));
        assert!(!state.accepts(9_999));
    }

    #[tokio::test]
    async fn same_authority_restart_removes_old_listener_and_rejects_late_registration() {
        let _serial = test_serial().lock().await;
        let old = test_key();
        begin_generation(old.0);
        let task = tokio::spawn(std::future::pending::<()>());
        forwards().lock().unwrap().insert(
            60001,
            Entry {
                authority: "docker+same-authority".into(),
                info: Forward {
                    remote_port: 60001,
                    local_port: 60001,
                    via: "tunnel",
                },
                _handle: Handle::Tunnel(task),
            },
        );
        let new = test_key();
        begin_generation(new.0);
        assert!(!forwards().lock().unwrap().contains_key(&60001));
        let (late, _rx, _signal) = tunnel_channel();
        assert!(register_tunnel(old, late).is_err());
        on_server_event_generation(&json!({"event":"tcp.close", "id":old.1}), old.0);
        assert!(early().lock().unwrap().frames.is_empty());
        end_generation(old.0);
        assert!(early().lock().unwrap().accepts(new.0));
    }

    #[tokio::test]
    async fn registered_connection_budget_closes_generation() {
        let _serial = test_serial().lock().await;
        let key = test_key();
        begin_generation(key.0);
        let mut guards = Vec::new();
        let mut signals = Vec::new();
        for id in 0..MAX_REGISTERED_CONNECTIONS as u64 {
            let (sink, _rx, signal) = tunnel_channel();
            guards.push(register_tunnel((key.0, id), sink).unwrap());
            signals.push(signal);
        }
        let (sink, _rx, _signal) = tunnel_channel();
        assert!(register_tunnel((key.0, MAX_REGISTERED_CONNECTIONS as u64), sink).is_err());
        assert!(signals.iter().all(|signal| *signal.borrow()));
        assert!(tunnels().lock().unwrap().is_empty());
        drop(guards);
    }

    #[tokio::test]
    async fn replay_cannot_silently_overflow_registered_queue() {
        let _serial = test_serial().lock().await;
        let key = test_key();
        begin_generation(key.0);
        for _ in 0..=REGISTERED_QUEUE_FRAMES {
            on_server_event_generation(
                &json!({"event":"tcp.data", "id":key.1, "base64":"eA=="}),
                key.0,
            );
        }
        let (sink, _rx, signal) = tunnel_channel();
        assert!(register_tunnel(key, sink).is_err());
        assert!(*signal.borrow());
        assert!(early().lock().unwrap().failed);
    }

    #[tokio::test]
    async fn failed_broker_generation_is_not_reported_as_cached_success() {
        let _serial = test_serial().lock().await;
        let key = test_key();
        begin_generation(key.0);
        let mut map = HashMap::new();
        map.insert(
            60002,
            Entry {
                authority: "docker+fixture".into(),
                info: Forward {
                    remote_port: 60002,
                    local_port: 60002,
                    via: "tunnel",
                },
                _handle: Handle::Tunnel(tokio::spawn(std::future::pending::<()>())),
            },
        );
        assert!(
            cached_forward(&mut map, "docker+fixture", 60002, Some(key.0))
                .unwrap()
                .is_some()
        );
        early().lock().unwrap().fail(key.0);
        let error = cached_forward(&mut map, "docker+fixture", 60002, Some(key.0)).unwrap_err();
        assert!(error.contains("reconnect"));
    }

    #[tokio::test]
    async fn list_removes_failed_broker_listener() {
        let _serial = test_serial().lock().await;
        let key = test_key();
        begin_generation(key.0);
        forwards().lock().unwrap().insert(
            60003,
            Entry {
                authority: "docker+fixture".into(),
                info: Forward {
                    remote_port: 60003,
                    local_port: 60003,
                    via: "tunnel",
                },
                _handle: Handle::Tunnel(tokio::spawn(std::future::pending::<()>())),
            },
        );
        assert!(list().iter().any(|forward| forward.remote_port == 60003));
        early().lock().unwrap().fail(key.0);
        assert!(!list().iter().any(|forward| forward.remote_port == 60003));
        assert!(!forwards().lock().unwrap().contains_key(&60003));
    }

    #[tokio::test]
    async fn failed_pipe_scope_removes_registration_and_aborts_downstream() {
        let _serial = test_serial().lock().await;
        let key = test_key();
        begin_generation(key.0);
        let (tx, mut rx, _signal) = tunnel_channel();
        let mut registration = register_tunnel(key, tx).unwrap();
        let (done, ended) = tokio::sync::oneshot::channel::<()>();
        registration.down = Some(tokio::spawn(async move {
            let _held_until_cancel = done;
            std::future::pending::<()>().await;
        }));
        drop(registration); // the same guard unwinds on request error/cancellation
        assert!(!tunnels().lock().unwrap().contains_key(&key));
        assert!(rx.recv().await.is_none());
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(2), ended)
                .await
                .unwrap()
                .is_err()
        );
    }

    #[tokio::test]
    async fn old_registration_drop_preserves_replacement_channel() {
        let _serial = test_serial().lock().await;
        let key = test_key();
        begin_generation(key.0);
        let (old_tx, _old_rx, _old_signal) = tunnel_channel();
        let old = register_tunnel(key, old_tx).unwrap();
        let (new_tx, mut new_rx, _new_signal) = tunnel_channel();
        let new = register_tunnel(key, new_tx).unwrap();
        drop(old);
        assert!(on_server_event_generation(
            &json!({"event":"tcp.data", "id":key.1, "base64":"eA=="}),
            key.0
        ));
        assert_eq!(new_rx.try_recv().unwrap(), Some(b"x".to_vec()));
        drop(new);
    }

    #[tokio::test]
    async fn same_server_id_in_different_generations_is_isolated() {
        let _serial = test_serial().lock().await;
        let old_key = test_key();
        let new_key = (test_key().0, old_key.1);
        begin_generation(old_key.0);
        let (old_tx, _old_rx, old_signal) = tunnel_channel();
        let (new_tx, mut new_rx, new_signal) = tunnel_channel();
        let old = register_tunnel(old_key, old_tx).unwrap();
        begin_generation(new_key.0);
        let new = register_tunnel(new_key, new_tx).unwrap();
        on_server_event_generation(&json!({"event":"tcp.close", "id":old_key.1}), old_key.0);
        end_generation(old_key.0);
        assert!(*old_signal.borrow());
        assert!(!*new_signal.borrow());
        assert!(new_rx.try_recv().is_err());
        drop(old);
        drop(new);
    }

    #[tokio::test]
    async fn exited_owned_child_is_not_a_live_cached_forward() {
        #[cfg(windows)]
        let mut command = piscis_kernel::proc::tokio_command("cmd");
        #[cfg(windows)]
        command.args(["/d", "/c", "exit", "0"]);
        #[cfg(not(windows))]
        let mut command = piscis_kernel::proc::tokio_command("sh");
        #[cfg(not(windows))]
        command.args(["-c", "exit 0"]);
        let mut child = command.kill_on_drop(true).spawn().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap();
        let mut entry = Entry {
            authority: "ssh-remote+fixture".into(),
            info: Forward {
                remote_port: 1,
                local_port: 1,
                via: "ssh",
            },
            _handle: Handle::Ssh(Box::new(SshHandle {
                child,
                stderr: None,
            })),
        };
        assert!(!entry.is_alive());
    }

    #[tokio::test]
    async fn cached_forward_is_not_reused_for_another_authority() {
        let task = tokio::spawn(std::future::pending::<()>());
        let mut entry = Entry {
            authority: "docker+fixture-a".into(),
            info: Forward {
                remote_port: 1,
                local_port: 1,
                via: "tunnel",
            },
            _handle: Handle::Tunnel(task),
        };
        assert!(entry.reusable_for("docker+FIXTURE-A"));
        assert!(!entry.reusable_for("docker+fixture-b"));
    }
}
