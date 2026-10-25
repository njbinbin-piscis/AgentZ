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

#[allow(dead_code)]
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

/// Per-line character cap. A single minified/base64/progress line can be
/// megabytes long; without a per-line cap it eats the whole presentation
/// budget while telling the model nothing. Long lines keep their head and
/// tail (paths, URLs and status codes usually sit at the ends).
pub const MAX_LINE_CHARS: usize = 1_000;

/// Elide the middle of every line longer than [`MAX_LINE_CHARS`].
/// Returns the rewritten text and how many lines were elided.
pub fn cap_long_lines(s: &str) -> (String, usize) {
    let mut elided = 0usize;
    let mut out = String::with_capacity(s.len().min(1 << 20));
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let chars: Vec<(usize, char)> = line.char_indices().collect();
        if chars.len() <= MAX_LINE_CHARS {
            out.push_str(line);
            continue;
        }
        elided += 1;
        let head_chars = MAX_LINE_CHARS * 3 / 5;
        let tail_chars = MAX_LINE_CHARS - head_chars - 40;
        let head_end = chars[head_chars - 1].0 + chars[head_chars - 1].1.len_utf8();
        let tail_start = chars[chars.len() - tail_chars].0;
        out.push_str(&line[..head_end]);
        out.push_str(&format!(
            " …[{} chars elided from long line]… ",
            chars.len() - head_chars - tail_chars
        ));
        out.push_str(&line[tail_start..]);
    }
    (out, elided)
}

/// Smart truncate `s` to at most `max_bytes` using `strategy`.
pub fn smart_truncate(s: &str, max_bytes: usize, strategy: TruncateStrategy) -> (String, ToolMeta) {
    let total = s.len();
    // Balance step 1: cap individual long lines first so one huge line
    // cannot consume the entire budget (wasted tokens, no information).
    let (capped, elided_lines) = cap_long_lines(s);
    if capped.len() <= max_bytes {
        let truncated = elided_lines > 0;
        return (
            capped.clone(),
            ToolMeta {
                truncated,
                total_bytes: total,
                shown_bytes: capped.len(),
                hint: truncated.then(|| {
                    format!(
                        "{elided_lines} over-long line(s) were middle-elided (cap {MAX_LINE_CHARS} chars/line)."
                    )
                }),
            },
        );
    }

    // Balance step 2: whole-line head/tail selection (never slice mid-line).
    match strategy {
        TruncateStrategy::HeadTail => head_tail_truncate(&capped, max_bytes, total),
        TruncateStrategy::ErrorAware => error_aware_truncate(&capped, max_bytes, total),
    }
}

/// Pick whole head lines (55%) + whole tail lines (45%) fitting `max_bytes`,
/// with a marker naming the omitted line count. Input is already line-capped.
fn head_tail_lines(s: &str, max_bytes: usize) -> String {
    let lines: Vec<&str> = s.split('\n').collect();
    let marker_reserve = 96usize;
    let budget = max_bytes.saturating_sub(marker_reserve);
    let head_budget = budget * 55 / 100;
    let tail_budget = budget - head_budget;

    let mut head_end = 0usize; // exclusive line index
    let mut used = 0usize;
    while head_end < lines.len() && used + lines[head_end].len() + 1 <= head_budget {
        used += lines[head_end].len() + 1;
        head_end += 1;
    }
    let mut tail_start = lines.len();
    let mut tused = 0usize;
    while tail_start > head_end && tused + lines[tail_start - 1].len() + 1 <= tail_budget {
        tused += lines[tail_start - 1].len() + 1;
        tail_start -= 1;
    }
    if head_end >= tail_start {
        return s.to_string();
    }
    if head_end == 0 && tail_start == lines.len() {
        // A single (already line-capped) line still exceeds the whole budget:
        // fall back to a char-boundary head/tail slice of that line.
        let head = &s[..char_boundary_index(s, head_budget)];
        let tail = &s[char_boundary_index(s, s.len().saturating_sub(tail_budget))..];
        return format!(
            "{head}\n… [{} bytes omitted from over-long output] …\n{tail}",
            s.len().saturating_sub(head.len() + tail.len())
        );
    }
    let omitted_lines = tail_start - head_end;
    let omitted_bytes: usize = lines[head_end..tail_start].iter().map(|l| l.len() + 1).sum();
    let mut out = String::new();
    out.push_str(&lines[..head_end].join("\n"));
    if head_end > 0 {
        out.push('\n');
    }
    out.push_str(&format!(
        "… [{omitted_lines} lines / ~{omitted_bytes} bytes omitted] …\n"
    ));
    out.push_str(&lines[tail_start..].join("\n"));
    out
}

