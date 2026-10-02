//! Remote-workspace replacements for the kernel's file and shell tools.
//!
//! When the project dir is an `agentz-remote://` URI the kernel tools would
//! touch the local disk, so these take over under the same names and input
//! schemas and execute through the connected agentz-server instead.

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolRegistry, ToolResult};
use serde_json::{json, Value};

use crate::remote::{self, shell_quote};

const MAX_READ_LINES: usize = 2000;
const MAX_OUTPUT_CHARS: usize = 60_000;

pub const REPLACED_TOOLS: &[&str] = &["file_read", "file_write", "file_edit", "file_list", "file_search", "shell"];

/// Swap the kernel file/shell tools for remote ones when `workspace_root` is
/// a remote URI. Tools that were disabled (plan mode, allowlists) stay absent;
/// local-index tools that can't see a remote disk are dropped.
pub fn install_if_remote(registry: &mut ToolRegistry, workspace_root: &str) {
    if !remote::is_remote(workspace_root) {
        return;
    }
    let present: Vec<&str> = REPLACED_TOOLS
        .iter()
        .copied()
        .filter(|n| registry.get(n).is_some())
        .collect();
    let mirrored: Vec<&str> = MIRRORED_TOOLS
        .iter()
        .copied()
        .filter(|n| registry.get(n).is_some())
        .collect();
    for name in REPLACED_TOOLS.iter().chain(LOCAL_ONLY_TOOLS).chain(MIRRORED_TOOLS) {
        registry.unregister(name);
    }
    for name in mirrored {
        let inner: Box<dyn Tool> = match name {
            "codebase_search" => Box::new(crate::tools::codebase_search::CodebaseSearchTool),
            "graph_search" => Box::new(crate::tools::graph_search::GraphSearchTool),
            "graph_explore" => Box::new(crate::tools::graph_explore::GraphExploreTool),
            "symbol_search" => Box::new(crate::tools::symbols::SymbolSearchTool),
            _ => Box::new(crate::tools::symbols::ImpactTool),
        };
        registry.register(Box::new(MirroredTool { inner }));
    }
    for name in present {
        let tool: Box<dyn Tool> = match name {
            "file_read" => Box::new(RemoteFileRead),
            "file_write" => Box::new(RemoteFileWrite),
            "file_edit" => Box::new(RemoteFileEdit),
            "file_list" => Box::new(RemoteFileList),
            "file_search" => Box::new(RemoteFileSearch),
            _ => Box::new(RemoteShell),
        };
        registry.register(tool);
    }
}

/// Index tools that run against the local mirror (`remote::mirror`).
const MIRRORED_TOOLS: &[&str] = &["codebase_search", "graph_search", "graph_explore", "symbol_search", "impact"];

/// Runs an index tool with `workspace_root` pointed at the synced local
/// mirror; reported paths are workspace-relative, so they hold on the remote.
struct MirroredTool {
    inner: Box<dyn Tool>,
}

#[async_trait]
impl Tool for MirroredTool {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn description(&self) -> &str {
        self.inner.description()
    }
    fn input_schema(&self) -> Value {
        self.inner.input_schema()
    }
    fn is_read_only(&self) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let workspace = ctx.workspace_root.to_string_lossy().to_string();
        let mirror = match remote::mirror::sync(&workspace, false).await {
            Ok(d) => d,
            Err(e) => return Ok(ToolResult::err(format!("remote index unavailable: {e}"))),
        };
        let mut local = ctx.clone();
        local.workspace_root = mirror;
        self.inner.call(input, &local).await
    }
}

/// Tools backed by local language servers / local execution.
const LOCAL_ONLY_TOOLS: &[&str] = &[
    "lsp",
    "read_lints",
    "code_run",
    "file_diff",
];

async fn remote_root(ctx: &ToolContext) -> anyhow::Result<String> {
    remote::resolve(&ctx.workspace_root.to_string_lossy())
        .await
        .map_err(anyhow::Error::msg)?
        .ok_or_else(|| anyhow::anyhow!("workspace is not remote"))
}

fn join(root: &str, p: &str) -> String {
    if p.is_empty() || p == "." {
        root.to_string()
    } else if p.starts_with('/') {
        p.to_string()
    } else if let Some(parsed) = remote::vfs::parse(p) {
        parsed.path
    } else {
        format!("{}/{}", root.trim_end_matches('/'), p.trim_start_matches("./"))
    }
}

