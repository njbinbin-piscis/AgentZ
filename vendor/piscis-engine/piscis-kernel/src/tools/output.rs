//! Shared tool output formatting — smart truncation, error envelopes, diagnostics.

use serde::{Deserialize, Serialize};
use std::io::Write;

/// Machine-readable error category for tool failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorCode {
    FileNotFound,
    MatchNotFound,
    MatchAmbiguous,
    EncodingError,
    Timeout,
    PermissionDenied,
    InvalidInput,
    OutputTooLarge,
    /// A tool *input* parameter (e.g. `file_write`'s `content`) exceeds the
    /// size this tool is willing to accept in one call — distinct from
    /// `OutputTooLarge`, which is about truncating a large *result* rather
    /// than refusing to run at all (P2-3, OpenAI 兼容链路鲁棒性计划).
    ContentTooLarge,
    ExternalCommandFailed,
    Other,
}

impl ToolErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileNotFound => "file_not_found",
            Self::MatchNotFound => "match_not_found",
            Self::MatchAmbiguous => "match_ambiguous",
            Self::EncodingError => "encoding_error",
            Self::Timeout => "timeout",
            Self::PermissionDenied => "permission_denied",
            Self::InvalidInput => "invalid_input",
            Self::OutputTooLarge => "output_too_large",
            Self::ContentTooLarge => "content_too_large",
            Self::ExternalCommandFailed => "external_command_failed",
            Self::Other => "other",
        }
    }
}

/// Metadata appended to successful tool output.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolMeta {
    pub truncated: bool,
    pub total_bytes: usize,
    pub shown_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Truncation strategy for large tool output.
#[derive(Debug, Clone, Copy)]
pub enum TruncateStrategy {
    /// Head 60% + tail 40%.
    HeadTail,
    /// Prefer lines matching error heuristics, then head/tail fill.
    ErrorAware,
}

/// Format a successful tool body with optional meta footer.
pub fn format_ok(body: &str, meta: ToolMeta) -> String {
    if !meta.truncated && meta.hint.is_none() {
        return body.to_string();
    }
    let mut out = body.to_string();
    out.push_str("\n\n--- meta ---\n");
    out.push_str(&format!(
        "truncated={} total_bytes={} shown_bytes={}",
        meta.truncated, meta.total_bytes, meta.shown_bytes
    ));
    if let Some(h) = &meta.hint {
        out.push_str("\nhint: ");
        out.push_str(h);
    }
    out
}

/// Format a structured tool error for the LLM.
pub fn format_err(code: ToolErrorCode, message: &str, hint: &str) -> String {
    let mut out = format!("[error: {}]\n{}", code.as_str(), message.trim());
    if !hint.trim().is_empty() {
        out.push_str("\n\nhint: ");
        out.push_str(hint.trim());
    }
    out
}

fn char_boundary_index(s: &str, byte_idx: usize) -> usize {
    if byte_idx >= s.len() {
        return s.len();
    }
    let mut i = byte_idx;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Smart truncate `s` to at most `max_bytes` using `strategy`.
pub fn smart_truncate(s: &str, max_bytes: usize, strategy: TruncateStrategy) -> (String, ToolMeta) {
    let total = s.len();
    if total <= max_bytes {
        return (
            s.to_string(),
            ToolMeta {
                truncated: false,
                total_bytes: total,
                shown_bytes: total,
                hint: None,
            },
        );
    }

    match strategy {
        TruncateStrategy::HeadTail => head_tail_truncate(s, max_bytes, total),
        TruncateStrategy::ErrorAware => error_aware_truncate(s, max_bytes, total),
    }
}

fn head_tail_truncate(s: &str, max_bytes: usize, total: usize) -> (String, ToolMeta) {
    let head_bytes = (max_bytes * 3) / 5;
    let tail_bytes = max_bytes.saturating_sub(head_bytes).saturating_sub(64);
    let head_end = char_boundary_index(s, head_bytes.min(total));
    let tail_start = char_boundary_index(s, total.saturating_sub(tail_bytes));
    let omitted = total.saturating_sub(head_end + (total - tail_start));
    let out = format!(
        "{}\n\n... [{} bytes truncated] ...\n\n{}",
        &s[..head_end],
        omitted,
        &s[tail_start..]
    );
    let shown = out.len();
    (
        out,
        ToolMeta {
            truncated: true,
            total_bytes: total,
            shown_bytes: shown,
            hint: Some("Output was truncated. Re-run with a narrower command or filter.".into()),
        },
    )
}

fn error_aware_truncate(s: &str, max_bytes: usize, total: usize) -> (String, ToolMeta) {
    let errors = extract_error_lines(s, "");
    if errors.is_empty() {
        return head_tail_truncate(s, max_bytes, total);
    }
    let mut picked = String::new();
    picked.push_str("--- errors ---\n");
    for line in &errors {
        picked.push_str(line);
        picked.push('\n');
    }
    picked.push_str("\n--- output (truncated) ---\n");
    let budget = max_bytes.saturating_sub(picked.len());
    if budget > 256 {
        let (tail, _) = head_tail_truncate(s, budget, total);
        picked.push_str(&tail);
    }
    (
        picked.clone(),
        ToolMeta {
            truncated: true,
            total_bytes: total,
            shown_bytes: picked.len(),
            hint: Some("Non-zero exit or errors detected; key error lines preserved.".into()),
        },
    )
}

/// Extract likely error lines from command output (rustc, cargo, npm, pytest, etc.).
pub fn extract_error_lines(stdout: &str, stderr: &str) -> Vec<String> {
    let combined = if stderr.is_empty() {
        stdout.to_string()
    } else if stdout.is_empty() {
        stderr.to_string()
    } else {
        format!("{stderr}\n{stdout}")
    };

    let patterns = [
        "error:",
        "Error:",
        "ERROR ",
        "FAILED",
        "panic:",
        "Panic",
        "fatal:",
        "FATAL",
        "SyntaxError",
        "TypeError",
        "cannot find",
        "Could not compile",
        "error[E",
        "npm ERR!",
        "FAILED tests",
        "AssertionError",
        "Exception:",
        "Traceback (most recent call last)",
    ];

    let mut out: Vec<String> = Vec::new();
    let lines: Vec<&str> = combined.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if patterns.iter().any(|p| line.contains(p)) {
            let start = i.saturating_sub(2);
            let end = (i + 3).min(lines.len());
            for ctx in &lines[start..end] {
                let trimmed = ctx.trim_end();
                if !trimmed.is_empty() && !out.iter().any(|x| x == trimmed) {
                    out.push(trimmed.to_string());
                }
            }
        }
    }
    out
}

