//! LSP tool for agents — diagnostics, hover, completion, go-to-definition,
//! references, symbols and rename preview, backed by a real, persistent
//! language-server session ([`crate::lsp::client::LspClient`]).

use async_trait::async_trait;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::time::Duration;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::lsp::client::{uri_to_path, LspClient};
use crate::lsp::manager::LspManager;

pub struct LspTool {
    pub lsp_manager: Arc<LspManager>,
}

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        "lsp"
    }

    fn description(&self) -> &str {
        "Semantic code navigation through a real language server (rust-analyzer, \
         typescript-language-server, pyright, clangd). Prefer this over grep when you \
         need to know what a symbol IS, where it is DEFINED, or who USES it.\n\
         Actions:\n\
         - 'definition': where the symbol at file:line:character is defined.\n\
         - 'references': every usage of that symbol (use before changing a signature \
         or renaming).\n\
         - 'hover': type / documentation of the symbol.\n\
         - 'diagnostics': compiler/type errors for the file (no line/character needed).\n\
         - 'symbols': outline of the file (functions, types, methods with line numbers; \
         no line/character needed).\n\
         - 'workspace_symbols': find symbols by name across the project (needs 'query'; \
         'file' only selects which project/language).\n\
         - 'complete': completions at a position.\n\
         - 'rename': PREVIEW of a project-wide rename (needs 'new_name'); it lists the \
         affected files but does not modify them — apply the edits yourself.\n\
         Positions: 'line' is 1-based, 'character' is the 0-based column. The first call \
         for a language starts its server and can take several seconds. If the server \
         is not installed the error says how to install it; then fall back to \
         codebase_search / file_search."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["definition", "references", "hover", "diagnostics", "symbols",
                             "workspace_symbols", "complete", "rename"],
                    "description": "LSP action to perform."
                },
                "file": { "type": "string", "description": "Absolute path to the source file." },
                "line": { "type": "integer", "description": "1-based line (definition/references/hover/complete/rename)." },
                "character": { "type": "integer", "description": "0-based column (same actions as 'line')." },
                "new_name": { "type": "string", "description": "Required for 'rename'." },
                "query": { "type": "string", "description": "Symbol name for 'workspace_symbols'." }
            },
            "required": ["action", "file"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = input["action"]
            .as_str()
            .unwrap_or("diagnostics")
            .to_string();
        let file = input["file"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("'file' parameter is required"))?
            .to_string();
        let position_actions = ["definition", "references", "hover", "complete", "rename"];
        if position_actions.contains(&action.as_str())
            && (input.get("line").is_none() || input.get("character").is_none())
        {
            return Ok(ToolResult::err(format!(
                "'{action}' needs both 'line' (1-based) and 'character' (0-based)"
            )));
        }
        if !std::path::Path::new(&file).is_file() {
            return Ok(ToolResult::err(format!("file not found: {file}")));
        }

        let language = match LspManager::language_for_extension(&file) {
            Some(l) => l,
            None => {
                return Ok(ToolResult::err(format!(
                    "no language server is mapped to this file type ({file}). Mapped: {}",
                    mapped_languages()
                )));
            }
        };
        let root = detect_project_root(&file);
        let client = match self.lsp_manager.client(&root, &language).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult::err(format!(
                    "LSP unavailable for {language}: {e}"
                )))
            }
        };
        match run_action(
            &client,
            &action,
            &file,
            &language,
            &input,
            &ctx.workspace_root,
        )
        .await
        {
            Ok(text) => Ok(ToolResult::ok(text)),
            Err(e) => Ok(ToolResult::err(e)),
        }
    }
}