fn head_tail_truncate(s: &str, max_bytes: usize, total: usize) -> (String, ToolMeta) {
    let out = head_tail_lines(s, max_bytes);
    let shown = out.len();
    (
        out,
        ToolMeta {
            truncated: true,
            total_bytes: total,
            shown_bytes: shown,
            hint: Some(
                "Output was truncated by lines (long lines elided first).                  The full output is preserved — re-run filtered, or read the saved copy when one is named."
                    .into(),
            ),
        },
    )
}

/// Balance step 3: keep the full output on disk so truncation never loses
/// information — the model can page through it with file_read / file_search
/// instead of re-running the command. Writes into
/// `<workspace>/.agentz/logs/tool-output/` and prunes to the newest 30
/// files. Returns the path when written.
pub fn spill_tool_output(
    workspace_root: &std::path::Path,
    tool: &str,
    stdout: &str,
    stderr: &str,
) -> Option<std::path::PathBuf> {
    let dir = workspace_root.join(".agentz").join("logs").join("tool-output");
    std::fs::create_dir_all(&dir).ok()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("{tool}-{stamp}.log"));
    let mut body = String::new();
    body.push_str(stdout);
    if !stderr.is_empty() {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("--- stderr ---\n");
        body.push_str(stderr);
    }
    std::fs::write(&path, body).ok()?;
    // Prune old spills so the folder stays bounded.
    if let Ok(read) = std::fs::read_dir(&dir) {
        let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = read
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
            .filter_map(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .map(|t| (t, e.path()))
            })
            .collect();
        files.sort_by_key(|(t, _)| *t);
        let excess = files.len().saturating_sub(30);
        for (_, old) in files.into_iter().take(excess) {
            let _ = std::fs::remove_file(old);
        }
    }
    Some(path)
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
        picked.push_str(&head_tail_lines(s, budget));
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

/// Decodes captured child-process output. A multi-byte UTF-8 sequence cut by
/// a capture cap is dropped. cmd built-ins write the ANSI code page even
/// after `chcp 65001` while UTF-8-aware programs share the same pipe, so
/// each line that is not valid UTF-8 is decoded with the ANSI code page.
pub fn decode_process_output(bytes: &[u8]) -> String {
    let bytes = trim_partial_utf8_tail(bytes);
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    let ansi = ansi_encoding();
    let mut out = String::with_capacity(bytes.len());
    for line in bytes.split_inclusive(|&b| b == b'\n') {
        match std::str::from_utf8(line) {
            Ok(s) => out.push_str(s),
            Err(_) => match ansi.map(|enc| enc.decode_without_bom_handling(line)) {
                Some((cow, false)) => out.push_str(&cow),
                _ => out.push_str(&String::from_utf8_lossy(line)),
            },
        }
    }
    out
}

pub(crate) fn trim_partial_utf8_tail(bytes: &[u8]) -> &[u8] {
    for back in 1..=3.min(bytes.len()) {
        let b = bytes[bytes.len() - back];
        if b & 0xC0 == 0x80 {
            continue;
        }
        let need = match b {
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => return bytes,
        };
        return if need > back { &bytes[..bytes.len() - back] } else { bytes };
    }
    bytes
}

#[cfg(windows)]
fn ansi_encoding() -> Option<&'static encoding_rs::Encoding> {
    let cp = unsafe { windows_sys::Win32::Globalization::GetACP() };
    Some(match cp {
        936 | 54936 => encoding_rs::GB18030,
        950 => encoding_rs::BIG5,
        932 => encoding_rs::SHIFT_JIS,
        949 => encoding_rs::EUC_KR,
        866 => encoding_rs::IBM866,
        874 => encoding_rs::WINDOWS_874,
        1250 => encoding_rs::WINDOWS_1250,
        1251 => encoding_rs::WINDOWS_1251,
        1252 => encoding_rs::WINDOWS_1252,
        1253 => encoding_rs::WINDOWS_1253,
        1254 => encoding_rs::WINDOWS_1254,
        1255 => encoding_rs::WINDOWS_1255,
        1256 => encoding_rs::WINDOWS_1256,
        1257 => encoding_rs::WINDOWS_1257,
        1258 => encoding_rs::WINDOWS_1258,
        _ => return None,
    })
}

