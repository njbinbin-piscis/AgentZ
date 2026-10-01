//! `symbol_search` and `impact` — tree-sitter symbol index tools.

use async_trait::async_trait;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

use crate::commands::symbols::{impact_report, index_for, search_symbols};

pub struct SymbolSearchTool;
pub struct ImpactTool;

#[async_trait]
impl Tool for SymbolSearchTool {
    fn name(&self) -> &str {
        "symbol_search"
    }

    fn description(&self) -> &str {
        "Find definitions of functions, methods, classes, structs, traits, types by name \
         (Rust, TypeScript/JavaScript, Python, Go). Returns kind, container, file and line range. \
         Faster and more precise than text search when you know (part of) a symbol name.\n\
         Parameters: 'query' (string, substring match), 'kind' (optional: fn, function, method, \
         class, struct, enum, trait, interface, type, mod, const, func), 'limit' (default 20)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "kind": { "type": "string" },
                "limit": { "type": "integer" }
            },
            "required": ["query"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let kind = input.get("kind").and_then(|v| v.as_str()).map(str::to_string);
        let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(20).clamp(1, 100) as usize;
        let root = ctx.workspace_root.clone();
        match tokio::task::spawn_blocking(move || {
            let idx = index_for(&root);
            search_symbols(&idx, &query, kind.as_deref(), limit)
        })
        .await
        {
            Ok(t) => Ok(ToolResult::ok(t)),
            Err(e) => Ok(ToolResult::err(format!("symbol_search failed: {e}"))),
        }
    }
}

#[async_trait]
impl Tool for ImpactTool {
    fn name(&self) -> &str {
        "impact"
    }

    fn description(&self) -> &str {
        "Impact analysis before changing a symbol: lists where it is defined, who calls it \
         (transitively, up to 'depth'), and the files to review/test afterwards. Call resolution \
         is by name, so common names may over-approximate.\n\
         Parameters: 'symbol' (exact name), 'depth' (default 2, max 5), 'limit' (default 40)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": { "type": "string" },
                "depth": { "type": "integer" },
                "limit": { "type": "integer" }
            },
            "required": ["symbol"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let symbol = input.get("symbol").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        if symbol.is_empty() {
            return Ok(ToolResult::err("impact: 'symbol' is required"));
        }
        let depth = input.get("depth").and_then(|v| v.as_u64()).unwrap_or(2).clamp(1, 5) as usize;
        let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(40).clamp(5, 200) as usize;
        let root = ctx.workspace_root.clone();
        match tokio::task::spawn_blocking(move || {
            let idx = index_for(&root);
            impact_report(&idx, &symbol, depth, limit)
        })
        .await
        {
            Ok(t) => Ok(ToolResult::ok(t)),
            Err(e) => Ok(ToolResult::err(format!("impact failed: {e}"))),
        }
    }
}
