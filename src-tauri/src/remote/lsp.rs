//! Built-in language servers for remote workspaces.
//!
//! The server runs on the remote host, spawned through the target's own
//! transport (`ssh` / `docker exec -i` / `wsl`) with stdio piped, and is exposed
//! to Monaco through a one-client WebSocket bridge — the same contract as the
//! local `LspManager`. Unlike the local bridge, `initialize` is forwarded to
//! the real server, and URIs are rewritten at the bridge:
//! `agentz-remote://<authority>/p` (client) ↔ `file:///p` (server).

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::Child;
use tokio_tungstenite::tungstenite::Message;

use super::{shell_quote, vfs, RemoteTarget};
use crate::lsp::manager::LspManager;

struct Session {
    port: u16,
    done: Arc<std::sync::atomic::AtomicBool>,
    _child: Child,
}

fn sessions() -> &'static StdMutex<HashMap<String, Session>> {
    static S: OnceLock<StdMutex<HashMap<String, Session>>> = OnceLock::new();
    S.get_or_init(|| StdMutex::new(HashMap::new()))
}

fn key(project_dir: &str, language: &str) -> String {
    format!("{project_dir}|{language}")
}

pub async fn start(target: RemoteTarget, project_dir: &str, language: &str) -> Result<u16, String> {
    let k = key(project_dir, language);
    {
        let mut map = sessions().lock().unwrap();
        if let Some(s) = map.get(&k) {
            if !s.done.load(std::sync::atomic::Ordering::SeqCst) {
                return Ok(s.port);
            }
        }
        map.remove(&k);
    }

    let root = vfs::parse(project_dir).ok_or("not a remote workspace")?.path;
    let lang = LspManager::supported_languages()
        .into_iter()
        .find(|l| l.language_id == language)
        .ok_or_else(|| format!("unsupported language: {language}"))?;
    let server = lang.server_command.clone();
    let script = server_script(&root, &server, &lang.server_args);

    let mut child = target
        .sh(&script)?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("{}: failed to spawn language server: {e}", target.authority()))?;

    // Fail fast when the binary is missing instead of handing Monaco a dead socket.
    let mut stderr = BufReader::new(child.stderr.take().ok_or("no stderr")?);
    let mut first = String::new();
    let probe = tokio::time::timeout(std::time::Duration::from_millis(1500), stderr.read_line(&mut first)).await;
    if first.contains("AGENTZ_LSP_MISSING") || matches!(probe, Ok(Ok(0))) && child.try_wait().ok().flatten().is_some() {
        return Err(format!(
            "language server '{server}' is not installed on {} — {}",
            target.authority(),
            LspManager::install_hint(language)
        ));
    }

    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done_task = done.clone();
    let authority = vfs::parse(project_dir).map(|u| u.authority).unwrap_or_default();
    tokio::spawn(async move {
        if let Err(e) = bridge(listener, stdin, stdout, stderr, first, authority).await {
            tracing::warn!("remote LSP bridge exited: {e}");
        }
        done_task.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    sessions().lock().unwrap().insert(k, Session { port, done, _child: child });
    Ok(port)
}

fn server_script(root: &str, server: &str, args: &[String]) -> String {
    let argv = std::iter::once(server.to_string())
        .chain(args.iter().cloned())
        .map(|a| shell_quote(&a))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "cd {root} || exit 1; {setup}; \
         command -v {srv} >/dev/null 2>&1 || {{ echo 'AGENTZ_LSP_MISSING' >&2; exit 127; }}; \
         exec {argv}",
        root = shell_quote(root),
        setup = super::TOOL_PATH_SETUP,
        srv = shell_quote(server),
    )
}

pub fn stop(project_dir: &str, language: &str) -> bool {
    sessions().lock().unwrap().remove(&key(project_dir, language)).is_some()
}

pub fn stop_all() {
    sessions().lock().unwrap().clear();
}

