//! Remote-workspace versions of `file_diff`, `code_run`, `read_lints` and `lsp`.
//!
//! Same names and input schemas as the local tools. Files are read through
//! agentz-server, commands run on the remote host, and language servers are
//! spawned there through the target's transport, so results describe the
//! remote tree rather than the local source mirror.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use piscis_ide_tools::lsp::client::{path_to_uri, LspClient};
use piscis_ide_tools::lsp::manager::LspManager;
use piscis_ide_tools::tools::lsp::{
    action_request, format_diagnostics_pub, format_result, request_with_retry,
};
use piscis_ide_tools::tools::read_lints::{checker_file_matches, parse_checker_output};
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use piscis_kernel::tools::file_diff::unified_diff;
use serde_json::{json, Value};

use super::remote_fs::{read_text, remote_root, resolve_path, sh, truncate};
use crate::remote::{self, shell_quote};

const MAX_DIFF_BYTES: usize = 500 * 1024;
const MAX_DIFF_LINES: usize = 5000;
const ROOT_MARKERS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "tsconfig.json",
    "pyproject.toml",
    "setup.py",
    "CMakeLists.txt",
    "Makefile",
];

// ─── file_diff ─────────────────────────────────────────────────────────────

pub struct RemoteFileDiff;

#[async_trait]
impl Tool for RemoteFileDiff {
    fn name(&self) -> &str {
        "file_diff"
    }
    fn description(&self) -> &str {
        "Preview changes before writing, or compare two files in the remote workspace. \
         Read-only. Preview mode: `path` + `new_content`. Compare mode: `path_a` + `path_b`. \
         Output is a unified diff."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Preview mode: existing remote file." },
                "new_content": { "type": "string", "description": "Preview mode: proposed content." },
                "path_a": { "type": "string", "description": "Compare mode: first file (---)." },
                "path_b": { "type": "string", "description": "Compare mode: second file (+++)." },
                "context_lines": { "type": "integer", "description": "Context lines (default 3)." }
            }
        })
    }
    fn is_read_only(&self) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let context = input["context_lines"].as_u64().unwrap_or(3) as usize;
        let (label_a, a, label_b, b) = if input["path"].is_string()
            && input["new_content"].is_string()
        {
            let path = resolve_path(ctx, &input, "path").await?;
            let current = match read_text(&path).await {
                Ok(t) => t,
                Err(e) => return Ok(ToolResult::err(format!("{path}: {e}"))),
            };
            let proposed = input["new_content"].as_str().unwrap_or_default().to_string();
            (
                format!("{path} (current)"),
                current,
                format!("{path} (proposed)"),
                proposed,
            )
        } else if input["path_a"].is_string() && input["path_b"].is_string() {
            let pa = resolve_path(ctx, &input, "path_a").await?;
            let pb = resolve_path(ctx, &input, "path_b").await?;
            let a = match read_text(&pa).await {
                Ok(t) => t,
                Err(e) => return Ok(ToolResult::err(format!("path_a {pa}: {e}"))),
            };
            let b = match read_text(&pb).await {
                Ok(t) => t,
                Err(e) => return Ok(ToolResult::err(format!("path_b {pb}: {e}"))),
            };
            (pa, a, pb, b)
        } else {
            return Ok(ToolResult::err(
                "Provide either (path + new_content) for preview mode, \
                 or (path_a + path_b) for compare mode",
            ));
        };
        if a.len() + b.len() > MAX_DIFF_BYTES || a.lines().count() + b.lines().count() > MAX_DIFF_LINES
        {
            return Ok(ToolResult::err(
                "Files too large for in-memory diff; use shell with `git diff` or `diff -u`.",
            ));
        }
        let diff = unified_diff(&a, &b, &label_a, &label_b, context);
        Ok(ToolResult::ok(if diff.is_empty() {
            "No differences — files are identical.".to_string()
        } else {
            truncate(diff)
        }))
    }
}

// ─── code_run ──────────────────────────────────────────────────────────────