fn mapped_languages() -> String {
    LspManager::supported_languages()
        .iter()
        .map(|l| {
            format!(
                "{} ({})",
                l.language_id,
                if l.available {
                    "installed"
                } else {
                    "server not installed"
                }
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

async fn run_action(
    client: &LspClient,
    action: &str,
    file: &str,
    language: &str,
    input: &Value,
    workspace_root: &Path,
) -> Result<String, String> {
    let lsp_id = LspManager::lsp_language_id(file, language);
    let (uri, changed) = client.sync_file(file, &lsp_id).await?;

    if action == "diagnostics" {
        let diags = client
            .diagnostics(&uri, changed, Duration::from_secs(8))
            .await;
        return Ok(format_diagnostics(&diags));
    }

    let (method, params) = action_request(action, &uri, input)?;
    request_with_retry(client, action, method, params, changed)
        .await
        .map(|result| format_result_scoped(action, &result, workspace_root))
}

/// LSP method and params for a non-diagnostics tool action on document `uri`.
pub fn action_request(
    action: &str,
    uri: &str,
    input: &Value,
) -> Result<(&'static str, Value), String> {
    let line = input["line"].as_u64().unwrap_or(1).saturating_sub(1);
    let character = input["character"].as_u64().unwrap_or(0);
    let position = json!({ "line": line, "character": character });
    let td = json!({ "uri": uri });

    Ok(match action {
        "hover" => (
            "textDocument/hover",
            json!({ "textDocument": td, "position": position }),
        ),
        "complete" => (
            "textDocument/completion",
            json!({ "textDocument": td, "position": position, "context": { "triggerKind": 1 } }),
        ),
        "definition" => (
            "textDocument/definition",
            json!({ "textDocument": td, "position": position }),
        ),
        "references" => (
            "textDocument/references",
            json!({ "textDocument": td, "position": position,
                    "context": { "includeDeclaration": true } }),
        ),
        "rename" => {
            let new_name = input["new_name"]
                .as_str()
                .ok_or("'rename' requires 'new_name'")?;
            (
                "textDocument/rename",
                json!({ "textDocument": td, "position": position, "newName": new_name }),
            )
        }
        "symbols" => ("textDocument/documentSymbol", json!({ "textDocument": td })),
        "workspace_symbols" => {
            let q = input["query"]
                .as_str()
                .ok_or("'workspace_symbols' requires 'query'")?;
            ("workspace/symbol", json!({ "query": q }))
        }
        other => return Err(format!("unknown action '{other}'")),
    })
}

/// Send `method`, retrying while a freshly started server is still indexing.
pub async fn request_with_retry(
    client: &LspClient,
    action: &str,
    method: &str,
    params: Value,
    changed: bool,
) -> Result<Value, String> {
    if changed {
        tokio::time::sleep(Duration::from_millis(600)).await;
    }
    let mut last_err = String::new();
    for attempt in 0..4 {
        match client
            .request(method, params.clone(), Duration::from_secs(30))
            .await
        {
            Ok(result) => {
                let empty = result.is_null() || result.as_array().is_some_and(|a| a.is_empty());
                // A freshly started server is often still indexing and answers
                // with an empty result; give it a couple more chances.
                if empty && attempt < 2 && changed {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    continue;
                }
                return Ok(result);
            }
            Err(e) => {
                let retryable = {
                    let l = e.to_lowercase();
                    l.contains("content modified")
                        || l.contains("contentmodified")
                        || l.contains("waiting")
                        || l.contains("not initialized")
                };
                last_err = e;
                if !retryable {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(1500)).await;
            }
        }
    }
    Err(format!("LSP {action} failed: {last_err}"))
}

/// Detect project root from a file path by looking for common markers.
fn detect_project_root(file: &str) -> String {
    let path = std::path::Path::new(file);
    let mut current = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    };

    let markers = [
        "Cargo.toml",
        "package.json",
        "tsconfig.json",
        "pyproject.toml",
        "setup.py",
        "CMakeLists.txt",
        "Makefile",
    ];

    // Prefer the outermost Cargo workspace / package root nearest the file; the
    // first marker wins, which matches how the servers locate their project.
    loop {
        for marker in &markers {
            if current.join(marker).exists() {
                return current.to_string_lossy().to_string();
            }
        }
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    std::path::Path::new(file)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string())
}

pub fn format_result(action: &str, result: &Value) -> String {
    match action {
        "hover" => match result.get("contents") {
            Some(Value::String(s)) => format!("Hover: {s}"),
            Some(Value::Object(o)) => format!(
                "Hover: {}",
                o.get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(hover info)")
            ),
            Some(Value::Array(a)) => format!(
                "Hover: {}",
                a.iter()
                    .filter_map(|v| v
                        .as_str()
                        .or_else(|| v.get("value").and_then(|x| x.as_str())))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            _ => "No hover information at this position.".to_string(),
        },
        "complete" => {
            let items = result
                .get("items")
                .and_then(|i| i.as_array())
                .or_else(|| result.as_array());
            match items {
                Some(items) if !items.is_empty() => {
                    let lines: Vec<String> = items
                        .iter()
                        .take(30)
                        .map(|it| {
                            let label = it.get("label").and_then(|l| l.as_str()).unwrap_or("?");
                            let detail = it
                                .get("detail")
                                .and_then(|d| d.as_str())
                                .map(|d| format!(" — {d}"))
                                .unwrap_or_default();
                            format!("  - {label}{detail}")
                        })
                        .collect();
                    format!("Completions ({} total):\n{}", items.len(), lines.join("\n"))
                }
                _ => "No completions available.".to_string(),
            }
        }
        "definition" => {
            let locs = locations(result);
            if locs.is_empty() {
                "Definition not found.".to_string()
            } else {
                format!("Definitions:\n{}", locs.join("\n"))
            }
        }
        "references" => {
            let locs = locations(result);
            if locs.is_empty() {
                "No references found.".to_string()
            } else {
                let more = locs.len().saturating_sub(80);
                let mut out = format!(
                    "{} reference(s):\n{}",
                    locs.len(),
                    locs.iter().take(80).cloned().collect::<Vec<_>>().join("\n")
                );
                if more > 0 {
                    out.push_str(&format!("\n  ... and {more} more"));
                }
                out
            }
        }
        "rename" => format_rename(result),
        "symbols" => {
            let mut lines = Vec::new();
            flatten_symbols(result, 0, &mut lines);
            if lines.is_empty() {
                "No symbols found.".to_string()
            } else {
                lines.join("\n")
            }
        }
        "workspace_symbols" => {
            let arr = result.as_array().cloned().unwrap_or_default();
            if arr.is_empty() {
                return "No matching symbols.".to_string();
            }
            let lines: Vec<String> = arr
                .iter()
                .take(60)
                .map(|s| {
                    let name = s.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                    let kind = symbol_kind(s.get("kind").and_then(|k| k.as_u64()).unwrap_or(0));
                    let loc = s
                        .get("location")
                        .and_then(format_location)
                        .unwrap_or_default();
                    format!("  {kind} {name} @ {}", loc.trim_start())
                })
                .collect();
            format!("{} symbol(s):\n{}", arr.len(), lines.join("\n"))
        }
        _ => serde_json::to_string_pretty(result).unwrap_or_default(),
    }
}

/// Format an LSP result only after removing resource objects outside the
/// calling task's workspace. This runs on structured JSON, before URI values
/// are rendered into ordinary text by `format_result`.
fn format_result_scoped(action: &str, result: &Value, workspace_root: &Path) -> String {
    let mut scoped = result.clone();
    let mut omitted = 0usize;
    filter_lsp_resources(&mut scoped, workspace_root, &mut omitted);
    let mut output = format_result(action, &scoped);
    if omitted > 0 {
        output.push_str(&format!(
            "\n{omitted} LSP resource location(s) outside the authorized workspace were omitted."
        ));
    }
    output
}

/// Returns false for a resource object whose URI is malformed or not an
/// existing local file within the workspace. Unknown schemes fail closed.
fn filter_lsp_resources(value: &mut Value, workspace_root: &Path, omitted: &mut usize) -> bool {
    match value {
        Value::Array(items) => {
            items.retain_mut(|item| filter_lsp_resources(item, workspace_root, omitted));
            true
        }
        Value::Object(object) => {
            for field in ["uri", "targetUri"] {
                if let Some(uri) = object.get(field) {
                    if !uri
                        .as_str()
                        .is_some_and(|uri| lsp_uri_is_in_workspace(uri, workspace_root))
                    {
                        *omitted += 1;
                        return false;
                    }
                }
            }
            if let Some(changes) = object.get_mut("changes").and_then(Value::as_object_mut) {
                changes.retain(|uri, _| {
                    let allowed = lsp_uri_is_in_workspace(uri, workspace_root);
                    if !allowed {
                        *omitted += 1;
                    }
                    allowed
                });
            }
            let mut keep = true;
            object.retain(|_, child| {
                let child_is_safe = filter_lsp_resources(child, workspace_root, omitted);
                keep &= child_is_safe;
                child_is_safe
            });
            keep
        }
        _ => true,
    }
}

fn lsp_uri_is_in_workspace(uri: &str, workspace_root: &Path) -> bool {
    // Only canonical local file URIs are meaningful to AgentZ's source tools.
    // Reject authority hosts, virtual documents, malformed encodings and paths
    // that no longer resolve instead of displaying a potentially followable path.
    if !uri.starts_with("file:///") {
        return false;
    }
    let decoded = uri_to_path(uri);
    let Ok(root) = std::fs::canonicalize(workspace_root) else {
        return false;
    };
    let Ok(target) = std::fs::canonicalize(PathBuf::from(decoded)) else {
        return false;
    };
    path_is_within_root(&root, &target)
}

#[cfg(windows)]
fn path_is_within_root(root: &Path, target: &Path) -> bool {
    let root_components: Vec<_> = root.components().collect();
    let target_components: Vec<_> = target.components().collect();
    root_components.len() <= target_components.len()
        && root_components
            .iter()
            .zip(target_components.iter())
            .all(|(root, target)| {
                root.as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&target.as_os_str().to_string_lossy())
            })
}

#[cfg(not(windows))]
fn path_is_within_root(root: &Path, target: &Path) -> bool {
    target.starts_with(root)
}

fn locations(result: &Value) -> Vec<String> {
    match result {
        Value::Array(arr) => arr.iter().filter_map(location_or_link).collect(),
        Value::Object(_) => location_or_link(result).into_iter().collect(),
        _ => vec![],
    }
}

fn location_or_link(v: &Value) -> Option<String> {
    if v.get("targetUri").is_some() {
        let uri = v.get("targetUri")?.as_str()?;
        let range = v
            .get("targetSelectionRange")
            .or_else(|| v.get("targetRange"))?;
        return Some(fmt_loc(uri, range));
    }
    format_location(v)
}

fn format_location(loc: &Value) -> Option<String> {
    let uri = loc.get("uri")?.as_str()?;
    Some(fmt_loc(uri, loc.get("range")?))
}

fn fmt_loc(uri: &str, range: &Value) -> String {
    let start = range.get("start");
    let line = start
        .and_then(|s| s.get("line"))
        .and_then(|l| l.as_u64())
        .unwrap_or(0)
        + 1;
    let col = start
        .and_then(|s| s.get("character"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);
    format!("  {}:{}:{}", uri_to_path(uri), line, col)
}

fn format_rename(result: &Value) -> String {
    let mut per_file: Vec<(String, usize)> = Vec::new();
    if let Some(changes) = result.get("changes").and_then(|c| c.as_object()) {
        for (uri, edits) in changes {
            per_file.push((uri_to_path(uri), edits.as_array().map_or(0, |a| a.len())));
        }
    }
    if let Some(docs) = result.get("documentChanges").and_then(|d| d.as_array()) {
        for d in docs {
            if let Some(uri) = d.pointer("/textDocument/uri").and_then(|u| u.as_str()) {
                per_file.push((
                    uri_to_path(uri),
                    d.get("edits")
                        .and_then(|e| e.as_array())
                        .map_or(0, |a| a.len()),
                ));
            }
        }
    }
    if per_file.is_empty() {
        return "Rename produced no edits (symbol not renameable at this position).".to_string();
    }
    let total: usize = per_file.iter().map(|(_, n)| n).sum();
    let mut out = format!(
        "Rename PREVIEW (nothing was modified): {total} edit(s) in {} file(s). Apply them with file_edit.\n",
        per_file.len()
    );
    for (p, n) in per_file {
        out.push_str(&format!("  {p}: {n} edit(s)\n"));
    }
    out
}

fn symbol_kind(k: u64) -> &'static str {
    match k {
        2 => "module",
        5 => "class",
        6 => "method",
        8 => "field",
        9 => "constructor",
        10 => "enum",
        11 => "interface",
        12 => "fn",
        13 => "var",
        14 => "const",
        22 => "enum-member",
        23 => "struct",
        _ => "sym",
    }
}

fn flatten_symbols(v: &Value, depth: usize, out: &mut Vec<String>) {
    let Some(arr) = v.as_array() else { return };
    for s in arr {
        let name = s.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let kind = symbol_kind(s.get("kind").and_then(|k| k.as_u64()).unwrap_or(0));
        let line = s
            .pointer("/selectionRange/start/line")
            .or_else(|| s.pointer("/range/start/line"))
            .or_else(|| s.pointer("/location/range/start/line"))
            .and_then(|l| l.as_u64())
            .unwrap_or(0)
            + 1;
        out.push(format!(
            "{}{kind} {name}  (line {line})",
            "  ".repeat(depth)
        ));
        if let Some(children) = s.get("children") {
            flatten_symbols(children, depth + 1, out);
        }
    }
}

/// Format diagnostics array into text.
fn format_diagnostics(diagnostics: &[Value]) -> String {
    if diagnostics.is_empty() {
        return "No diagnostics found.".to_string();
    }
    let mut lines = vec![format!("{} diagnostic(s):", diagnostics.len())];
    for (i, diag) in diagnostics.iter().enumerate().take(50) {
        let severity = match diag.get("severity").and_then(|s| s.as_u64()).unwrap_or(3) {
            1 => "ERROR",
            2 => "WARNING",
            3 => "INFO",
            4 => "HINT",
            _ => "?",
        };
        let message = diag.get("message").and_then(|m| m.as_str()).unwrap_or("?");
        let start = diag.get("range").and_then(|r| r.get("start"));
        let line = start
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .unwrap_or(0)
            + 1;
        let col = start
            .and_then(|s| s.get("character"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0);
        let source = diag
            .get("source")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| format!(" ({s})"))
            .unwrap_or_default();
        let code = diag
            .get("code")
            .map(|c| {
                format!(
                    " [{}]",
                    c.as_str().map_or_else(|| c.to_string(), str::to_string)
                )
            })
            .unwrap_or_default();
        lines.push(format!(
            "  {}. [{severity}]{source} line {line}:{col} — {message}{code}",
            i + 1
        ));
    }
    if diagnostics.len() > 50 {
        lines.push(format!(
            "  ... and {} more diagnostics",
            diagnostics.len() - 50
        ));
    }
    lines.join("\n")
}

/// Open `file` in the language server and return its diagnostics.
pub(crate) async fn collect_diagnostics_for_file(
    manager: &LspManager,
    file: &str,
    language: &str,
    project_root: &str,
    wait_ms: u64,
) -> Result<Vec<Value>, String> {
    let client = manager.client(project_root, language).await?;
    let lsp_id = LspManager::lsp_language_id(file, language);
    let (uri, changed) = client.sync_file(file, &lsp_id).await?;
    Ok(client
        .diagnostics(&uri, changed, Duration::from_millis(wait_ms))
        .await)
}

pub fn format_diagnostics_pub(diagnostics: &[Value]) -> String {
    format_diagnostics(diagnostics)
}

pub(crate) fn detect_project_root_pub(file: &str) -> String {
    detect_project_root(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::client::path_to_uri;
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static TEST_ROOT_ID: AtomicUsize = AtomicUsize::new(0);

    fn scoped_fixture() -> (PathBuf, PathBuf) {
        let id = TEST_ROOT_ID.fetch_add(1, Ordering::Relaxed);
        let base =
            std::env::temp_dir().join(format!("piscis-lsp-scope-{}-{id}", std::process::id()));
        let root = base.join("workspace");
        let outside = base.join("outside.rs");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("inside.rs"), "inside").unwrap();
        fs::write(&outside, "outside").unwrap();
        (root, outside)
    }

    #[test]
    fn structured_lsp_locations_are_filtered_to_the_authorized_workspace() {
        let (root, outside) = scoped_fixture();
        let inside_uri = path_to_uri(root.join("inside.rs").to_string_lossy().as_ref());
        let outside_uri = path_to_uri(outside.to_string_lossy().as_ref());

        let definition = json!([
            {"uri": inside_uri.clone(), "range": {"start": {"line": 1, "character": 2}}},
            {"uri": outside_uri.clone(), "range": {"start": {"line": 9, "character": 4}}},
            {"uri": "vscode-remote://host/private.rs", "range": {"start": {"line": 0, "character": 0}}}
        ]);
        let rendered = format_result_scoped("definition", &definition, &root);
        assert!(rendered.contains("inside.rs:2:2"), "{rendered}");
        assert!(!rendered.contains("outside.rs"), "{rendered}");
        assert!(!rendered.contains("private.rs"), "{rendered}");
        assert!(
            rendered.contains("2 LSP resource location(s)"),
            "{rendered}"
        );

        let mut changes = serde_json::Map::new();
        changes.insert(inside_uri.clone(), json!([{}]));
        changes.insert(outside_uri.clone(), json!([{}, {}]));
        let rename = json!({"changes": changes});
        let rendered = format_result_scoped("rename", &rename, &root);
        assert!(rendered.contains("1 edit(s) in 1 file(s)"), "{rendered}");
        assert!(rendered.contains("inside.rs: 1 edit(s)"), "{rendered}");
        assert!(!rendered.contains("outside.rs"), "{rendered}");
        assert!(
            rendered.contains("1 LSP resource location(s)"),
            "{rendered}"
        );

        let symbols = json!([{
            "name": "Visible",
            "kind": 12,
            "location": {"uri": inside_uri, "range": {"start": {"line": 0, "character": 0}}}
        }, {
            "name": "External",
            "kind": 12,
            "location": {"uri": outside_uri, "range": {"start": {"line": 0, "character": 0}}}
        }]);
        let rendered = format_result_scoped("workspace_symbols", &symbols, &root);
        assert!(rendered.contains("Visible"), "{rendered}");
        assert!(!rendered.contains("External"), "{rendered}");
        assert!(!rendered.contains("outside.rs"), "{rendered}");

        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }

    #[test]
    fn formats_definition_links_and_locations() {
        let r = json!([{
            "targetUri": "file:///C:/p/src/a.rs",
            "targetRange": {"start": {"line": 9, "character": 0}, "end": {"line": 12, "character": 1}},
            "targetSelectionRange": {"start": {"line": 9, "character": 4}, "end": {"line": 9, "character": 8}}
        }]);
        assert_eq!(
            format_result("definition", &r),
            "Definitions:\n  C:/p/src/a.rs:10:4"
        );
    }

    #[test]
    fn rename_is_labelled_preview() {
        let r = json!({"changes": {"file:///a.rs": [{}, {}], "file:///b.rs": [{}]}});
        let out = format_rename(&r);
        assert!(out.contains("PREVIEW"));
        assert!(out.contains("3 edit(s) in 2 file(s)"));
    }

    #[test]
    fn symbols_are_indented_by_nesting() {
        let r = json!([{"name": "S", "kind": 23, "selectionRange": {"start": {"line": 0, "character": 0}},
                        "children": [{"name": "f", "kind": 6, "selectionRange": {"start": {"line": 2, "character": 0}}}]}]);
        let mut out = vec![];
        flatten_symbols(&r, 0, &mut out);
        assert_eq!(out, vec!["struct S  (line 1)", "  method f  (line 3)"]);
    }
}