/// Collapse consecutive duplicate lines: `line\nline\n...` → `line\n... (repeated N times)`.
pub fn collapse_repeated_lines(s: &str) -> String {
    let mut out = String::new();
    let mut prev: Option<&str> = None;
    let mut repeat = 0u32;
    for line in s.lines() {
        if prev == Some(line) {
            repeat += 1;
            continue;
        }
        if repeat > 0 {
            if let Some(p) = prev {
                out.push_str(p);
                out.push('\n');
                if repeat > 1 {
                    out.push_str(&format!("... (repeated {} times)\n", repeat));
                }
            }
            repeat = 0;
        }
        prev = Some(line);
    }
    if let Some(p) = prev {
        out.push_str(p);
        if repeat > 0 {
            out.push('\n');
            if repeat > 1 {
                out.push_str(&format!("... (repeated {} times)", repeat));
            }
        }
    }
    out
}

/// Append a JSONL metrics record when `AGENTZ_TOOL_METRICS` or path is set.
pub fn log_tool_metric(
    log_path: Option<&std::path::Path>,
    name: &str,
    duration_ms: u128,
    output_bytes: usize,
    truncated: bool,
    is_error: bool,
) {
    let path = match log_path {
        Some(p) => p.to_path_buf(),
        None => match std::env::var("PISCIS_TOOL_METRICS").ok() {
            Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
            _ => return,
        },
    };
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let ts = chrono::Utc::now().to_rfc3339();
        let _ = writeln!(
            f,
            r#"{{"ts":"{ts}","tool":"{name}","duration_ms":{duration_ms},"output_bytes":{output_bytes},"truncated":{truncated},"is_error":{is_error}}}"#
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_tail_truncates_large_output() {
        let s = "a".repeat(10_000);
        let (out, meta) = smart_truncate(&s, 500, TruncateStrategy::HeadTail);
        assert!(meta.truncated);
        assert!(out.contains("truncated"));
        assert!(out.len() < 10_000);
    }

    #[test]
    fn extract_rust_errors() {
        let stderr =
            "   Compiling foo v0.1.0\nerror[E0425]: cannot find value `x`\n --> src/main.rs:1:5";
        let lines = extract_error_lines("", stderr);
        assert!(lines.iter().any(|l| l.contains("error[E0425]")));
    }

    #[test]
    fn format_err_includes_code_and_hint() {
        let s = format_err(
            ToolErrorCode::MatchNotFound,
            "old_string not found",
            "Re-read the file and use a smaller unique snippet",
        );
        assert!(s.contains("[error: match_not_found]"));
        assert!(s.contains("hint:"));
    }

    #[test]
    fn collapse_repeated_lines_works() {
        let s = "ok\nok\nok\nok\ndone";
        let c = collapse_repeated_lines(s);
        assert!(c.contains("repeated"));
        assert!(c.contains("done"));
    }
}