pub struct RemoteCodeRun;

#[async_trait]
impl Tool for RemoteCodeRun {
    fn name(&self) -> &str {
        "code_run"
    }
    fn description(&self) -> &str {
        "Run a build / test / script command on the remote machine that hosts the workspace \
         (sh -c, cwd defaults to the remote workspace root; per-user tool dirs such as \
         ~/.cargo/bin and node_modules/.bin are on PATH). Returns exit_code, duration_ms, \
         stdout and stderr. Background mode is not available for remote workspaces: for \
         servers, use shell with nohup and poll the log."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Command to run (e.g. \"cargo test\")." },
                "cwd": { "type": "string", "description": "Working directory (relative to the remote workspace root, or absolute)." },
                "timeout_secs": { "type": "integer", "description": "Timeout in seconds (default 60, max 300)." },
                "env": { "type": "object", "description": "Extra environment variables." }
            },
            "required": ["command"]
        })
    }
    fn needs_confirmation(&self, _input: &Value) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let Some(command) = input["command"].as_str().filter(|c| !c.trim().is_empty()) else {
            return Ok(ToolResult::err("Missing required parameter: command"));
        };
        if input["background"].as_bool().unwrap_or(false) {
            return Ok(ToolResult::err(
                "background=true is not available for remote workspaces; run it with shell \
                 (`nohup <cmd> > /tmp/<name>.log 2>&1 &`) and read the log.",
            ));
        }
        let cwd = resolve_path(ctx, &input, "cwd").await?;
        let timeout = input["timeout_secs"].as_u64().unwrap_or(60).clamp(1, 300);
        let mut script = format!("{}; ", remote::TOOL_PATH_SETUP);
        if let Some(env) = input["env"].as_object() {
            for (k, v) in env {
                if k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    script.push_str(&format!(
                        "export {k}={}; ",
                        shell_quote(v.as_str().unwrap_or_default())
                    ));
                }
            }
        }
        script.push_str(command);
        let started = Instant::now();
        let out = sh(&script, &cwd, timeout * 1000).await?;
        let text = truncate(format!(
            "exit_code: {}\nduration_ms: {}\ncwd: {cwd}\n[stdout]\n{}\n[stderr]\n{}",
            out.code,
            started.elapsed().as_millis(),
            out.stdout,
            out.stderr
        ));
        Ok(if out.code == 0 {
            ToolResult::ok(text)
        } else {
            ToolResult::err(text)
        })
    }
}

// ─── language servers on the remote ────────────────────────────────────────

fn clients() -> &'static Mutex<HashMap<String, Arc<LspClient>>> {
    static C: OnceLock<Mutex<HashMap<String, Arc<LspClient>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Nearest ancestor of `file` holding a project marker, bounded by the
/// workspace root (which is returned when nothing closer is found).
async fn project_root(file: &str, workspace: &str) -> anyhow::Result<String> {
    let markers = ROOT_MARKERS
        .iter()
        .map(|m| shell_quote(m))
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        "W={w}; d=$(dirname {f}); \
         while :; do for m in {markers}; do [ -e \"$d/$m\" ] && {{ echo \"$d\"; exit 0; }}; done; \
         [ \"$d\" = \"$W\" ] || [ \"$d\" = / ] && break; d=$(dirname \"$d\"); done; echo \"$W\"",
        w = shell_quote(workspace.trim_end_matches('/')),
        f = shell_quote(file),
    );
    let out = sh(&script, workspace, 15_000).await?;
    let root = out.stdout.trim();
    Ok(if root.is_empty() {
        workspace.to_string()
    } else {
        root.to_string()
    })
}