async fn resolve_path(ctx: &ToolContext, input: &Value, key: &str) -> anyhow::Result<String> {
    let root = remote_root(ctx).await?;
    Ok(join(&root, input[key].as_str().unwrap_or("")))
}

async fn read_text(path: &str) -> anyhow::Result<String> {
    let v = remote::call("fs.readFile", json!({ "path": path }))
        .await
        .map_err(anyhow::Error::msg)?;
    let bytes = STANDARD.decode(v["base64"].as_str().unwrap_or_default())?;
    String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("{path} is not UTF-8 text; use shell to inspect it"))
}

async fn write_text(path: &str, text: &str) -> anyhow::Result<()> {
    remote::call("fs.writeFile", json!({ "path": path, "base64": STANDARD.encode(text) }))
        .await
        .map(|_| ())
        .map_err(anyhow::Error::msg)
}

struct ExecOut {
    code: i64,
    stdout: String,
    stderr: String,
}

async fn sh(script: &str, cwd: &str, timeout_ms: u64) -> anyhow::Result<ExecOut> {
    let v = remote::call(
        "exec",
        json!({ "command": "sh", "args": ["-c", script], "cwd": cwd, "timeoutMs": timeout_ms }),
    )
    .await
    .map_err(anyhow::Error::msg)?;
    Ok(ExecOut {
        code: v["code"].as_i64().unwrap_or(-1),
        stdout: v["stdout"].as_str().unwrap_or_default().to_string(),
        stderr: v["stderr"].as_str().unwrap_or_default().to_string(),
    })
}

fn truncate(mut s: String) -> String {
    if s.len() > MAX_OUTPUT_CHARS {
        let mut cut = MAX_OUTPUT_CHARS;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push_str("\n… [output truncated]");
    }
    s
}

pub struct RemoteFileRead;

#[async_trait]
impl Tool for RemoteFileRead {
    fn name(&self) -> &str {
        "file_read"
    }
    fn description(&self) -> &str {
        "Read a file from the remote workspace. Returns numbered lines. Relative paths resolve \
         from the workspace root on the remote machine; use offset/limit for large files."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path (relative to the remote workspace root, or absolute on the remote)." },
                "offset": { "type": "integer", "description": "1-indexed line to start from." },
                "limit": { "type": "integer", "description": "Maximum number of lines." }
            },
            "required": ["path"]
        })
    }
    fn is_read_only(&self) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let path = resolve_path(ctx, &input, "path").await?;
        let text = match read_text(&path).await {
            Ok(t) => t,
            Err(e) => return Ok(ToolResult::err(format!("{path}: {e}"))),
        };
        let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = input["limit"].as_u64().map(|n| n as usize).unwrap_or(MAX_READ_LINES);
        let lines: Vec<&str> = text.lines().collect();
        let end = (offset - 1 + limit).min(lines.len());
        let mut out = format!("--- {path} ({} lines) ---\n", lines.len());
        for (i, line) in lines.iter().enumerate().take(end).skip(offset - 1) {
            out.push_str(&format!("{:>6}|{}\n", i + 1, line));
        }
        if end < lines.len() {
            out.push_str(&format!("… {} more lines (use offset={})\n", lines.len() - end, end + 1));
        }
        Ok(ToolResult::ok(truncate(out)))
    }
}

pub struct RemoteFileWrite;

#[async_trait]
impl Tool for RemoteFileWrite {
    fn name(&self) -> &str {
        "file_write"
    }
    fn description(&self) -> &str {
        "Write full content to a file in the remote workspace (creates parents, overwrites). \
         Use file_edit to change only part of a file."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string", "description": "Full content; replaces the entire file." }
            },
            "required": ["path", "content"]
        })
    }
    fn needs_confirmation(&self, _input: &Value) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let Some(content) = input["content"].as_str() else {
            return Ok(ToolResult::err("Missing required parameter: content"));
        };
        let path = resolve_path(ctx, &input, "path").await?;
        write_text(&path, content).await?;
        Ok(ToolResult::ok(format!("Wrote {} bytes to {path}", content.len())))
    }
}

pub struct RemoteFileEdit;

