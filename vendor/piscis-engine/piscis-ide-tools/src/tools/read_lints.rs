//! `read_lints` — agent tool returning diagnostics for one or more files.
//!
//! Primary source is the language server. When no server is installed or it
//! fails, the project's own checker is run instead (`cargo check` for Rust,
//! `tsc --noEmit` for TS/JS) so the tool never silently returns nothing.

use async_trait::async_trait;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

use crate::lsp::manager::LspManager;
use crate::tools::lsp::{
    collect_diagnostics_for_file, detect_project_root_pub, format_diagnostics_pub,
};

pub struct ReadLintsTool {
    pub lsp_manager: Arc<LspManager>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Checker {
    Cargo,
    Tsc,
}

impl Checker {
    fn label(self) -> &'static str {
        match self {
            Checker::Cargo => "cargo check",
            Checker::Tsc => "tsc --noEmit",
        }
    }
}

/// Diagnostics produced by a project checker, keyed later by file path.
struct CheckerDiag {
    file: String,
    value: Value,
}

#[async_trait]
impl Tool for ReadLintsTool {
    fn name(&self) -> &str {
        "read_lints"
    }

    fn description(&self) -> &str {
        "Read compiler / type-checker diagnostics for one or more source files. \
         Uses the language server when available and otherwise falls back to the \
         project's own checker (`cargo check` for Rust, `tsc --noEmit` for TypeScript/\
         JavaScript), so a result is always real — it is labelled with its source.\n\
         \n\
         WHEN TO USE: right after editing code, before continuing. Cheaper than a full \
         build. Do not call it on every read.\n\
         \n\
         Parameters:\n\
         - 'paths' (string[]): absolute file paths to check.\n\
         - 'severity' ('error' | 'warning' | 'all'): default 'warning' (errors + warnings).\n\
         - 'wait_ms' (number): per-file wait for the language server. Default 1500.\n\
         \n\
         Output: per file `[SEVERITY] line:col — message`, or 'No diagnostics found.'. \
         Files of a language with neither a server nor a checker say so explicitly."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Absolute paths of files to check."
                },
                "severity": {
                    "type": "string",
                    "enum": ["error", "warning", "all"],
                    "description": "Minimum severity to include. Default 'warning'."
                },
                "wait_ms": {
                    "type": "integer",
                    "description": "Per-file wait for language-server diagnostics (ms). Default 1500."
                }
            },
            "required": ["paths"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let paths: Vec<String> = match input.get("paths").and_then(|p| p.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            None => {
                return Ok(ToolResult::err(
                    "'paths' parameter is required (array of absolute file paths)",
                ));
            }
        };
        if paths.is_empty() {
            return Ok(ToolResult::err("'paths' must contain at least one path"));
        }

        let min_severity = severity_floor(
            input.get("severity").and_then(|s| s.as_str()).unwrap_or("warning"),
        );
        let wait_ms = input
            .get("wait_ms")
            .and_then(|w| w.as_u64())
            .unwrap_or(1500)
            .clamp(200, 15_000);

        // One checker run per (root, checker), shared by all files in the call.
        let mut checker_cache: HashMap<(String, Checker), Result<Vec<CheckerDiag>, String>> =
            HashMap::new();
        let mut sections: Vec<String> = Vec::new();
        let (mut total_errors, mut total_warnings) = (0usize, 0usize);

        for file in &paths {
            let Some(language) = LspManager::language_for_extension(file) else {
                sections.push(format!(
                    "── {file} ──\n  (no language server or project checker is mapped to this file type)"
                ));
                continue;
            };
            let root = detect_project_root_pub(file);

            let mut source = "language server".to_string();
            let mut diags: Option<Vec<Value>> =
                match collect_diagnostics_for_file(&self.lsp_manager, file, &language, &root, wait_ms)
                    .await
                {
                    Ok(d) => Some(d),
                    Err(e) => {
                        warn!("read_lints: LSP unavailable for {file}: {e}");
                        None
                    }
                };

            let mut note = String::new();
            if diags.is_none() {
                let lsp_err = self
                    .lsp_manager
                    .client(&root, &language)
                    .await
                    .err()
                    .unwrap_or_else(|| "language server query failed".to_string());
                match checker_for(&language, &root) {
                    Some(checker) => {
                        let entry = checker_cache
                            .entry((root.clone(), checker))
                            .or_insert_with(|| Err(String::new()));
                        if entry.as_ref().err().is_some_and(|e| e.is_empty()) {
                            *entry = run_checker(checker, &root).await;
                        }
                        match entry {
                            Ok(all) => {
                                source = checker.label().to_string();
                                diags = Some(
                                    all.iter()
                                        .filter(|d| same_file(&d.file, file))
                                        .map(|d| d.value.clone())
                                        .collect(),
                                );
                            }
                            Err(e) => {
                                note = format!("language server: {lsp_err}; {}: {e}", checker.label());
                            }
                        }
                    }
                    None => note = format!("language server: {lsp_err}; no project checker for {language}"),
                }
            }

            let Some(diags) = diags else {
                sections.push(format!("── {file} ──\n  (could not check — {note})"));
                continue;
            };

            let filtered: Vec<Value> = diags
                .into_iter()
                .filter(|d| d.get("severity").and_then(|s| s.as_u64()).unwrap_or(3) <= min_severity)
                .collect();
            for d in &filtered {
                match d.get("severity").and_then(|s| s.as_u64()).unwrap_or(3) {
                    1 => total_errors += 1,
                    2 => total_warnings += 1,
                    _ => {}
                }
            }
            let body = if filtered.is_empty() {
                "  No diagnostics found.".to_string()
            } else {
                format_diagnostics_pub(&filtered)
                    .lines()
                    .map(|l| format!("  {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            sections.push(format!("── {file} ── (via {source})\n{body}"));
        }

        Ok(ToolResult::ok(format!(
            "read_lints: {} file(s) checked — {} error(s), {} warning(s)\n\n{}",
            paths.len(),
            total_errors,
            total_warnings,
            sections.join("\n\n")
        )))
    }
}

fn severity_floor(name: &str) -> u64 {
    match name.to_ascii_lowercase().as_str() {
        "error" => 1,
        "warning" => 2,
        "all" | "info" | "hint" => 4,
        _ => 2,
    }
}

fn checker_for(language: &str, root: &str) -> Option<Checker> {
    let root = std::path::Path::new(root);
    match language {
        "rust" if root.join("Cargo.toml").is_file() => Some(Checker::Cargo),
        "typescript" if root.join("tsconfig.json").is_file() => Some(Checker::Tsc),
        _ => None,
    }
}

fn norm(p: &str) -> String {
    p.replace('\\', "/").to_lowercase()
}

/// `checker_file` is usually relative to the project root; `file` is absolute.
fn same_file(checker_file: &str, file: &str) -> bool {
    let c = norm(checker_file);
    let c = c.trim_start_matches("./");
    norm(file).ends_with(c)
}

async fn run_checker(checker: Checker, root: &str) -> Result<Vec<CheckerDiag>, String> {
    let mut cmd = match checker {
        Checker::Cargo => {
            let mut c = piscis_kernel::proc::tokio_command("cargo");
            c.args(["check", "--message-format=json", "--quiet", "--all-targets"]);
            c
        }
        Checker::Tsc => {
            let bin = LspManager::resolve_command("tsc", root)
                .ok_or("`tsc` not found (install typescript in the project)")?;
            let mut c = piscis_kernel::proc::tokio_command(&bin);
            c.args(["--noEmit", "--pretty", "false"]);
            c
        }
    };
    cmd.current_dir(root).kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(180), cmd.output())
        .await
        .map_err(|_| "timed out after 180s".to_string())?
        .map_err(|e| format!("failed to run: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(match checker {
        Checker::Cargo => parse_cargo(&stdout),
        Checker::Tsc => parse_tsc(&stdout),
    })
}

fn parse_cargo(stdout: &str) -> Vec<CheckerDiag> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }
        let Some(msg) = v.get("message") else { continue };
        let level = msg.get("level").and_then(|l| l.as_str()).unwrap_or("");
        let severity = match level {
            "error" | "error: internal compiler error" => 1,
            "warning" => 2,
            _ => continue,
        };
        let Some(spans) = msg.get("spans").and_then(|s| s.as_array()) else { continue };
        let Some(span) = spans
            .iter()
            .find(|s| s.get("is_primary").and_then(|p| p.as_bool()) == Some(true))
        else {
            continue;
        };
        let file = span.get("file_name").and_then(|f| f.as_str()).unwrap_or("").to_string();
        let ln = span.get("line_start").and_then(|l| l.as_u64()).unwrap_or(1);
        let col = span.get("column_start").and_then(|l| l.as_u64()).unwrap_or(1);
        let text = msg.get("message").and_then(|m| m.as_str()).unwrap_or("");
        let code = msg.pointer("/code/code").and_then(|c| c.as_str());
        out.push(CheckerDiag {
            file,
            value: json!({
                "range": { "start": { "line": ln.saturating_sub(1), "character": col.saturating_sub(1) } },
                "severity": severity,
                "message": text,
                "source": "rustc",
                "code": code,
            }),
        });
    }
    out
}

