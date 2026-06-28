//! `graph_explore` — surgical structural context (Phase 0).
//!
//! Returns numbered source + import blast radius from `.agentz/graph.db`.
//! See `docs/graph-explore-spec.md`.

use async_trait::async_trait;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

use crate::commands::graph_db::explore_graph;

pub struct GraphExploreTool;

#[async_trait]
impl Tool for GraphExploreTool {
    fn name(&self) -> &str {
        "graph_explore"
    }

    fn description(&self) -> &str {
        "Explore the repository knowledge graph in one call — returns verbatim numbered \
         source, import/dependency flow, and blast radius for matched files.\n\
         \n\
         Use FIRST for structural questions (how does X work, who calls Y, what depends on Z). \
         Then use `codebase_search` only for extra implementation detail.\n\
         \n\
         Legacy `graph_search` returns node ids only; prefer this tool.\n\
         \n\
         Parameters:\n\
         - 'query' (string): symbol name, path fragment, or natural language topic.\n\
         - 'limit' (number): max seed nodes (default 12, max 25).\n\
         \n\
         Requires `.agentz/graph.db` (auto-built from graph.json on first use)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Topic, symbol, or path." },
                "limit": { "type": "integer", "description": "Max nodes (default 12)." }
            },
            "required": ["query"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let query = input
            .get("query")
            .and_then(|q| q.as_str())
            .unwrap_or("")
            .to_string();
        let limit = input.get("limit").and_then(|l| l.as_u64()).unwrap_or(12) as usize;
        let root = ctx.workspace_root.clone();

        match tokio::task::spawn_blocking(move || explore_graph(&root, &query, limit)).await {
            Ok(Ok(text)) => Ok(ToolResult::ok(text)),
            Ok(Err(e)) => Ok(ToolResult::err(format!("graph_explore failed: {e}"))),
            Err(e) => Ok(ToolResult::err(format!("graph_explore task failed: {e}"))),
        }
    }
}
