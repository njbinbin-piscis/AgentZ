//! Live end-to-end checks against a real host. Ignored by default:
//! `AGENTZ_LIVE_HOST=user@host cargo test --lib remote::live_tests -- --ignored --nocapture --test-threads=1`
//! (key auth must already work; see `ssh_setup`).

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::*;

fn target() -> RemoteTarget {
    RemoteTarget::Ssh { host: std::env::var("AGENTZ_LIVE_HOST").expect("AGENTZ_LIVE_HOST") }
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

struct Server {
    _child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    lines: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    next: u64,
    events: Vec<Value>,
}

impl Server {
    async fn req(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        let line = json!({ "$agentz": "req", "id": id, "method": method, "params": params }).to_string() + "\n";
        self.stdin.write_all(line.as_bytes()).await.unwrap();
        loop {
            let l = tokio::time::timeout(Duration::from_secs(60), self.lines.next_line())
                .await
                .map_err(|_| format!("{method}: timeout"))?
                .unwrap()
                .ok_or("server closed")?;
            let Ok(v) = serde_json::from_str::<Value>(&l) else { continue };
            match v.get("$agentz").and_then(|k| k.as_str()) {
                Some("res") if v["id"] == id => {
                    return match v.get("error") {
                        Some(e) if !e.is_null() => Err(e.to_string()),
                        _ => Ok(v["result"].clone()),
                    }
                }
                Some("event") => self.events.push(v),
                _ => {}
            }
        }
    }

    async fn wait_event(&mut self, name: &str, secs: u64) -> Option<Value> {
        if let Some(i) = self.events.iter().position(|e| e["event"] == name) {
            return Some(self.events.remove(i));
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
        while let Ok(Ok(Some(l))) = tokio::time::timeout_at(deadline, self.lines.next_line()).await {
            if let Ok(v) = serde_json::from_str::<Value>(&l) {
                if v["$agentz"] == "event" && v["event"] == name {
                    return Some(v);
                }
            }
        }
        None
    }
}

async fn start_server(t: &RemoteTarget) -> (DeployedServer, Server) {
    let host_js = repo().join("extension-host/dist/host.js");
    let node = repo().join("src-tauri/resources/node");
    let started = std::time::Instant::now();
    let d = deploy(t, &host_js, Some(&node)).await.expect("deploy");
    println!("deploy: {d:?} in {:?}", started.elapsed());
    let mut child = t
        .sh(&launch_script(&d, &d.home))
        .unwrap()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let lines = BufReader::new(child.stdout.take().unwrap()).lines();
    (d, Server { _child: child, stdin, lines, next: 0, events: vec![] })
}

fn b64(s: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(s)
}

fn unb64(v: &Value) -> String {
    use base64::Engine as _;
    String::from_utf8(base64::engine::general_purpose::STANDARD.decode(v["base64"].as_str().unwrap()).unwrap()).unwrap()
}

#[tokio::test]
#[ignore]
async fn live_server_protocol() {
    let t = target();
    println!("run: {}", t.run("echo ok").await.unwrap().trim());
    let (d, mut s) = start_server(&t).await;
    let root = format!("{}/agentz-e2e", d.home);
    let _ = t.run(&format!("rm -rf {}", shell_quote(&root))).await;

    let info = s.req("info", json!({})).await.unwrap();
    println!("info: {info}");

    // fs round trip
    s.req("fs.mkdir", json!({ "path": format!("{root}/src") })).await.unwrap();
    s.req("fs.writeFile", json!({ "path": format!("{root}/src/main.py"), "base64": b64("def hello():\n    return 'needle'\n") }))
        .await
        .unwrap();
    s.req("fs.writeFile", json!({ "path": format!("{root}/README.md"), "text": "# e2e 中文\n" })).await.unwrap();
    let r = s.req("fs.readFile", json!({ "path": format!("{root}/README.md") })).await.unwrap();
    assert_eq!(unb64(&r), "# e2e 中文\n");
    let st = s.req("fs.stat", json!({ "path": format!("{root}/src/main.py") })).await.unwrap();
    assert_eq!(st["isFile"], true);
    let dir = s.req("fs.readDir", json!({ "path": &root })).await.unwrap();
    println!("readDir: {dir}");
    s.req("fs.rename", json!({ "from": format!("{root}/README.md"), "to": format!("{root}/README2.md") })).await.unwrap();
    assert!(s.req("fs.stat", json!({ "path": format!("{root}/README.md") })).await.is_err());

    let tree = s.req("fs.tree", json!({ "path": &root })).await.unwrap();
    println!("tree: {tree}");

    let hits = s.req("search", json!({ "root": &root, "query": "needle" })).await.unwrap();
    println!("search: {hits}");
    assert!(hits.to_string().contains("main.py"));

    // git via exec + nested discovery
    let g = s
        .req("exec", json!({ "command": "sh", "args": ["-c", "git init -q sub && cd sub && git -c user.email=a@b -c user.name=e2e commit -q --allow-empty -m init && git log --oneline | wc -l"], "cwd": &root }))
        .await
        .unwrap();
    println!("exec git: {g}");
    let repos = s.req("git.discover", json!({ "path": &root })).await.unwrap();
    println!("git.discover: {repos}");
    assert!(repos.to_string().contains("sub"));

    // watcher
    s.req("fs.watch", json!({ "path": &root, "token": "w1" })).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    t.run(&format!("echo x >> {}/src/main.py", shell_quote(&root))).await.unwrap();
    let ev = s.wait_event("fs.change", 10).await;
    println!("fs.change: {ev:?}");
    assert!(ev.is_some());
    s.req("fs.unwatch", json!({ "token": "w1" })).await.unwrap();

    // ports + tcp tunnel (sshd banner on 22)
    let ports = s.req("ports.list", json!({})).await.unwrap();
    println!("ports.list: {ports}");
    let c = s.req("tcp.connect", json!({ "host": "127.0.0.1", "port": 22 })).await.unwrap();
    let data = s.wait_event("tcp.data", 10).await.expect("tcp.data");
    println!("tcp.data: {}", unb64(&data).trim());
    assert!(unb64(&data).starts_with("SSH-"));
    s.req("tcp.close", json!({ "id": c["id"] })).await.unwrap();

    // streaming proc
    let p = s.req("proc.spawn", json!({ "command": "cat", "args": [] })).await.unwrap();
    s.req("proc.write", json!({ "pid": p["pid"], "data": "ping\n" })).await.unwrap();
    let out = s.wait_event("proc.data", 10).await.expect("proc.data");
    println!("proc.data: {}", out["data"]);
    s.req("proc.kill", json!({ "pid": p["pid"] })).await.unwrap();

    s.req("fs.delete", json!({ "path": &root, "recursive": true })).await.unwrap();
}

const DEV: &str = "agentz-e2e-dev";

#[tokio::test]
#[ignore]
async fn live_remote_lsp() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let t = target();
    let host = std::env::var("AGENTZ_LIVE_HOST").unwrap();
    let home = t.run("echo $HOME").await.unwrap().trim().to_string();
    let project = format!("agentz-remote://ssh-remote+{host}/{}/{DEV}", home.trim_start_matches('/'));
    // What Monaco serializes for that project (authority escaped).
    let client_root = format!("agentz-remote://ssh-remote%2B{host}{home}/{DEV}");

    let port = lsp::start(t.clone(), &project, "python").await.expect("lsp start");
    let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}")).await.unwrap();
    let (mut tx, mut rx) = ws.split();
    let send = |v: Value| Message::Text(v.to_string());
    type Rx = futures::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>;
    async fn recv(rx: &mut Rx, pred: impl Fn(&Value) -> bool) -> Option<Value> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
        while let Ok(Some(Ok(Message::Text(t)))) = tokio::time::timeout_at(deadline, rx.next()).await {
            let v: Value = serde_json::from_str(&t).unwrap();
            if pred(&v) {
                return Some(v);
            }
        }
        None
    }
    macro_rules! recv_until {
        ($p:expr) => {
            recv(&mut rx, $p)
        };
    }

    tx.send(send(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "processId": null, "rootUri": client_root, "capabilities": {},
        "workspaceFolders": [{ "uri": client_root, "name": "e2e" }] } })))
        .await
        .unwrap();
    let init = recv_until!(|v| v["id"] == 1).await.expect("initialize response");
    println!("server: {}", init["result"]["serverInfo"]);
    assert!(init["result"]["capabilities"]["definitionProvider"].as_bool().unwrap_or(true));
    tx.send(send(json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }))).await.unwrap();

    let main_uri = format!("{client_root}/main.py");
    let text = t.run(&format!("cat {home}/{DEV}/main.py")).await.unwrap();
    tx.send(send(json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
        "textDocument": { "uri": main_uri, "languageId": "python", "version": 1, "text": text } } })))
        .await
        .unwrap();
    tx.send(send(json!({ "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition", "params": {
        "textDocument": { "uri": main_uri }, "position": { "line": 2, "character": 9 } } })))
        .await
        .unwrap();
    let def = recv_until!(|v| v["id"] == 2).await.expect("definition response");
    println!("definition: {}", def["result"]);
    assert!(def["result"].to_string().contains(&format!("{client_root}/util.py")));

    tx.send(send(json!({ "jsonrpc": "2.0", "id": 3, "method": "textDocument/hover", "params": {
        "textDocument": { "uri": main_uri }, "position": { "line": 2, "character": 9 } } })))
        .await
        .unwrap();
    let hover = recv_until!(|v| v["id"] == 3).await.expect("hover response");
    println!("hover: {}", hover["result"]["contents"]);
    assert!(hover["result"].to_string().contains("add"));
    lsp::stop(&project, "python");
}