/// `path(12,5): error TS2322: message`
fn parse_tsc(stdout: &str) -> Vec<CheckerDiag> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let Some(open) = line.find('(') else { continue };
        let Some(close) = line[open..].find("): ") else { continue };
        let file = &line[..open];
        let pos = &line[open + 1..open + close];
        let rest = &line[open + close + 3..];
        let mut it = pos.split(',');
        let (Some(l), Some(c)) = (it.next(), it.next()) else { continue };
        let (Ok(l), Ok(c)) = (l.trim().parse::<u64>(), c.trim().parse::<u64>()) else { continue };
        let (severity, rest) = if let Some(r) = rest.strip_prefix("error ") {
            (1, r)
        } else if let Some(r) = rest.strip_prefix("warning ") {
            (2, r)
        } else {
            continue;
        };
        let (code, message) = rest.split_once(": ").unwrap_or(("", rest));
        out.push(CheckerDiag {
            file: file.to_string(),
            value: json!({
                "range": { "start": { "line": l.saturating_sub(1), "character": c.saturating_sub(1) } },
                "severity": severity,
                "message": message,
                "source": "tsc",
                "code": code,
            }),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tsc_output() {
        let d = parse_tsc("src/a.tsx(12,5): error TS2322: Type 'x' is not assignable.\nnoise\n");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].file, "src/a.tsx");
        assert_eq!(d[0].value["range"]["start"]["line"], 11);
        assert_eq!(d[0].value["code"], "TS2322");
        assert!(same_file(&d[0].file, r"C:\proj\src\A.tsx"));
    }

    #[test]
    fn parses_cargo_json() {
        let line = r#"{"reason":"compiler-message","message":{"level":"error","message":"boom","code":{"code":"E0308"},"spans":[{"is_primary":true,"file_name":"src/lib.rs","line_start":3,"column_start":2}]}}"#;
        let d = parse_cargo(line);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].value["severity"], 1);
        assert_eq!(d[0].value["range"]["start"]["character"], 1);
    }
}
