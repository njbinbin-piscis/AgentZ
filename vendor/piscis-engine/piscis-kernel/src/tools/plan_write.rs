//! `plan_write` — write structured execution plans to `.agentz/plans/*.md`.
//!
//! Enabled in Plan mode as the **only** write surface; also available in Agent
//! mode so the agent can update step status and execution records.

use crate::agent::plan_doc::{
    default_plan_rel_path, is_allowed_plan_path, plan_template, validate_plan_content, PLANS_DIR,
};
use crate::agent::tool::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::borrow::Cow;

pub struct PlanWriteTool;

#[async_trait]
impl Tool for PlanWriteTool {
    fn name(&self) -> &str {
        "plan_write"
    }

    fn description(&self) -> &str {
        "Write or replace the structured execution plan markdown under `.agentz/plans/`. \
         Plan mode: this is the ONLY file you may write — use read-only tools to explore, \
         then persist the full plan here. Each step must be atomic and list expected \
         artifacts plus verifiable evidence. Agent mode: update the same file to track \
         step status and execution records as you work. \
         Paths must be relative like `.agentz/plans/<name>.md`."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path under workspace, must be `.agentz/plans/*.md`. Defaults to session-scoped path."
                },
                "content": {
                    "type": "string",
                    "description": "Full markdown plan body. Must include `# 执行步骤` with `## Step N:` blocks, each having 状态/描述/预期产物/验收证据/执行记录 fields."
                },
                "title": {
                    "type": "string",
                    "description": "When content is omitted, generate a starter template with this title."
                }
            }
        })
    }

    fn description_minimal(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Write/replace `.agentz/plans/*.md` execution plan (Plan mode's only write tool).",
        )
    }

    fn input_schema_minimal(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" },
                "title": { "type": "string" }
            }
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let rel_path = input["path"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| default_plan_rel_path(&ctx.session_id));

        if !is_allowed_plan_path(&ctx.workspace_root, &rel_path) {
            return Ok(ToolResult::err(format!(
                "路径无效：Plan 文件必须位于 `{PLANS_DIR}/` 下且扩展名为 `.md`，当前: {rel_path}"
            )));
        }

        let content = match input["content"].as_str() {
            Some(c) if !c.trim().is_empty() => c.to_string(),
            _ => {
                let title = input["title"].as_str().unwrap_or("未命名任务");
                plan_template(title, &ctx.session_id)
            }
        };

        if let Err(e) = validate_plan_content(&content) {
            return Ok(ToolResult::err(format!("计划格式校验失败: {e}")));
        }

        let abs = ctx.workspace_root.join(&rel_path);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&abs, content.as_bytes())?;

        Ok(ToolResult::ok(format!(
            "计划已写入 `{rel_path}`（{} 个步骤）。Plan 模式完成后请总结 trade-off 并询问用户是否切换到 Agent 模式执行。",
            content.lines().filter(|l| l.starts_with("## Step")).count()
        )))
    }
}