/// Persistent client for `language` rooted at `root` on the bound remote.
async fn remote_client(root: &str, language: &str) -> Result<Arc<LspClient>, String> {
    let session = remote::bound_session().ok_or("remote session is not bound")?;
    let key = format!(
        "{}|{}|{root}|{language}",
        session.generation,
        session.authority.to_ascii_lowercase()
    );
    if let Some(c) = clients().lock().unwrap().get(&key) {
        if c.is_alive() {
            return Ok(c.clone());
        }
    }
    let lang = LspManager::supported_languages()
        .into_iter()
        .find(|l| l.language_id == language)
        .ok_or_else(|| format!("unsupported language: {language}"))?;
    let script = remote::lsp::server_script(root, &lang.server_command, &lang.server_args);
    let child = session
        .target
        .sh(&script)?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("{}: failed to spawn language server: {e}", session.authority))?;
    let client = LspClient::attach(child, root, language, None)
        .await
        .map_err(|e| {
            if e.contains("AGENTZ_LSP_MISSING") {
                format!(
                    "language server '{}' is not installed on {} — {}",
                    lang.server_command,
                    session.authority,
                    LspManager::install_hint(language)
                )
            } else {
                e
            }
        })?;
    clients().lock().unwrap().insert(key, client.clone());
    Ok(client)
}

/// Open/refresh `file` (a remote path) in `client`; returns its URI.
async fn sync_remote_file(
    client: &LspClient,
    file: &str,
    language: &str,
) -> Result<(String, bool), String> {
    let text = read_text(file).await.map_err(|e| e.to_string())?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("file too large (>4MB)".into());
    }
    let lsp_id = LspManager::lsp_language_id(file, language);
    client.sync_text(&path_to_uri(file), text, &lsp_id).await
}

/// Drop resources outside the remote workspace (mirrors the local tool's
/// workspace scoping, by URI prefix since the remote disk is not local).
fn scope_to_root(value: &mut Value, root_uri: &str, omitted: &mut usize) -> bool {
    let inside = |uri: &str| uri == root_uri || uri.starts_with(&format!("{root_uri}/"));
    match value {
        Value::Array(items) => {
            items.retain_mut(|item| scope_to_root(item, root_uri, omitted));
            true
        }
        Value::Object(object) => {
            for field in ["uri", "targetUri"] {
                if let Some(uri) = object.get(field) {
                    if !uri.as_str().is_some_and(inside) {
                        *omitted += 1;
                        return false;
                    }
                }
            }
            if let Some(changes) = object.get_mut("changes").and_then(Value::as_object_mut) {
                changes.retain(|uri, _| {
                    let ok = inside(uri);
                    if !ok {
                        *omitted += 1;
                    }
                    ok
                });
            }
            let mut keep = true;
            object.retain(|_, child| {
                let ok = scope_to_root(child, root_uri, omitted);
                keep &= ok;
                ok
            });
            keep
        }
        _ => true,
    }
}

pub struct RemoteLsp;

