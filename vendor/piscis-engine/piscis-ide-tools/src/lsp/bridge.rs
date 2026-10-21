//! WebSocket ↔ LSP stdio bridge.
//!
//! Spawned per LSP session. Accepts one WebSocket connection on a bound TCP
//! listener and bidirectionally relays JSON-RPC messages between the WS client
//! (Monaco Editor) and the LSP server process (stdin/stdout) using standard
//! `Content-Length` framing.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite;
use tracing::{info, warn};

/// Run the LSP ↔ WebSocket bridge by binding `127.0.0.1:{port}`.
pub async fn run_lsp_bridge(
    port: u16,
    lsp_stdin: ChildStdin,
    lsp_stdout: ChildStdout,
    lsp_stderr: Option<ChildStderr>,
    language: &str,
    project_dir: &str,
    bridge_app_name: &str,
) -> Result<(), String> {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("LSP bridge: bind port {} failed: {}", port, e))?;
    run_lsp_bridge_with_listener(
        listener,
        lsp_stdin,
        lsp_stdout,
        lsp_stderr,
        language,
        project_dir,
        bridge_app_name,
    )
    .await
}

/// Run the LSP ↔ WebSocket bridge on an already-bound [`TcpListener`].
///
/// Takes ownership of the LSP process I/O handles:
/// - Accepts one WebSocket connection on `listener`
/// - Relays messages bidirectionally until either side disconnects or the
///   language server's stdout closes
/// - Uses proper `Content-Length` framing on both stdin and stdout so the real
///   language server receives `initialize` and all subsequent requests intact.
pub async fn run_lsp_bridge_with_listener(
    listener: TcpListener,
    lsp_stdin: ChildStdin,
    lsp_stdout: ChildStdout,
    lsp_stderr: Option<ChildStderr>,
    language: &str,
    project_dir: &str,
    _bridge_app_name: &str,
) -> Result<(), String> {
    let addr = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "127.0.0.1:?".to_string());

    info!(
        "LSP bridge listening on ws://{} for language={} project={}",
        addr, language, project_dir
    );

    // Wrap stdin for shared async access
    let stdin = Arc::new(Mutex::new(lsp_stdin));

    // ── stdout reader task (Content-Length framed) ──────────────────
    let (stdout_tx, mut stdout_rx) = tokio::sync::mpsc::channel::<String>(256);
    let stderr_out_tx = stdout_tx.clone();
    let mut stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(lsp_stdout);
        loop {
            let mut content_len: Option<usize> = None;
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        info!("LSP stdout: EOF");
                        return;
                    }
                    Ok(_) => {
                        let trimmed = line.trim_end();
                        if trimmed.is_empty() {
                            if content_len.is_some() {
                                break;
                            }
                            continue;
                        }
                        if let Some(v) =
                            trimmed.to_ascii_lowercase().strip_prefix("content-length:")
                        {
                            content_len = v.trim().parse().ok();
                        }
                    }
                    Err(e) => {
                        warn!("LSP stdout read header error: {}", e);
                        return;
                    }
                }
            }
            let Some(len) = content_len else { continue };
            let mut buf = vec![0u8; len];
            if let Err(e) = reader.read_exact(&mut buf).await {
                warn!("LSP stdout read body error: {}", e);
                return;
            }
            let body = String::from_utf8_lossy(&buf).into_owned();
            if stdout_tx.send(body).await.is_err() {
                return;
            }
        }
    });

    // ── stderr reader task ──────────────────────────────────────────
    let stderr_task = lsp_stderr.map(|stderr| {
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end();
                        if !trimmed.is_empty() {
                            let log_msg = serde_json::json!({
                                "jsonrpc": "2.0",
                                "method": "window/logMessage",
                                "params": { "type": 4, "message": trimmed }
                            })
                            .to_string();
                            if stderr_out_tx.send(log_msg).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        })
    });

    // Accept one connection (or exit early if the language server dies first)
    let accept = async {
        let (stream, peer) = listener
            .accept()
            .await
            .map_err(|e| format!("LSP bridge: accept failed: {}", e))?;
        info!(
            "LSP bridge: client connected from {} (lang={})",
            peer, language
        );
        tokio_tungstenite::accept_async(stream)
            .await
            .map_err(|e| format!("LSP bridge: WS handshake failed: {}", e))
    };

    let ws_stream = tokio::select! {
        res = accept => match res {
            Ok(ws) => ws,
            Err(e) => {
                stdout_task.abort();
                if let Some(t) = stderr_task {
                    t.abort();
                }
                return Err(e);
            }
        },
        _ = &mut stdout_task => {
            if let Some(t) = stderr_task {
                t.abort();
            }
            return Err("LSP process exited before WebSocket client connected".into());
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // ── main relay loop ─────────────────────────────────────────────
    use futures::{SinkExt, StreamExt};

    let mut first_message = true;
    let result: Result<(), String> = loop {
        tokio::select! {
            _ = &mut stdout_task => {
                info!("LSP bridge: stdout reader finished");
                break Ok(());
            }

            // WS client → LSP stdin
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(tungstenite::Message::Text(text))) => {
                        if first_message {
                            first_message = false;
                            info!("LSP bridge: forwarding first WS message (len={})", text.len());
                        }

                        // Forward to LSP stdin (LSP uses Content-Length framing)
                        let content_len = text.len();
                        let header = format!("Content-Length: {}\r\n\r\n", content_len);
                        {
                            let mut stdin_guard = stdin.lock().await;
                            if stdin_guard.write_all(header.as_bytes()).await.is_err() {
                                break Err("LSP stdin write (header) failed".into());
                            }
                            if stdin_guard.write_all(text.as_bytes()).await.is_err() {
                                break Err("LSP stdin write (body) failed".into());
                            }
                            if stdin_guard.flush().await.is_err() {
                                break Err("LSP stdin flush failed".into());
                            }
                        }
                    }
                    Some(Ok(tungstenite::Message::Close(_))) => {
                        info!("LSP bridge: WS client closed");
                        break Ok(());
                    }
                    Some(Err(e)) => {
                        warn!("LSP bridge: WS recv error: {}", e);
                        break Ok(());
                    }
                    None => break Ok(()),
                    _ => {} // Ignore binary/ping/pong
                }
            }

            // LSP stdout / stderr → WS client
            msg = stdout_rx.recv() => {
                match msg {
                    Some(text) => {
                        let body = strip_lsp_header(&text);
                        if ws_sender.send(tungstenite::Message::Text(body.to_string())).await.is_err() {
                            break Err("WS send error".into());
                        }
                    }
                    None => {
                        info!("LSP bridge: stdout channel closed");
                        break Ok(());
                    }
                }
            }
        }
    };

    // Cleanup
    stdout_task.abort();
    if let Some(t) = stderr_task {
        t.abort();
    }

    info!("LSP bridge for {} shut down", language);
    result
}

