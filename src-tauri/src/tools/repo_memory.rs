//! `repo_memory` — Agent tool for querying, recording, and verifying source-anchored
//! repository architecture notes (`repo_notes` in `.agentz/graph.db` + `.agentz/REPO_MEMORY.md`).

use async_trait::async_trait;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

use crate::commands::graph_db::{
    delete_repo_note, query_repo_notes, record_repo_note, verify_repo_note, NoteCitation,
    RepoNoteInput,
};

pub struct RepoMemoryTool;

#[async_trait]
impl Tool for RepoMemoryTool {
    fn name(&self) -> &str {
        "repo_memory"
    }

    fn description(&self) -> &str {
        "Read or write source-anchored repository memory (`.agentz/REPO_MEMORY.md` & `graph.db`).\n\
         \n\
         Unlike personal user-preference memory, `repo_memory` stores engineering knowledge bound \
         to specific source file ranges (`path`, `start_line`, `end_line`, `symbol`). When the cited \
         source lines change on disk, the note is automatically marked `stale` so outdated notes \
         never mislead future sessions.\n\
         \n\
         Actions:\n\
         - `query`: Search repository notes by module, file path, symbol, or keyword.\n\
         - `record`: Save a newly discovered module invariant, debugging pitfall, cross-boundary \
           IPC contract, or architectural decision with source citations.\n\
         - `verify`: Re-anchor a `stale` note (`note_id`) after confirming it still holds on current code.\n\
         - `delete`: Remove an obsolete note (`note_id`)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["query", "record", "verify", "delete"],
                    "description": "Operation to perform."
                },
                "query": {
                    "type": "string",
                    "description": "For `query`: optional module, file path, symbol, or keyword filter."
                },
                "include_stale": {
                    "type": "boolean",
                    "description": "For `query`: include stale notes (default true)."
                },
                "note_id": {
                    "type": "string",
                    "description": "For `verify` or `delete`: target note ID (e.g. `note_...`)."
                },
                "scope_kind": {
                    "type": "string",
                    "enum": ["module", "file", "symbol", "architecture"],
                    "description": "For `record`: granularity of the note (default `module`)."
                },
                "scope_target": {
                    "type": "string",
                    "description": "For `record`: logical module (e.g. `src-tauri/commands`) or file path."
                },
                "kind": {
                    "type": "string",
                    "enum": ["invariant", "pitfall", "dataflow", "decision"],
                    "description": "For `record`: category of engineering knowledge."
                },
                "title": {
                    "type": "string",
                    "description": "For `record`: concise one-line title."
                },
                "body": {
                    "type": "string",
                    "description": "For `record`: clear explanation of the invariant, pitfall, or contract."
                },
                "citations": {
                    "type": "array",
                    "description": "For `record`: source file anchors whose hash will be tracked for staleness.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Repo-relative file path." },
                            "start_line": { "type": "integer", "description": "Optional 1-based start line." },
                            "end_line": { "type": "integer", "description": "Optional 1-based end line." },
                            "symbol": { "type": "string", "description": "Optional function/struct name." }
                        },
                        "required": ["path"]
                    }
                }
            },
            "required": ["action"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("query")
            .trim()
            .to_string();
        let root = ctx.workspace_root.clone();

        match action.as_str() {
            "query" => {
                let q = input
                    .get("query")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let include_stale = input
                    .get("include_stale")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let res = tokio::task::spawn_blocking(move || {
                    query_repo_notes(&root, q.as_deref(), include_stale, 25)
                })
                .await;
                match res {
                    Ok(Ok(notes)) => {
                        if notes.is_empty() {
                            return Ok(ToolResult::ok(
                                "No matching repository memory notes found.".to_string(),
                            ));
                        }
                        let mut out = format!("Found {} repository memory note(s):\n\n", notes.len());
                        for n in notes {
                            let status = if n.stale {
                                "⚠️ STALE (cited source changed — verify before relying)"
                            } else {
                                "✓ ACTIVE"
                            };
                            let cites = n
                                .citations
                                .iter()
                                .map(|c| match (c.start_line, c.end_line) {
                                    (Some(s), Some(e)) => format!("`{}:{s}-{e}`", c.path),
                                    (Some(s), None) => format!("`{}:{s}`", c.path),
                                    _ => format!("`{}`", c.path),
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            out.push_str(&format!(
                                "- **`{}`** [{} | {}] **{}** (`{}`)\n  {}\n",
                                n.id, status, n.kind, n.title, n.scope_target, n.body
                            ));
                            if !cites.is_empty() {
                                out.push_str(&format!("  Anchors: {cites}\n"));
                            }
                        }
                        Ok(ToolResult::ok(out))
                    }
                    Ok(Err(e)) => Ok(ToolResult::err(format!("repo_memory query failed: {e}"))),
                    Err(e) => Ok(ToolResult::err(format!("repo_memory task failed: {e}"))),
                }
            }
            "record" => {
                let scope_kind = input
                    .get("scope_kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("module")
                    .to_string();
                let scope_target = input
                    .get("scope_target")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(root)")
                    .to_string();
                let kind = input
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("invariant")
                    .to_string();
                let title = input
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let body = input
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let citations: Vec<NoteCitation> = input
                    .get("citations")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();

                let note_in = RepoNoteInput {
                    scope_kind,
                    scope_target,
                    kind,
                    title,
                    body,
                    citations,
                    source: "agent_tool".to_string(),
                };
                let res =
                    tokio::task::spawn_blocking(move || record_repo_note(&root, note_in)).await;
                match res {
                    Ok(Ok(note)) => Ok(ToolResult::ok(format!(
                        "Recorded repository note `{}` (**{}** under `{}`) and synced `.agentz/REPO_MEMORY.md`.",
                        note.id, note.title, note.scope_target
                    ))),
                    Ok(Err(e)) => Ok(ToolResult::err(format!("repo_memory record failed: {e}"))),
                    Err(e) => Ok(ToolResult::err(format!("repo_memory task failed: {e}"))),
                }
            }
            "verify" => {
                let note_id = input
                    .get("note_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if note_id.trim().is_empty() {
                    return Ok(ToolResult::err(
                        "verify requires `note_id` parameter".to_string(),
                    ));
                }
                let res =
                    tokio::task::spawn_blocking(move || verify_repo_note(&root, &note_id)).await;
                match res {
                    Ok(Ok(note)) => Ok(ToolResult::ok(format!(
                        "Verified and re-anchored note `{}` (**{}**).",
                        note.id, note.title
                    ))),
                    Ok(Err(e)) => Ok(ToolResult::err(format!("repo_memory verify failed: {e}"))),
                    Err(e) => Ok(ToolResult::err(format!("repo_memory task failed: {e}"))),
                }
            }
            "delete" => {
                let note_id = input
                    .get("note_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if note_id.trim().is_empty() {
                    return Ok(ToolResult::err(
                        "delete requires `note_id` parameter".to_string(),
                    ));
                }
                let res =
                    tokio::task::spawn_blocking(move || delete_repo_note(&root, &note_id)).await;
                match res {
                    Ok(Ok(true)) => Ok(ToolResult::ok(
                        "Deleted repository note and refreshed `.agentz/REPO_MEMORY.md`."
                            .to_string(),
                    )),
                    Ok(Ok(false)) => Ok(ToolResult::ok("Note ID not found.".to_string())),
                    Ok(Err(e)) => Ok(ToolResult::err(format!("repo_memory delete failed: {e}"))),
                    Err(e) => Ok(ToolResult::err(format!("repo_memory task failed: {e}"))),
                }
            }
            other => Ok(ToolResult::err(format!(
                "Unsupported repo_memory action `{other}` (expected query, record, verify, or delete)"
            ))),
        }
    }
}