async fn bridge(
    listener: TcpListener,
    mut stdin: tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
    mut stderr: BufReader<tokio::process::ChildStderr>,
    first_stderr: String,
    authority: String,
) -> Result<(), String> {
    let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
    let ws = tokio_tungstenite::accept_async(stream).await.map_err(|e| e.to_string())?;
    let (mut tx, mut rx) = ws.split();

    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<String>(256);
    let reader = tokio::spawn(read_frames(stdout, out_tx.clone()));
    let log = tokio::spawn(async move {
        let mut line = first_stderr;
        loop {
            let text = line.trim_end();
            if !text.is_empty() {
                let msg = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "window/logMessage",
                    "params": { "type": 4, "message": text },
                });
                if out_tx.send(msg.to_string()).await.is_err() {
                    break;
                }
            }
            line.clear();
            match stderr.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });

    // Raw authority as the client encodes it (Monaco lowercases/escapes it);
    // learned from the first client URI so server URIs map back exactly.
    let mut client_authority: Option<String> = None;
    let result = loop {
        tokio::select! {
            msg = rx.next() => match msg {
                Some(Ok(Message::Text(text))) => {
                    let Ok(mut v) = serde_json::from_str::<Value>(&text) else { continue };
                    rewrite(&mut v, &mut |s| to_server(s, &mut client_authority));
                    let body = v.to_string();
                    let frame = format!("Content-Length: {}\r\n\r\n{body}", body.len());
                    if stdin.write_all(frame.as_bytes()).await.is_err() || stdin.flush().await.is_err() {
                        break Err("language server stdin closed".to_string());
                    }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break Ok(()),
                _ => {}
            },
            body = out_rx.recv() => match body {
                Some(body) => {
                    let text = match serde_json::from_str::<Value>(&body) {
                        Ok(mut v) => {
                            let auth = client_authority.clone().unwrap_or_else(|| encode_authority(&authority));
                            rewrite(&mut v, &mut |s| to_client(s, &auth));
                            v.to_string()
                        }
                        Err(_) => body,
                    };
                    if tx.send(Message::Text(text)).await.is_err() {
                        break Ok(());
                    }
                }
                None => break Ok(()),
            },
        }
    };
    reader.abort();
    log.abort();
    result
}

/// Content-Length framed LSP messages → JSON bodies.
async fn read_frames(stdout: tokio::process::ChildStdout, tx: tokio::sync::mpsc::Sender<String>) {
    let mut r = BufReader::new(stdout);
    loop {
        let mut len: Option<usize> = None;
        loop {
            let mut line = String::new();
            match r.read_line(&mut line).await {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                len = v.trim().parse().ok();
            }
        }
        let Some(len) = len else { continue };
        let mut buf = vec![0u8; len];
        if r.read_exact(&mut buf).await.is_err() {
            return;
        }
        if tx.send(String::from_utf8_lossy(&buf).into_owned()).await.is_err() {
            return;
        }
    }
}

const CLIENT_PREFIX: &str = "agentz-remote://";

fn to_server(s: &str, learned: &mut Option<String>) -> Option<String> {
    let rest = s.strip_prefix(CLIENT_PREFIX)?;
    let (auth, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    if learned.is_none() {
        *learned = Some(auth.to_string());
    }
    Some(format!("file://{}", if path.is_empty() { "/" } else { path }))
}

fn to_client(s: &str, authority: &str) -> Option<String> {
    let path = s.strip_prefix("file://")?;
    path.starts_with('/').then(|| format!("{CLIENT_PREFIX}{authority}{path}"))
}

/// Matches Monaco's `URI.toString()` authority form.
fn encode_authority(a: &str) -> String {
    let mut out = String::new();
    for b in a.to_lowercase().bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Rewrites every string value and object key (WorkspaceEdit `changes` is keyed by URI).
fn rewrite(v: &mut Value, f: &mut dyn FnMut(&str) -> Option<String>) {
    match v {
        Value::String(s) => {
            if let Some(n) = f(s) {
                *s = n;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| rewrite(x, f)),
        Value::Object(m) => {
            let old = std::mem::take(m);
            for (k, mut val) in old {
                rewrite(&mut val, f);
                m.insert(f(&k).unwrap_or(k), val);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_round_trip() {
        let mut learned = None;
        let s = to_server("agentz-remote://ssh-remote%2Bbox/home/me/a.rs", &mut learned).unwrap();
        assert_eq!(s, "file:///home/me/a.rs");
        assert_eq!(learned.as_deref(), Some("ssh-remote%2Bbox"));
        assert_eq!(
            to_client("file:///home/me/b.rs", learned.as_deref().unwrap()).unwrap(),
            "agentz-remote://ssh-remote%2Bbox/home/me/b.rs"
        );
        assert_eq!(encode_authority("ssh-remote+Box"), "ssh-remote%2Bbox");
    }

    #[test]
    fn rewrites_keys() {
        let mut v = serde_json::json!({ "changes": { "file:///a": [1] }, "uri": "file:///b" });
        rewrite(&mut v, &mut |s| to_client(s, "wsl%2Bu"));
        assert_eq!(v["uri"], "agentz-remote://wsl%2Bu/b");
        assert!(v["changes"].get("agentz-remote://wsl%2Bu/a").is_some());
    }
}