/// Strip Content-Length header from an LSP message, returning just the JSON body.
fn strip_lsp_header(raw: &str) -> &str {
    if let Some(idx) = raw.find("\r\n\r\n") {
        let body = &raw[idx + 4..];
        if body.starts_with('{') {
            return body;
        }
    }
    if let Some(idx) = raw.find("\n\n") {
        let body = &raw[idx + 2..];
        if body.starts_with('{') {
            return body;
        }
    }
    raw
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use std::process::Stdio;
    use tokio_tungstenite::tungstenite::Message;

    #[tokio::test]
    async fn bridge_forwards_initialize_and_frames_multibyte_messages() {
        #[cfg(windows)]
        let mut cmd = {
            let mut c = piscis_kernel::proc::tokio_command("node");
            c.arg("-e");
            c
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = piscis_kernel::proc::tokio_command("node");
            c.arg("-e");
            c
        };
        let script = r#"
          let buf = Buffer.alloc(0);
          function send(obj) {
            const body = Buffer.from(JSON.stringify(obj), 'utf8');
            process.stdout.write(`Content-Length: ${body.length}\r\n\r\n`);
            process.stdout.write(body);
          }
          process.stdin.on('data', (chunk) => {
            buf = Buffer.concat([buf, chunk]);
            while (true) {
              const sep = buf.indexOf('\r\n\r\n');
              if (sep === -1) break;
              const header = buf.slice(0, sep).toString('utf8');
              const m = /content-length:\s*(\d+)/i.exec(header);
              if (!m) break;
              const len = parseInt(m[1], 10);
              if (buf.length < sep + 4 + len) break;
              const body = buf.slice(sep + 4, sep + 4 + len).toString('utf8');
              buf = buf.slice(sep + 4 + len);
              const msg = JSON.parse(body);
              if (msg.method === 'initialize') {
                send({ jsonrpc: '2.0', id: msg.id, result: { capabilities: { hoverProvider: true } } });
              } else if (msg.method === 'textDocument/didOpen') {
                send({
                  jsonrpc: '2.0',
                  method: 'textDocument/publishDiagnostics',
                  params: { uri: msg.params.textDocument.uri, diagnostics: [{ message: '中文诊断' }] }
                });
              }
            }
          });
        "#;
        let mut child = cmd
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take();

        let bridge_task = tokio::spawn(async move {
            run_lsp_bridge_with_listener(
                listener,
                stdin,
                stdout,
                stderr,
                "typescript",
                ".",
                "AgentZ",
            )
            .await
        });

        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .unwrap();

        ws.send(Message::Text(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "rootUri": "file:///proj", "capabilities": {} }
            })
            .to_string(),
        ))
        .await
        .unwrap();

        let init_reply = ws.next().await.unwrap().unwrap().into_text().unwrap();
        let init_json: Value = serde_json::from_str(&init_reply).unwrap();
        assert_eq!(init_json["id"], 1);
        assert_eq!(init_json["result"]["capabilities"]["hoverProvider"], true);

        ws.send(Message::Text(
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///proj/a.ts",
                        "languageId": "typescript",
                        "version": 1,
                        "text": "const x = 1;"
                    }
                }
            })
            .to_string(),
        ))
        .await
        .unwrap();

        let diag_reply = ws.next().await.unwrap().unwrap().into_text().unwrap();
        let diag_json: Value = serde_json::from_str(&diag_reply).unwrap();
        assert_eq!(diag_json["method"], "textDocument/publishDiagnostics");
        assert_eq!(diag_json["params"]["diagnostics"][0]["message"], "中文诊断");

        let _ = ws.close(None).await;
        let _ = bridge_task.await;
    }
}