#[async_trait]
impl Tool for RemoteLsp {
    fn name(&self) -> &str {
        "lsp"
    }
    fn description(&self) -> &str {
        "Semantic code navigation through a language server running on the remote host \
         (rust-analyzer, typescript-language-server, pyright, clangd). Actions: definition, \
         references, hover, diagnostics, symbols, workspace_symbols (needs 'query'), complete, \
         rename (preview only; needs 'new_name'). 'line' is 1-based, 'character' 0-based. \
         The first call per language starts the server and can take several seconds. If the \
         server is not installed on the remote the error says how to install it."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["definition", "references", "hover", "diagnostics", "symbols",
                             "workspace_symbols", "complete", "rename"]
                },
                "file": { "type": "string", "description": "Remote file (relative to the workspace root, or absolute)." },
                "line": { "type": "integer" },
                "character": { "type": "integer" },
                "new_name": { "type": "string" },
                "query": { "type": "string" }
            },
            "required": ["action", "file"]
        })
    }
    fn is_read_only(&self) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = input["action"].as_str().unwrap_or("diagnostics").to_string();
        let position_actions = ["definition", "references", "hover", "complete", "rename"];
        if position_actions.contains(&action.as_str())
            && (input.get("line").is_none() || input.get("character").is_none())
        {
            return Ok(ToolResult::err(format!(
                "'{action}' needs both 'line' (1-based) and 'character' (0-based)"
            )));
        }
        let file = resolve_path(ctx, &input, "file").await?;
        let Some(language) = LspManager::language_for_extension(&file) else {
            return Ok(ToolResult::err(format!(
                "no language server is mapped to this file type ({file})"
            )));
        };
        let workspace = remote_root(ctx).await?;
        let root = project_root(&file, &workspace).await?;
        let client = match remote_client(&root, &language).await {
            Ok(c) => c,
            Err(e) => return Ok(ToolResult::err(format!("LSP unavailable for {language}: {e}"))),
        };
        let (uri, changed) = match sync_remote_file(&client, &file, &language).await {
            Ok(v) => v,
            Err(e) => return Ok(ToolResult::err(format!("{file}: {e}"))),
        };
        if action == "diagnostics" {
            let diags = client.diagnostics(&uri, changed, Duration::from_secs(8)).await;
            return Ok(ToolResult::ok(format_diagnostics_pub(&diags)));
        }
        let (method, params) = match action_request(&action, &uri, &input) {
            Ok(v) => v,
            Err(e) => return Ok(ToolResult::err(e)),
        };
        match request_with_retry(&client, &action, method, params, changed).await {
            Ok(mut result) => {
                let mut omitted = 0;
                scope_to_root(&mut result, &path_to_uri(&workspace), &mut omitted);
                let mut text = format_result(&action, &result);
                if omitted > 0 {
                    text.push_str(&format!(
                        "\n{omitted} location(s) outside the remote workspace were omitted."
                    ));
                }
                Ok(ToolResult::ok(text))
            }
            Err(e) => Ok(ToolResult::err(e)),
        }
    }
}

// ─── read_lints ────────────────────────────────────────────────────────────

fn checker_command(language: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match language {
        "rust" => Some((
            "cargo",
            "Cargo.toml",
            "cargo check --message-format=json --quiet --all-targets",
        )),
        "typescript" => Some(("tsc", "tsconfig.json", "tsc --noEmit --pretty false")),
        _ => None,
    }
}

/// Project checker on the remote; `None` when the project has no config for it.
async fn run_remote_checker(
    language: &str,
    root: &str,
) -> Option<Result<Vec<(String, Value)>, String>> {
    let (kind, marker, cmd) = checker_command(language)?;
    let probe = sh(
        &format!("test -f {}", shell_quote(&format!("{root}/{marker}"))),
        root,
        10_000,
    )
    .await
    .ok()?;
    if probe.code != 0 {
        return None;
    }
    let script = format!("{}; {cmd}", remote::TOOL_PATH_SETUP);
    Some(match sh(&script, root, 180_000).await {
        Ok(out) if out.code == 127 => Err(format!("`{kind}` is not installed on the remote")),
        Ok(out) => Ok(parse_checker_output(kind, &out.stdout)),
        Err(e) => Err(e.to_string()),
    })
}

pub struct RemoteReadLints;