#[async_trait]
impl Tool for RemoteFileEdit {
    fn name(&self) -> &str {
        "file_edit"
    }
    fn description(&self) -> &str {
        "Edit a file in the remote workspace by exact string replacement. Single mode: \
         {old_string,new_string}. Batch mode: {edits:[...]} applied atomically. Each old_string \
         must appear exactly once."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "old_string": { "type": "string" },
                "new_string": { "type": "string" },
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_string": { "type": "string" },
                            "new_string": { "type": "string" }
                        },
                        "required": ["old_string", "new_string"]
                    }
                }
            },
            "required": ["path"]
        })
    }
    fn needs_confirmation(&self, _input: &Value) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let path = resolve_path(ctx, &input, "path").await?;
        let mut edits: Vec<(String, String)> = Vec::new();
        if let Some(arr) = input["edits"].as_array() {
            for e in arr {
                edits.push((
                    e["old_string"].as_str().unwrap_or_default().to_string(),
                    e["new_string"].as_str().unwrap_or_default().to_string(),
                ));
            }
        } else if let (Some(o), Some(n)) = (input["old_string"].as_str(), input["new_string"].as_str()) {
            edits.push((o.to_string(), n.to_string()));
        }
        if edits.is_empty() {
            return Ok(ToolResult::err("Provide old_string/new_string or edits[]"));
        }
        let mut text = match read_text(&path).await {
            Ok(t) => t,
            Err(e) => return Ok(ToolResult::err(format!("{path}: {e}"))),
        };
        for (i, (old, new)) in edits.iter().enumerate() {
            match text.matches(old.as_str()).count() {
                1 => text = text.replacen(old.as_str(), new, 1),
                0 => return Ok(ToolResult::err(format!("edit {}: old_string not found in {path}", i + 1))),
                n => return Ok(ToolResult::err(format!("edit {}: old_string appears {n} times in {path}; add context", i + 1))),
            }
        }
        write_text(&path, &text).await?;
        Ok(ToolResult::ok(format!("Applied {} edit(s) to {path}", edits.len())))
    }
}

pub struct RemoteFileList;

#[async_trait]
impl Tool for RemoteFileList {
    fn name(&self) -> &str {
        "file_list"
    }
    fn description(&self) -> &str {
        "List a directory in the remote workspace. Relative paths resolve from the remote \
         workspace root; set recursive=true (max_depth default 3) to descend."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "recursive": { "type": "boolean" },
                "max_depth": { "type": "integer" },
                "include_hidden": { "type": "boolean" },
                "dirs_only": { "type": "boolean" }
            },
            "required": ["path"]
        })
    }
    fn is_read_only(&self) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let path = resolve_path(ctx, &input, "path").await?;
        let depth = if input["recursive"].as_bool().unwrap_or(false) {
            input["max_depth"].as_u64().unwrap_or(3).clamp(1, 10)
        } else {
            1
        };
        let mut script = format!(
            "find {} -mindepth 1 -maxdepth {depth} -not -path '*/.git/*' -not -path '*/node_modules/*'",
            shell_quote(&path)
        );
        if !input["include_hidden"].as_bool().unwrap_or(false) {
            script.push_str(" -not -name '.*' -not -path '*/.*/*'");
        }
        if input["dirs_only"].as_bool().unwrap_or(false) {
            script.push_str(" -type d");
        }
        script.push_str(&format!(
            " \\( -type d -exec printf '%s/\\n' {{}} + -o -print \\) 2>/dev/null | sed \"s|^{}/||\" | sort | head -n 2000",
            path.trim_end_matches('/').replace('|', "\\|")
        ));
        let out = sh(&script, &path, 60_000).await?;
        if out.code != 0 && out.stdout.is_empty() {
            return Ok(ToolResult::err(format!("{path}: {}", out.stderr.trim())));
        }
        Ok(ToolResult::ok(truncate(format!("--- {path} ---\n{}", out.stdout))))
    }
}

pub struct RemoteFileSearch;