#[tokio::test]
#[ignore]
async fn live_remote_debugpy() {
    use tokio::io::AsyncReadExt;
    let t = target();
    let home = t.run("echo $HOME").await.unwrap().trim().to_string();
    let dir = format!("{home}/{DEV}");
    let script = crate::commands::dap::remote_adapter_script(&dir, "python", &["-m".into(), "debugpy.adapter".into()]);
    let mut child = t.sh(&script).unwrap().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut seq = 0u64;
    let mut buf: Vec<u8> = Vec::new();

    macro_rules! send {
        ($cmd:expr, $args:expr) => {{
            seq += 1;
            let body = json!({ "seq": seq, "type": "request", "command": $cmd, "arguments": $args }).to_string();
            stdin.write_all(format!("Content-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            seq
        }};
    }
    async fn next_msg(stdout: &mut tokio::process::ChildStdout, buf: &mut Vec<u8>) -> Option<Value> {
        loop {
            if let Some(h) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&buf[..h]).to_string();
                let len: usize = header.lines().find_map(|l| l.strip_prefix("Content-Length:")).map(|v| v.trim().parse().unwrap())?;
                if buf.len() >= h + 4 + len {
                    let body = buf[h + 4..h + 4 + len].to_vec();
                    buf.drain(..h + 4 + len);
                    return serde_json::from_slice(&body).ok();
                }
            }
            let mut chunk = [0u8; 8192];
            let n = tokio::time::timeout(Duration::from_secs(60), stdout.read(&mut chunk)).await.ok()?.ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
    }
    macro_rules! wait {
        ($pred:expr) => {{
            let mut found = None;
            while let Some(m) = next_msg(&mut stdout, &mut buf).await {
                if m["type"] == "event" && m["event"] == "output" {
                    print!("[out] {}", m["body"]["output"].as_str().unwrap_or(""));
                }
                if ($pred)(&m) {
                    found = Some(m);
                    break;
                }
            }
            found.expect("expected DAP message")
        }};
    }

    let id = send!("initialize", json!({ "clientID": "agentz", "adapterID": "debugpy", "linesStartAt1": true, "columnsStartAt1": true, "pathFormat": "path" }));
    wait!(|m: &Value| m["type"] == "response" && m["request_seq"] == id);
    send!("launch", json!({ "type": "debugpy", "request": "launch", "program": format!("{dir}/main.py"), "cwd": &dir, "console": "internalConsole", "justMyCode": true }));
    wait!(|m: &Value| m["type"] == "event" && m["event"] == "initialized");
    let id = send!("setBreakpoints", json!({ "source": { "path": format!("{dir}/main.py") }, "breakpoints": [{ "line": 5 }] }));
    let bp = wait!(|m: &Value| m["type"] == "response" && m["request_seq"] == id);
    println!("breakpoints: {}", bp["body"]);
    send!("configurationDone", json!({}));
    let stopped = wait!(|m: &Value| m["type"] == "event" && m["event"] == "stopped");
    let thread = stopped["body"]["threadId"].clone();
    let id = send!("stackTrace", json!({ "threadId": thread, "levels": 5 }));
    let st = wait!(|m: &Value| m["type"] == "response" && m["request_seq"] == id);
    let top = &st["body"]["stackFrames"][0];
    println!("stopped at {}:{}", top["source"]["path"], top["line"]);
    assert_eq!(top["line"], 5);
    let id = send!("scopes", json!({ "frameId": top["id"] }));
    let sc = wait!(|m: &Value| m["type"] == "response" && m["request_seq"] == id);
    let id = send!("variables", json!({ "variablesReference": sc["body"]["scopes"][0]["variablesReference"] }));
    let vars = wait!(|m: &Value| m["type"] == "response" && m["request_seq"] == id);
    let total = vars["body"]["variables"].as_array().unwrap().iter().find(|v| v["name"] == "total").cloned();
    println!("total = {:?}", total.as_ref().map(|v| &v["value"]));
    assert_eq!(total.unwrap()["value"], "3");
    send!("disconnect", json!({ "terminateDebuggee": true }));
}

#[tokio::test]
#[ignore]
async fn live_mirror_script() {
    let t = target();
    let home = t.run("echo $HOME").await.unwrap().trim().to_string();
    let root = format!("{home}/agentz-e2e-mirror");
    t.run(&format!(
        "rm -rf {r}; mkdir -p {r}/src {r}/node_modules/x && echo a > {r}/src/a.rs && echo b > {r}/src/b.rs && echo skip > {r}/node_modules/x/i.js",
        r = shell_quote(&root)
    ))
    .await
    .unwrap();
    let stamp = format!("{home}/.agentz-server/e2e-mirror.stamp");
    let local = std::env::temp_dir().join("agentz-e2e-mirror");
    let _ = std::fs::remove_dir_all(&local);
    std::fs::create_dir_all(&local).unwrap();

    let full = t.sh(&mirror::sync_script_for_test(&root, &stamp, true)).unwrap().output().await.unwrap();
    mirror::extract_for_test(&local, &full.stdout).await;
    assert!(local.join("src/a.rs").exists() && !local.join("node_modules").exists());

    tokio::time::sleep(Duration::from_millis(1100)).await;
    t.run(&format!("echo changed > {}/src/b.rs", shell_quote(&root))).await.unwrap();
    let inc = t.sh(&mirror::sync_script_for_test(&root, &stamp, false)).unwrap().output().await.unwrap();
    let names = String::from_utf8_lossy(
        &std::process::Command::new("tar").args(["-tzf", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(&inc.stdout)?;
            c.wait_with_output()
        }).unwrap().stdout,
    )
    .to_string();
    println!("incremental: {names:?}");
    assert!(names.contains("b.rs") && !names.contains("a.rs"));
    t.run(&format!("rm -rf {} {}*", shell_quote(&root), shell_quote(&stamp))).await.unwrap();
    let _ = std::fs::remove_dir_all(&local);
}
