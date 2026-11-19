//! MCP client that spawns `robotz-mcp` as a child process (stdio transport).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use base64::Engine;
use rmcp::{
    model::{CallToolRequestParams, CallToolResult, ListToolsResult, RawContent},
    serve_client, ClientHandler, RoleClient,
};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use tokio::process::Command;
use serde_json::{json, Map, Value};

/// Minimal MCP client handler (no sampling / elicitation).
#[derive(Clone, Default)]
pub struct HostMcpClient;

impl ClientHandler for HostMcpClient {}

type McpRunning = rmcp::service::RunningService<RoleClient, HostMcpClient>;

/// Live connection to a `robotz-mcp` subprocess.
pub struct McpSession {
    rt: tokio::runtime::Runtime,
    running: Mutex<Option<Arc<McpRunning>>>,
    mcp_path: PathBuf,
}

impl McpSession {
    pub fn new(mcp_path: PathBuf) -> Self {
        Self {
            rt: tokio::runtime::Runtime::new().expect("tokio runtime"),
            running: Mutex::new(None),
            mcp_path,
        }
    }

    pub fn mcp_path(&self) -> &Path {
        &self.mcp_path
    }

    pub fn is_connected(&self) -> bool {
        self.running.lock().unwrap().is_some()
    }

    pub fn connect_sync(&self, readonly: bool) -> Result<()> {
        self.rt.block_on(self.connect_async(readonly))
    }

    pub async fn connect_async(&self, readonly: bool) -> Result<()> {
        let path = self.mcp_path.clone();
        let transport = TokioChildProcess::new(Command::new(&path).configure(|cmd| {
            if readonly {
                cmd.arg("--readonly");
            }
        }))
        .context("spawn robotz-mcp")?;

        let running = Arc::new(serve_client(HostMcpClient, transport).await?);
        *self.running.lock().unwrap() = Some(running);
        Ok(())
    }

    pub fn disconnect_sync(&self) {
        *self.running.lock().unwrap() = None;
    }

    pub fn list_tools_sync(&self) -> Result<ListToolsResult> {
        let running = self.running_arc()?;
        self.rt
            .block_on(async { running.list_tools(None).await.map_err(Into::into) })
    }

    pub fn call_tool_sync(&self, name: &str, input: Value) -> Result<CallToolResult> {
        let args = match input {
            Value::Object(map) => Some(map),
            Value::Null => None,
            other => Some(Map::from_iter([("input".into(), other)])),
        };
        let running = self.running_arc()?;
        let tool_name = name.to_string();
        self.rt.block_on(async move {
            let mut params = CallToolRequestParams::new(tool_name);
            if let Some(args) = args {
                params = params.with_arguments(args);
            }
            running.call_tool(params).await.map_err(Into::into)
        })
    }

    fn running_arc(&self) -> Result<Arc<McpRunning>> {
        self.running
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("MCP not connected — use Connect MCP first"))
    }
}

/// Resolve `robotz-mcp` next to the current executable (debug/release builds).
pub fn find_robotz_mcp_binary() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join(if cfg!(windows) {
                "robotz-mcp.exe"
            } else {
                "robotz-mcp"
            });
            if sibling.exists() {
                return sibling;
            }
        }
    }
    PathBuf::from("robotz-mcp")
}

/// Convert MCP result to text (+ optional PNG bytes) for the panel.
pub fn mcp_result_summary(result: &CallToolResult) -> (String, Option<Vec<u8>>, bool) {
    let mut text = String::new();
    let mut png = None;
    for c in &result.content {
        match &c.raw {
            RawContent::Text(t) => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&t.text);
            }
            RawContent::Image(img) => {
                if img.mime_type.contains("png") || img.mime_type.contains("jpeg") {
                    if let Ok(bytes) =
                        base64::engine::general_purpose::STANDARD.decode(&img.data)
                    {
                        png = Some(bytes);
                    }
                }
            }
            _ => {}
        }
    }
    let is_error = result.is_error.unwrap_or(false);
    if text.is_empty() {
        text = if is_error {
            "tool error".into()
        } else {
            "ok".into()
        };
    }
    (text, png, is_error)
}

/// Smoke demo: list tools + two read-only calls.
pub fn mcp_demo_sync(readonly: bool) -> Result<()> {
    let session = McpSession::new(find_robotz_mcp_binary());
    session.connect_sync(readonly)?;
    let tools = session.list_tools_sync()?;
    println!("MCP tools ({}):", tools.tools.len());
    for t in &tools.tools {
        println!("  - {}", t.name);
    }
    for (name, input) in [
        ("screen_capture", json!({ "action": "list_monitors" })),
        (
            "desktop_automation",
            json!({ "action": "get_cursor_position" }),
        ),
    ] {
        let r = session.call_tool_sync(name, input)?;
        let (text, _, err) = mcp_result_summary(&r);
        println!("\n[{name}] error={err}\n{text}\n");
    }
    session.disconnect_sync();
    Ok(())
}
