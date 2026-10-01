//! `devenv` — inspect the project's development environment and settle the
//! implicit environment todo. Never installs anything: configuration is done
//! by the agent with `shell`, which already asks the user to confirm.

use async_trait::async_trait;
use piscis_core::host::EventSink;
use piscis_kernel::agent::plan::PlanStore;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::commands::devenv;

pub struct DevEnvTool {
    pub plan_store: PlanStore,
    pub event_sink: Arc<dyn EventSink>,
}

#[async_trait]
impl Tool for DevEnvTool {
    fn name(&self) -> &str {
        "devenv"
    }

    fn description(&self) -> &str {
        "Inspect the project's development environment (detected tech stacks, required toolchain, \
         versions, missing tools with per-OS install hints).\n\
         Actions:\n\
         - `check`: rescan now and report every tool.\n\
         - `resolve`: rescan; if nothing is missing any more, clear the implicit environment todo. \
           Call this after you finished configuring the environment.\n\
         - `dismiss`: stop reminding about the given `tools` (ids) — or all current problems when \
           omitted — and clear the todo. Use it when the user does not want them.\n\
         This tool never installs software; install with `shell` (the user confirms) and then \
         call `resolve`."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["check", "resolve", "dismiss"] },
                "tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "For dismiss: tool ids to stop reminding about."
                }
            },
            "required": ["action"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = input["action"].as_str().unwrap_or("").trim();
        let project = ctx.workspace_root.clone();
        match action {
            "check" => {
                let report = devenv::scan(&project, true).await;
                let dismissed = devenv::load_dismissed(&project);
                Ok(ToolResult::ok(devenv::render_report(&report, &dismissed)))
            }
            "resolve" => {
                let report = devenv::scan(&project, true).await;
                let dismissed = devenv::load_dismissed(&project);
                let remaining = report.problems(&dismissed);
                if remaining.is_empty() {
                    devenv::clear_todo(&self.plan_store, &self.event_sink, &ctx.session_id).await;
                    Ok(ToolResult::ok(
                        "Environment looks good; the implicit environment todo is cleared.",
                    ))
                } else {
                    let names: Vec<&str> = remaining.iter().map(|t| t.id.as_str()).collect();
                    Ok(ToolResult::err(format!(
                        "Still missing or outdated: {}.\n\n{}",
                        names.join(", "),
                        devenv::render_report(&report, &dismissed)
                    )))
                }
            }
            "dismiss" => {
                let report = devenv::scan(&project, false).await;
                let existing = devenv::load_dismissed(&project);
                let ids: Vec<String> = match input["tools"].as_array() {
                    Some(list) if !list.is_empty() => list
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect(),
                    _ => report
                        .problems(&existing)
                        .iter()
                        .map(|t| t.id.clone())
                        .collect(),
                };
                if ids.is_empty() {
                    return Ok(ToolResult::ok("Nothing to dismiss."));
                }
                devenv::dismiss(&project, &ids).map_err(|e| anyhow::anyhow!(e))?;
                devenv::clear_todo(&self.plan_store, &self.event_sink, &ctx.session_id).await;
                Ok(ToolResult::ok(format!(
                    "Dismissed: {}. They will not be raised again for this project.",
                    ids.join(", ")
                )))
            }
            other => Ok(ToolResult::err(format!(
                "Unknown action '{other}'. Use check, resolve or dismiss."
            ))),
        }
    }
}
