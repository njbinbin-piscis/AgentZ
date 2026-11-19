//! Async helpers to invoke RobotZ tools from the UI thread.

use std::sync::Arc;

use anyhow::Result;
use robotz_core::{Tool, ToolContext, ToolResult};
use serde_json::Value;

pub struct ToolRunner {
    tools: Arc<Vec<Arc<dyn Tool>>>,
    rt: tokio::runtime::Runtime,
}

impl ToolRunner {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        Self {
            tools: Arc::new(tools),
            rt: tokio::runtime::Runtime::new().expect("tokio runtime"),
        }
    }

    pub fn tools(&self) -> &[Arc<dyn Tool>] {
        &self.tools
    }

    pub fn find(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name).cloned()
    }

    pub fn call_sync(&self, name: &str, input: Value) -> Result<ToolResult> {
        let tool = self
            .find(name)
            .ok_or_else(|| anyhow::anyhow!("unknown tool: {name}"))?;
        let ctx = ToolContext::default();
        self.rt.block_on(tool.call(input, &ctx))
    }
}
