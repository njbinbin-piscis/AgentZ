//! The core [`Tool`] abstraction and its supporting value types.
//!
//! Deliberately shaped like `piscis_kernel::agent::tool` (same method names,
//! same `ToolResult`/`ImageData`) so that automation tools port over with a
//! single import swap. Fields that only the full piscis agent loop needs
//! (email credentials, memory owner, pool session, …) are intentionally
//! dropped — the computer-use tools never read them.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Context handed to every tool invocation.
///
/// Hosts (the MCP server, an embedded agent, or openpiscis via the
/// `piscis-kernel` feature) construct this per call. Computer-use tools only
/// rely on the cooperative `cancel` flag and, occasionally, `workspace_root`
/// for resolving relative `output_path`s.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Logical session / connection identifier (for logging & correlation).
    pub session_id: String,
    /// Root used to resolve relative artifact paths (e.g. screenshot
    /// `output_path`). Defaults to the process working directory.
    pub workspace_root: PathBuf,
    /// When true, skip any host-side permission / confirmation prompts.
    pub bypass_permissions: bool,
    /// Cooperative cancellation flag for long-running tools.
    pub cancel: Arc<AtomicBool>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            bypass_permissions: false,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ToolContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// Image payload attached to a tool result (for Vision-capable models / MCP
/// `ImageContent`).
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Base64-encoded image bytes (no data-URL prefix).
    pub base64: String,
    /// MIME type, e.g. `image/png` or `image/jpeg`.
    pub media_type: String,
}

impl ImageData {
    pub fn png(base64: impl Into<String>) -> Self {
        Self {
            base64: base64.into(),
            media_type: "image/png".into(),
        }
    }
    pub fn jpeg(base64: impl Into<String>) -> Self {
        Self {
            base64: base64.into(),
            media_type: "image/jpeg".into(),
        }
    }
}

/// Result of a tool execution.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Textual content surfaced to the model / MCP client.
    pub content: String,
    /// Whether this represents an error.
    pub is_error: bool,
    /// Optional image (screenshot etc.).
    pub image: Option<ImageData>,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            image: None,
        }
    }
    pub fn err(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            image: None,
        }
    }
    pub fn with_image(mut self, image: ImageData) -> Self {
        self.image = Some(image);
        self
    }
}

/// The trait every RobotZ tool implements.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name (used in tool definitions / MCP tool registration).
    fn name(&self) -> &str;

    /// Human-readable description.
    fn description(&self) -> &str;

    /// JSON Schema for the tool's input parameters.
    fn input_schema(&self) -> Value;

    /// Whether this tool only observes (screenshot, list windows, UIA reads)
    /// and is therefore safe in `--readonly` deployments and concurrent runs.
    fn is_read_only(&self) -> bool {
        false
    }

    /// Whether this call mutates host state in a way the host may want to
    /// confirm (clicks, typing, drags). Hosts can surface this as an MCP
    /// `destructiveHint` annotation.
    fn needs_confirmation(&self, _input: &Value) -> bool {
        !self.is_read_only()
    }

    /// Execute the tool.
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult>;
}
