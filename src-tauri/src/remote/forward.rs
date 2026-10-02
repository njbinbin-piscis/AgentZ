//! Port forwarding from the remote workspace to `127.0.0.1` on this machine.
//!
//! SSH targets use the client's native `-L` forwarding. Docker / WSL targets
//! have no SSH, so each accepted local connection is tunnelled through the
//! agentz-server stdio (`tcp.connect` / `tcp.write` / `tcp.data` events).

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::{manager, RemoteTarget};

#[derive(Debug, Clone, Serialize)]
pub struct Forward {
    pub remote_port: u16,
    pub local_port: u16,
    pub via: &'static str,
}

enum Handle {
    /// Held only for `kill_on_drop`.
    Ssh(#[allow(dead_code)] Box<tokio::process::Child>),
    Tunnel(JoinHandle<()>),
}

struct Entry {
    info: Forward,
    _handle: Handle,
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

/// A chunk from the remote socket; `None` = closed.
type Frame = Option<Vec<u8>>;

/// Tunnel id -> sink for bytes arriving from the remote socket.
fn tunnels() -> &'static Mutex<HashMap<u64, mpsc::UnboundedSender<Frame>>> {
    static T: OnceLock<Mutex<HashMap<u64, mpsc::UnboundedSender<Frame>>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Route `tcp.data` / `tcp.close` server events; returns true if consumed.
pub fn on_server_event(v: &Value) -> bool {
    let event = v.get("event").and_then(|e| e.as_str()).unwrap_or_default();
    if event != "tcp.data" && event != "tcp.close" {
        return false;
    }
    let Some(id) = v.get("id").and_then(|i| i.as_u64()) else { return true };
    let frame = if event == "tcp.data" {
        match v.get("base64").and_then(|b| b.as_str()).and_then(|b| STANDARD.decode(b).ok()) {
            Some(bytes) => Some(bytes),
            None => return true,
        }
    } else {
        None
    };
    let map = tunnels().lock().unwrap();
    match map.get(&id) {
        Some(tx) => {
            let _ = tx.send(frame);
        }
        // The reply to tcp.connect and the first data can race the
        // registration in `pipe`; keep frames until it registers.
        None => early().lock().unwrap().entry(id).or_default().push(frame),
    }
    true
}

fn early() -> &'static Mutex<HashMap<u64, Vec<Frame>>> {
    static E: OnceLock<Mutex<HashMap<u64, Vec<Frame>>>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_tunnel(id: u64, tx: mpsc::UnboundedSender<Option<Vec<u8>>>) {
    let mut map = tunnels().lock().unwrap();
    for frame in early().lock().unwrap().remove(&id).unwrap_or_default() {
        let _ = tx.send(frame);
    }
    map.insert(id, tx);
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

pub async fn start(target: &RemoteTarget, remote_port: u16, local_port: Option<u16>) -> Result<Forward, String> {
    if let Some(existing) = forwards().lock().unwrap().get(&remote_port) {
        return Ok(existing.info.clone());
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
            // ExitOnForwardFailure makes setup errors surface within a moment.
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            if let Ok(Some(_)) = child.try_wait() {
                let mut err = String::new();
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut err).await;
                }
                return Err(format!("port forward failed: {}", err.trim()));
            }
            (Forward { remote_port, local_port: local, via: "ssh" }, Handle::Ssh(Box::new(child)))
        }
        RemoteTarget::Local => return Err("local workspace needs no forwarding".into()),
        _ => {
            let listener = bind_local(preferred).await?;
            let local = listener.local_addr().map_err(|e| e.to_string())?.port();
            let task = tokio::spawn(accept_loop(listener, remote_port));
            (Forward { remote_port, local_port: local, via: "tunnel" }, Handle::Tunnel(task))
        }
    };
    forwards()
        .lock()
        .unwrap()
        .insert(remote_port, Entry { info: info.clone(), _handle: handle });
    Ok(info)
}

async fn accept_loop(listener: TcpListener, remote_port: u16) {
    while let Ok((sock, _)) = listener.accept().await {
        tokio::spawn(async move {
            if let Err(e) = pipe(sock, remote_port).await {
                tracing::debug!("tunnel to remote :{remote_port} ended: {e}");
            }
        });
    }
}

async fn pipe(sock: tokio::net::TcpStream, remote_port: u16) -> Result<(), String> {
    let mgr = manager().ok_or("remote broker unavailable")?;
    let opened = mgr
        .request("tcp.connect", json!({ "host": "127.0.0.1", "port": remote_port }))
        .await?;
    let id = opened.get("id").and_then(|i| i.as_u64()).ok_or("bad tcp.connect reply")?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    register_tunnel(id, tx);

    let (mut rd, mut wr) = sock.into_split();
    let down = tokio::spawn(async move {
        while let Some(Some(bytes)) = rx.recv().await {
            if wr.write_all(&bytes).await.is_err() {
                break;
            }
        }
        let _ = wr.shutdown().await;
    });

    let mut buf = vec![0u8; 32 * 1024];
    loop {
        match rd.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                mgr.request("tcp.write", json!({ "id": id, "base64": STANDARD.encode(&buf[..n]) }))
                    .await?;
            }
        }
    }
    let _ = mgr.request("tcp.close", json!({ "id": id })).await;
    tunnels().lock().unwrap().remove(&id);
    down.abort();
    Ok(())
}

pub fn stop(remote_port: u16) {
    forwards().lock().unwrap().remove(&remote_port);
}

pub fn stop_all() {
    forwards().lock().unwrap().clear();
}

pub fn list() -> Vec<Forward> {
    let mut v: Vec<Forward> = forwards().lock().unwrap().values().map(|e| e.info.clone()).collect();
    v.sort_by_key(|f| f.remote_port);
    v
}