#[async_trait]
impl Tool for RemoteFileSearch {
    fn name(&self) -> &str {
        "file_search"
    }
    fn description(&self) -> &str {
        "Search the remote workspace. action='glob' finds files by name pattern; action='grep' \
         searches contents by regex (ripgrep if installed on the remote, else grep)."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["glob", "grep"] },
                "pattern": { "type": "string" },
                "path": { "type": "string" },
                "include": { "type": "string" },
                "file_extensions": { "type": "array", "items": { "type": "string" } },
                "max_results": { "type": "integer" },
                "case_sensitive": { "type": "boolean" }
            },
            "required": ["action", "pattern"]
        })
    }
    fn is_read_only(&self) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let root = resolve_path(ctx, &input, "path").await?;
        let pattern = input["pattern"].as_str().unwrap_or_default();
        let max = input["max_results"].as_u64().unwrap_or(50).clamp(1, 500) as usize;
        let mut globs: Vec<String> = input["file_extensions"]
            .as_array()
            .map(|a| a.iter().filter_map(|e| e.as_str()).map(|e| format!("*.{}", e.trim_start_matches('.'))).collect())
            .unwrap_or_default();
        if let Some(inc) = input["include"].as_str().filter(|s| !s.is_empty()) {
            globs.push(inc.to_string());
        }

        if input["action"].as_str() == Some("glob") {
            let name = pattern.rsplit('/').next().unwrap_or(pattern);
            let script = format!(
                "find . -not -path '*/.git/*' -not -path '*/node_modules/*' -name {} -print 2>/dev/null | sed 's|^\\./||' | head -n {max}",
                shell_quote(name)
            );
            let out = sh(&script, &root, 60_000).await?;
            return Ok(ToolResult::ok(if out.stdout.trim().is_empty() {
                format!("No files matching {pattern} under {root}")
            } else {
                truncate(out.stdout)
            }));
        }

        let v = remote::call(
            "search",
            json!({
                "root": root,
                "query": pattern,
                "filePattern": if globs.is_empty() { Value::Null } else { Value::String(globs.join(",")) },
                "caseSensitive": input["case_sensitive"].as_bool().unwrap_or(false),
                "useRegex": true,
                "maxResults": max,
            }),
        )
        .await
        .map_err(anyhow::Error::msg)?;
        let hits = v.as_array().cloned().unwrap_or_default();
        if hits.is_empty() {
            return Ok(ToolResult::ok(format!("No matches for /{pattern}/ under {root}")));
        }
        let mut out = String::new();
        for h in &hits {
            out.push_str(&format!(
                "{}:{}: {}\n",
                h["path"].as_str().unwrap_or_default(),
                h["line"].as_u64().unwrap_or(0),
                h["text"].as_str().unwrap_or_default()
            ));
        }
        Ok(ToolResult::ok(truncate(out)))
    }
}

pub struct RemoteShell;

#[async_trait]
impl Tool for RemoteShell {
    fn name(&self) -> &str {
        "shell"
    }
    fn description(&self) -> &str {
        "Execute a POSIX shell command (sh -c) on the remote machine that hosts the workspace. \
         Working directory defaults to the remote workspace root."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "cwd": { "type": "string", "description": "Working directory (relative to the remote workspace root, or absolute)." },
                "timeout": { "type": "integer", "description": "Timeout in seconds (default 120)." },
                "env": { "type": "object", "description": "Extra environment variables." }
            },
            "required": ["command"]
        })
    }
    fn needs_confirmation(&self, _input: &Value) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let Some(command) = input["command"].as_str() else {
            return Ok(ToolResult::err("Missing required parameter: command"));
        };
        let cwd = resolve_path(ctx, &input, "cwd").await?;
        let mut script = String::new();
        if let Some(env) = input["env"].as_object() {
            for (k, v) in env {
                if k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    script.push_str(&format!("export {k}={}; ", shell_quote(v.as_str().unwrap_or_default())));
                }
            }
        }
        script.push_str(command);
        let timeout = input["timeout"].as_u64().unwrap_or(120).clamp(1, 3600) * 1000;
        let out = sh(&script, &cwd, timeout).await?;
        let mut text = out.stdout;
        if !out.stderr.trim().is_empty() {
            text.push_str("\n[stderr]\n");
            text.push_str(&out.stderr);
        }
        text.push_str(&format!("\n[exit code: {}]", out.code));
        let text = truncate(text);
        Ok(if out.code == 0 { ToolResult::ok(text) } else { ToolResult::err(text) })
    }
}
