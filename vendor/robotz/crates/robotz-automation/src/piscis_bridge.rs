//! Adapter layer (enabled by the `piscis-kernel` feature) that re-implements
//! `piscis_kernel::agent::tool::Tool` on top of the RobotZ tools.
//!
//! This lets openpiscis consume `robotz-automation` as a drop-in replacement
//! for its old in-tree `screen` / `desktop_automation` tools: the registry
//! keeps handing the kernel `Box<dyn piscis_kernel::Tool>`, but the actual
//! implementation now lives here.
//!
//! The bridge is intentionally thin: the computer-use tools don't read any of
//! the rich `piscis_kernel::ToolContext` fields, so we only carry across the
//! handful that map cleanly (`session_id`, `workspace_root`,
//! `bypass_permissions`, `cancel`).

use async_trait::async_trait;
use piscis_kernel::agent::tool as pk;
use serde_json::Value;

use crate::{DesktopAutomationTool, ScreenTool};

#[cfg(target_os = "windows")]
use crate::UiaTool;

fn to_robotz_ctx(ctx: &pk::ToolContext) -> robotz_core::ToolContext {
    robotz_core::ToolContext {
        session_id: ctx.session_id.clone(),
        workspace_root: ctx.workspace_root.clone(),
        bypass_permissions: ctx.bypass_permissions,
        cancel: ctx.cancel.clone(),
    }
}

fn to_pk_result(r: robotz_core::ToolResult) -> pk::ToolResult {
    let mut out = if r.is_error {
        pk::ToolResult::err(r.content)
    } else {
        pk::ToolResult::ok(r.content)
    };
    if let Some(img) = r.image {
        out = out.with_image(pk::ImageData {
            base64: img.base64,
            media_type: img.media_type,
        });
    }
    out
}

macro_rules! bridge_tool {
    ($t:ty) => {
        #[async_trait]
        impl pk::Tool for $t {
            fn name(&self) -> &str {
                robotz_core::Tool::name(self)
            }
            fn description(&self) -> &str {
                robotz_core::Tool::description(self)
            }
            fn input_schema(&self) -> Value {
                robotz_core::Tool::input_schema(self)
            }
            fn is_read_only(&self) -> bool {
                robotz_core::Tool::is_read_only(self)
            }
            fn needs_confirmation(&self, input: &Value) -> bool {
                robotz_core::Tool::needs_confirmation(self, input)
            }
            async fn call(&self, input: Value, ctx: &pk::ToolContext) -> anyhow::Result<pk::ToolResult> {
                let rctx = to_robotz_ctx(ctx);
                robotz_core::Tool::call(self, input, &rctx).await.map(to_pk_result)
            }
        }
    };
}

bridge_tool!(ScreenTool);
bridge_tool!(DesktopAutomationTool);
#[cfg(target_os = "windows")]
bridge_tool!(UiaTool);
