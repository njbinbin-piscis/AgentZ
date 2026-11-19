//! `robotz-mcp` — an MCP server that exposes the RobotZ computer-use tools
//! (`screen_capture`, `desktop_automation`, `browser`, and `uia` on Windows)
//! over stdio, so any MCP client (Cursor, Claude Desktop, …) can drive the
//! machine.
//!
//! Each RobotZ tool is a `robotz_core::Tool`; this binary bridges the dynamic
//! tool set onto the rmcp `ServerHandler` low-level `list_tools` / `call_tool`
//! seam, preserving the existing JSON schemas and forwarding screenshots as
//! MCP image content.
//!
//! Pass `--readonly` to expose only observation tools (screenshots, window
//! listing, UIA reads) — useful for untrusted clients.

use std::sync::Arc;

use rmcp::{
    model::{Tool as McpTool, *},
    service::RequestContext,
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use robotz_core::{Tool, ToolContext};
use robotz_toolset::{build_tools, default_browser_manager};

#[derive(Clone)]
struct RobotzServer {
    tools: Arc<Vec<Arc<dyn Tool>>>,
}

impl RobotzServer {
    fn new(readonly: bool) -> Self {
        let browser = default_browser_manager();
        Self {
            tools: Arc::new(build_tools(readonly, browser)),
        }
    }

    fn find(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name).cloned()
    }
}

impl ServerHandler for RobotzServer {
    fn get_info(&self) -> ServerInfo {
        // `ServerInfo` / `Implementation` are `#[non_exhaustive]`, so build
        // from `Default` and assign public fields.
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info.name = "robotz-mcp".into();
        info.server_info.version = env!("CARGO_PKG_VERSION").into();
        info.instructions = Some(
            "RobotZ computer-use tools: capture the screen, control mouse/keyboard, \
             manage windows, and automate a browser. Typical loop: call screen_capture \
             with grid=true, identify the target from the labelled grid, then call \
             desktop_automation.click(x, y)."
                .into(),
        );
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = self
            .tools
            .iter()
            .map(|t| {
                let schema = t
                    .input_schema()
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                let mut annotations = ToolAnnotations::default();
                annotations.read_only_hint = Some(t.is_read_only());
                annotations.destructive_hint = Some(!t.is_read_only());
                McpTool::new(
                    t.name().to_string(),
                    t.description().to_string(),
                    Arc::new(schema),
                )
                .annotate(annotations)
            })
            .collect();
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name = request.name.as_ref();
        let Some(tool) = self.find(name) else {
            return Err(McpError::invalid_params(
                format!("unknown tool: {name}"),
                None,
            ));
        };

        let input = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or(serde_json::Value::Null);
        let ctx = ToolContext::default();

        match tool.call(input, &ctx).await {
            Ok(result) => {
                let mut contents = vec![Content::text(result.content)];
                if let Some(img) = result.image {
                    contents.push(Content::image(img.base64, img.media_type));
                }
                if result.is_error {
                    Ok(CallToolResult::error(contents))
                } else {
                    Ok(CallToolResult::success(contents))
                }
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let readonly = std::env::args().any(|a| a == "--readonly");
    tracing::info!("starting robotz-mcp (readonly={readonly})");

    let service = RobotzServer::new(readonly)
        .serve(rmcp::transport::stdio())
        .await
        .inspect_err(|e| tracing::error!("failed to start server: {e}"))?;
    service.waiting().await?;
    Ok(())
}