#[async_trait]
impl Tool for RemoteReadLints {
    fn name(&self) -> &str {
        "read_lints"
    }
    fn description(&self) -> &str {
        "Read compiler / type-checker diagnostics for files in the remote workspace. Uses the \
         language server on the remote host when available, otherwise the project's own \
         checker there (`cargo check` for Rust, `tsc --noEmit` for TypeScript). Parameters: \
         'paths' (string[], relative to the workspace root or absolute), 'severity' \
         ('error' | 'warning' | 'all', default 'warning'), 'wait_ms' (default 1500)."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "paths": { "type": "array", "items": { "type": "string" } },
                "severity": { "type": "string", "enum": ["error", "warning", "all"] },
                "wait_ms": { "type": "integer" }
            },
            "required": ["paths"]
        })
    }
    fn is_read_only(&self) -> bool {
        true
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let raw: Vec<String> = input["paths"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        if raw.is_empty() {
            return Ok(ToolResult::err("'paths' must contain at least one path"));
        }
        let floor = match input["severity"].as_str().unwrap_or("warning") {
            "error" => 1,
            "all" => 4,
            _ => 2,
        };
        let wait = Duration::from_millis(input["wait_ms"].as_u64().unwrap_or(1500).clamp(200, 15_000));
        let workspace = remote_root(ctx).await?;

        let mut checker_cache: HashMap<(String, String), Result<Vec<(String, Value)>, String>> =
            HashMap::new();
        let (mut errors, mut warnings) = (0usize, 0usize);
        let mut sections = Vec::new();
        for p in &raw {
            let file = resolve_path(ctx, &json!({ "path": p }), "path").await?;
            let Some(language) = LspManager::language_for_extension(&file) else {
                sections.push(format!(
                    "── {file} ──\n  (no language server or project checker is mapped to this file type)"
                ));
                continue;
            };
            let root = project_root(&file, &workspace).await?;

            let mut source = "remote language server".to_string();
            let mut note = String::new();
            let mut diags: Option<Vec<Value>> = match remote_client(&root, &language).await {
                Ok(client) => match sync_remote_file(&client, &file, &language).await {
                    Ok((uri, changed)) => Some(client.diagnostics(&uri, changed, wait).await),
                    Err(e) => {
                        note = e;
                        None
                    }
                },
                Err(e) => {
                    note = format!("language server: {e}");
                    None
                }
            };
            if diags.is_none() {
                let key = (root.clone(), language.clone());
                if !checker_cache.contains_key(&key) {
                    if let Some(r) = run_remote_checker(&language, &root).await {
                        checker_cache.insert(key.clone(), r);
                    }
                }
                match checker_cache.get(&key) {
                    Some(Ok(all)) => {
                        source = format!("remote {}", checker_command(&language).map(|c| c.0).unwrap_or("checker"));
                        diags = Some(
                            all.iter()
                                .filter(|(f, _)| checker_file_matches(f, &file))
                                .map(|(_, v)| v.clone())
                                .collect(),
                        );
                    }
                    Some(Err(e)) => note = format!("{note}; checker: {e}"),
                    None => note = format!("{note}; no project checker for {language}"),
                }
            }
            let Some(diags) = diags else {
                sections.push(format!("── {file} ──\n  (could not check — {note})"));
                continue;
            };
            let kept: Vec<Value> = diags
                .into_iter()
                .filter(|d| d["severity"].as_u64().unwrap_or(3) <= floor)
                .collect();
            for d in &kept {
                match d["severity"].as_u64().unwrap_or(3) {
                    1 => errors += 1,
                    2 => warnings += 1,
                    _ => {}
                }
            }
            let body = if kept.is_empty() {
                "  No diagnostics found.".to_string()
            } else {
                format_diagnostics_pub(&kept)
                    .lines()
                    .map(|l| format!("  {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            sections.push(format!("── {file} ── (via {source})\n{body}"));
        }
        Ok(ToolResult::ok(truncate(format!(
            "read_lints: {} file(s) checked — {errors} error(s), {warnings} warning(s)\n\n{}",
            raw.len(),
            sections.join("\n\n")
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::scope_to_root;
    use serde_json::json;

    #[test]
    fn lsp_results_are_scoped_to_the_remote_workspace() {
        let mut v = json!([
            { "uri": "file:///work/src/a.rs", "range": {} },
            { "uri": "file:///work-other/b.rs", "range": {} },
            { "uri": "file:///usr/lib/rust/core.rs", "range": {} }
        ]);
        let mut omitted = 0;
        scope_to_root(&mut v, "file:///work", &mut omitted);
        assert_eq!(omitted, 2);
        assert_eq!(v.as_array().unwrap().len(), 1);

        let mut edit = json!({ "changes": { "file:///work/a.rs": [], "file:///etc/x": [] } });
        let mut omitted = 0;
        scope_to_root(&mut edit, "file:///work", &mut omitted);
        assert_eq!(omitted, 1);
        assert!(edit["changes"].get("file:///work/a.rs").is_some());
    }
}