#[cfg(not(windows))]
fn ansi_encoding() -> Option<&'static encoding_rs::Encoding> {
    None
}

/// Collapse consecutive duplicate lines: `line\nline\n...` → `line\n... (repeated N times)`.
pub fn collapse_repeated_lines(s: &str) -> String {
    fn flush(out: &mut String, line: &str, repeat: u32) {
        out.push_str(line);
        out.push('\n');
        match repeat {
            0 => {}
            1 => {
                out.push_str(line);
                out.push('\n');
            }
            n => out.push_str(&format!("... (repeated {n} times)\n")),
        }
    }
    let mut out = String::new();
    let mut prev: Option<&str> = None;
    let mut repeat = 0u32;
    for line in s.lines() {
        if prev == Some(line) {
            repeat += 1;
            continue;
        }
        if let Some(p) = prev {
            flush(&mut out, p, repeat);
        }
        repeat = 0;
        prev = Some(line);
    }
    if let Some(p) = prev {
        flush(&mut out, p, repeat);
    }
    if out.ends_with('\n') {
        out.pop();
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
        assert!(out.contains("omitted"), "{out}");
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
    fn long_lines_are_middle_elided_not_budget_eating() {
        let long_line = format!("START{}END", "x".repeat(50_000));
        let s = format!("{long_line}\nshort tail line");
        let (out, meta) = smart_truncate(&s, 24 * 1024, TruncateStrategy::HeadTail);
        assert!(meta.truncated);
        assert!(out.contains("chars elided from long line"), "{out}");
        assert!(out.contains("START"));
        assert!(out.contains("END"));
        assert!(out.contains("short tail line"));
        assert!(out.len() < 4_000, "elided output stays small: {}", out.len());
    }

    #[test]
    fn many_lines_truncate_on_line_boundaries() {
        let s = (0..5_000)
            .map(|i| format!("line number {i} with some content"))
            .collect::<Vec<_>>()
            .join("\n");
        let (out, meta) = smart_truncate(&s, 4 * 1024, TruncateStrategy::HeadTail);
        assert!(meta.truncated);
        assert!(out.contains("lines / ~"), "{out}");
        assert!(out.contains("line number 0"));
        assert!(out.contains("line number 4999"));
        // Middle lines are omitted whole, never sliced.
        assert!(!out.contains("line number 2500 with"));
        assert!(out.len() <= 4 * 1024 + 512);
    }

    #[test]
    fn spill_writes_full_output_and_prunes() {
        let dir = tempfile::tempdir().unwrap();
        let big = "y".repeat(100_000);
        let path = spill_tool_output(dir.path(), "shell", &big, "err text").expect("spilled");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains(&big));
        assert!(body.contains("--- stderr ---"));
        assert!(path.starts_with(dir.path().join(".agentz").join("logs")));
    }

    #[test]
    fn collapse_repeated_lines_works() {
        let s = "ok\nok\nok\nok\ndone";
        let c = collapse_repeated_lines(s);
        assert_eq!(c, "ok\n... (repeated 3 times)\ndone");
    }

    #[test]
    fn decode_process_output_drops_cut_utf8_tail() {
        let mut b = "ab中".as_bytes().to_vec();
        b.truncate(b.len() - 1);
        assert_eq!(decode_process_output(&b), "ab");
        assert_eq!(decode_process_output("ab中".as_bytes()), "ab中");
        assert_eq!(decode_process_output(b"plain"), "plain");
    }

    #[test]
    fn decode_process_output_keeps_utf8_lines_in_mixed_streams() {
        let mut b = "外部 utf8\n".as_bytes().to_vec();
        b.extend_from_slice(b"caf\xE9\r\n");
        b.extend_from_slice("尾行".as_bytes());
        let s = decode_process_output(&b);
        assert!(s.starts_with("外部 utf8\n"), "{s}");
        assert!(s.ends_with("尾行"), "{s}");
        assert_eq!(s.lines().count(), 3);
    }

    #[test]
    fn collapse_repeated_lines_keeps_every_distinct_line() {
        assert_eq!(collapse_repeated_lines("a\r\nb\r\nc\r\n"), "a\nb\nc");
        assert_eq!(collapse_repeated_lines("a\na\nb"), "a\na\nb");
        assert_eq!(collapse_repeated_lines("x"), "x");
        assert_eq!(collapse_repeated_lines(""), "");
    }
}
