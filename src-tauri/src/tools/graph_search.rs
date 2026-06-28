//! `graph_search` — agent tool for structural codebase navigation (P2).
//!
//! Complements `codebase_search` (text/snippets) with module/file dependency
//! relationships from `.agentz/graph.json`.

use async_trait::async_trait;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

use crate::commands::graph_agent::search_graph;

pub struct GraphSearchTool;

#[async_trait]
impl Tool for GraphSearchTool {
    fn name(&self) -> &str {
        "graph_search"
    }

    fn description(&self) -> &str {
        "Search the repository knowledge graph for modules, files, and import/dependency \
         relationships. Use BEFORE editing unfamiliar code to find hub files, module \
         boundaries, and what depends on a file.\n\
         \n\
         Complements `codebase_search` (code snippets) — use graph_search for structure, \
         codebase_search for implementation details.\n\
         \n\
         Parameters:\n\
         - 'query' (string): module name, file path fragment, layer (api/service/data/ui), \
           or empty for repo overview.\n\
         - 'limit' (number): max matched nodes (default 12).\n\
         \n\
         Requires `.agentz/graph.json` (Wiki → Rebuild graph)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Module/file/layer search term." },
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

        match tokio::task::spawn_blocking(move || search_graph(&root, &query, limit)).await {
            Ok(Ok(text)) => Ok(ToolResult::ok(text)),
            Ok(Err(e)) => Ok(ToolResult::err(format!("graph_search failed: {e}"))),
            Err(e) => Ok(ToolResult::err(format!("graph_search task failed: {e}"))),
        }
    }
}
