use crate::agent::tool::{Tool, ToolContext, ToolResult};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::borrow::Cow;

use super::file_read::decode_bytes;
use super::output::{format_err, ToolErrorCode};

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// P2-3: default ceiling on `file_write`'s `content` parameter, in UTF-8
/// characters. This is deliberately separate from JSON-truncation handling
/// (P0-2/P2-2) — the arguments here parsed *successfully*, this is purely
/// "the model asked for something too big to write in one call, tell it to
/// use a different tool instead of silently truncating or OOM-ing."
const MAX_FILE_WRITE_CONTENT_CHARS: usize = 200_000;

/// Return true if the file starts with a UTF-8 BOM.
fn file_has_utf8_bom(path: &std::path::Path) -> bool {
    let mut buf = [0u8; 3];
    if let Ok(mut f) = std::fs::File::open(path) {
        use std::io::Read;
        if f.read_exact(&mut buf).is_ok() {
            return buf == *UTF8_BOM;
        }
    }
    false
}

/// Write content preserving UTF-8 BOM and detected source encoding.
fn write_preserving_encoding(
    path: &std::path::Path,
    content: &str,
    encoding: &str,
    preserve_bom: bool,
) -> std::io::Result<()> {
    if encoding == "gbk" {
        let (bytes, _, _) = encoding_rs::GBK.encode(content);
        return std::fs::write(path, &*bytes);
    }
    write_with_bom_policy(path, content, preserve_bom)
}

fn write_with_bom_policy(
    path: &std::path::Path,
    content: &str,
    preserve_bom: bool,
) -> std::io::Result<()> {
    if preserve_bom {
        let mut bytes = Vec::with_capacity(UTF8_BOM.len() + content.len());
        bytes.extend_from_slice(UTF8_BOM);
        bytes.extend_from_slice(content.as_bytes());
        std::fs::write(path, bytes)
    } else {
        std::fs::write(path, content.as_bytes())
    }
}

pub struct FileWriteTool;

/// Convert every line ending in `s` to CRLF (`crlf == true`) or LF.
fn adapt_eol(s: &str, crlf: bool) -> String {
    let lf = s.replace("\r\n", "\n");
    if crlf {
        lf.replace('\n', "\r\n")
    } else {
        lf
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file and all parent directories if they don't exist. \
         Completely overwrites existing content — use file_edit if you only want to change part of a file. \
         Paths: use relative paths (e.g. src/auth/auth.service.ts) to write inside the current workspace root. \
         Use absolute paths only when writing outside the workspace. \
         Note: writing to system directories (C:\\Windows\\, C:\\Program Files\\) will fail with permission denied — \
         write to user directories (C:\\Users\\name\\, Desktop, Documents) or the workspace instead.\n\
         \n\
         Encoding: this tool always writes UTF-8. If the target file already exists and has a UTF-8 BOM \
         (common for files created by Notepad or PowerShell on Windows), the BOM is automatically preserved. \
         WARNING: do NOT use file_write for files that must stay in GBK/GB18030 or other non-UTF-8 encodings \
         (e.g. legacy config files, files consumed by older Chinese software). For those, use shell with \
         `[System.IO.File]::WriteAllText(path, content, [System.Text.Encoding]::GetEncoding('gbk'))` instead. \
         If you are unsure of a file's original encoding, read it first with file_read and check the \
         '[encoding: ...]' label in the result header."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file. Relative paths (e.g. src/auth/auth.service.ts) are resolved from workspace root — prefer relative paths when working inside the workspace. Use absolute paths only for files outside the workspace."
                },
                "content": {
                    "type": "string",
                    "description": "Full content to write. This REPLACES the entire file. Use file_edit to modify only part of an existing file."
                }
            },
            "required": ["path", "content"]
        })
    }

    fn description_minimal(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Write full content to a file; creates parents and overwrites existing content. \
             Use file_edit to change only part. Relative paths resolve from workspace root. \
             Writes UTF-8 and preserves an existing UTF-8 BOM. For GBK/legacy encodings, use shell.",
        )
    }

    fn input_schema_minimal(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["path", "content"]
        })
    }

    fn needs_confirmation(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path_str = match input["path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::err("Missing required parameter: path")),
        };
        let content = match input["content"].as_str() {
            Some(c) => c,
            None => return Ok(ToolResult::err("Missing required parameter: content")),
        };

        // P2-3: reject oversized content explicitly rather than writing a
        // truncated/partial file or risking the arguments having been cut
        // off by an upstream token/byte limit in the first place (that
        // failure mode is caught earlier, in the JSON parsing layer — this
        // guards the case where the JSON parsed fine but the value is just
        // too large for a single write).
        let content_chars = content.chars().count();
        if content_chars > MAX_FILE_WRITE_CONTENT_CHARS {
            return Ok(ToolResult::err(format_err(
                ToolErrorCode::ContentTooLarge,
                &format!(
                    "content is {} characters, which exceeds the {} character limit for a single file_write call",
                    content_chars, MAX_FILE_WRITE_CONTENT_CHARS
                ),
                "Split the write into smaller pieces: create the file with an initial chunk via file_write, \
                 then use file_edit to append/replace the remaining sections — or write it in multiple \
                 file_write calls to different files and combine them via shell if appropriate.",
            )));
        }

        let path = if std::path::Path::new(path_str).is_absolute() {
            std::path::PathBuf::from(path_str)
        } else {
            ctx.workspace_root.join(path_str)
        };

        // Create parent directories
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let existed = path.exists();
        let preserve_bom = existed && file_has_utf8_bom(&path);
        write_with_bom_policy(&path, content, preserve_bom)?;

        let action = if existed { "Updated" } else { "Created" };
        Ok(ToolResult::ok(format!(
            "{} file: {} ({} bytes)",
            action,
            path.display(),
            content.len()
        )))
    }
}

// ---------------------------------------------------------------------------
// File Edit Tool (patch-based, supports single edit or batched edits array)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod file_write_size_limit_tests {
    use super::*;
    use crate::agent::tool::ToolSettings;
    use serde_json::json;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn test_ctx(root: std::path::PathBuf) -> ToolContext {
        ToolContext {
            session_id: "file-write-size-limit-test".into(),
            workspace_root: root,
            bypass_permissions: false,
            settings: Arc::new(ToolSettings::default()),
            max_iterations: Some(1),
            memory_owner_id: "piscis".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        }
    }

    fn unique_workspace(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("piscis-file-write-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn rejects_content_over_the_limit_without_writing_anything() {
        let root = unique_workspace("too-large");
        let ctx = test_ctx(root.clone());
        let oversized = "x".repeat(MAX_FILE_WRITE_CONTENT_CHARS + 1);

        let result = FileWriteTool
            .call(json!({"path": "out.txt", "content": oversized}), &ctx)
            .await
            .expect("call should not error at the Rust level");

        assert!(result.is_error, "expected a tool error, got {:?}", result);
        assert!(result.content.contains("content_too_large"), "content={}", result.content);
        assert!(!root.join("out.txt").exists(), "must not write a partial file");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn accepts_content_at_exactly_the_limit() {
        let root = unique_workspace("at-limit");
        let ctx = test_ctx(root.clone());
        let exactly_at_limit = "x".repeat(MAX_FILE_WRITE_CONTENT_CHARS);

        let result = FileWriteTool
            .call(json!({"path": "out.txt", "content": exactly_at_limit}), &ctx)
            .await
            .expect("call should succeed");

        assert!(!result.is_error, "content={}", result.content);
        assert!(root.join("out.txt").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn small_content_is_unaffected() {
        let root = unique_workspace("small");
        let ctx = test_ctx(root.clone());

        let result = FileWriteTool
            .call(json!({"path": "out.txt", "content": "hello"}), &ctx)
            .await
            .expect("call should succeed");

        assert!(!result.is_error, "content={}", result.content);
        assert_eq!(std::fs::read_to_string(root.join("out.txt")).unwrap(), "hello");

        let _ = std::fs::remove_dir_all(&root);
    }
}

pub struct FileEditTool;

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing exact strings with new strings. \
         Supports two modes:\n\
         1. Single edit: provide `old_string` and `new_string` — replaces one occurrence.\n\
         2. Batch edits: provide `edits` array of `{old_string, new_string}` objects — \
            all replacements are validated first (each old_string must appear exactly once) \
            then applied atomically in a single write. Prefer batch mode when making \
            multiple changes to the same file to reduce round-trips.\n\
         \n\
         Encoding: file_edit reads the file as bytes, strips any UTF-8 BOM before matching, \
         then restores the BOM on write-back — so BOM-bearing files are handled transparently. \
         However, file_edit only supports UTF-8 and UTF-8-BOM files. Do NOT use file_edit on \
         GBK/GB18030 files — the byte-level mismatch will corrupt the file. If file_read reports \
         '[encoding: gbk]' for a file, edit it via shell with PowerShell string replacement instead."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file. Relative paths (e.g. src/auth/auth.service.ts) are resolved from workspace root — prefer relative paths when working inside the workspace."
                },
                "old_string": {
                    "type": "string",
                    "description": "Single-edit mode: the exact string to replace (must appear exactly once)"
                },
                "new_string": {
                    "type": "string",
                    "description": "Single-edit mode: the replacement string"
                },
                "edits": {
                    "type": "array",
                    "description": "Batch-edit mode: list of replacements to apply atomically. Each old_string must appear exactly once.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_string": { "type": "string", "description": "Exact text to replace (must appear exactly once)" },
                            "new_string": { "type": "string", "description": "Replacement text" }
                        },
                        "required": ["old_string", "new_string"]
                    }
                }
            },
            "required": ["path"]
        })
    }

    fn description_minimal(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Edit a file by exact string replacement. Single mode: {old_string,new_string}. \
             Batch mode: {edits:[{old_string,new_string}...]} — validated first, applied atomically. \
             Each old_string must appear exactly once. UTF-8 / UTF-8-BOM only; not GBK.",
        )
    }

    fn input_schema_minimal(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":       { "type": "string" },
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

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path_str = match input["path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::err("Missing required parameter: path")),
        };

        let path = if std::path::Path::new(path_str).is_absolute() {
            std::path::PathBuf::from(path_str)
        } else {
            ctx.workspace_root.join(path_str)
        };

        if !path.exists() {
            return Ok(ToolResult::err(format_err(
                ToolErrorCode::FileNotFound,
                &format!("File not found: {}", path.display()),
                "Verify the path with file_list or file_search.",
            )));
        }

        // Build the list of (old, new) pairs from either mode
        let mut pairs: Vec<(String, String)> = if let Some(edits_arr) = input["edits"].as_array() {
            if edits_arr.is_empty() {
                return Ok(ToolResult::err("edits array is empty"));
            }
            let mut pairs = Vec::with_capacity(edits_arr.len());
            for (i, edit) in edits_arr.iter().enumerate() {
                let old = match edit["old_string"].as_str() {
                    Some(s) if !s.is_empty() => s.to_string(),
                    Some(_) => {
                        return Ok(ToolResult::err(format!(
                            "edits[{}].old_string cannot be empty",
                            i
                        )))
                    }
                    None => return Ok(ToolResult::err(format!("edits[{}] missing old_string", i))),
                };
                let new = match edit["new_string"].as_str() {
                    Some(s) => s.to_string(),
                    None => return Ok(ToolResult::err(format!("edits[{}] missing new_string", i))),
                };
                pairs.push((old, new));
            }
            pairs
        } else {
            let old_str =
                match input["old_string"].as_str() {
                    Some(s) if !s.is_empty() => s.to_string(),
                    Some(_) => return Ok(ToolResult::err(
                        "old_string cannot be empty — provide the exact text you want to replace",
                    )),
                    None => {
                        return Ok(ToolResult::err(
                            "Missing required parameter: old_string (or use edits array)",
                        ))
                    }
                };
            let new_str = match input["new_string"].as_str() {
                Some(s) => s.to_string(),
                None => return Ok(ToolResult::err("Missing required parameter: new_string")),
            };
            vec![(old_str, new_str)]
        };

        let raw = std::fs::read(&path)?;
        let preserve_bom = raw.starts_with(UTF8_BOM);
        let (content, encoding) = decode_bytes(&raw);
        let lines_before = content.lines().count();

        // Line-ending tolerance: model-supplied snippets are usually LF, while the
        // file on disk may be CRLF (or vice versa). If the snippet does not match
        // verbatim, retry with its line endings converted to the file's style, and
        // convert `new` the same way so the file keeps a consistent EOL.
        let file_is_crlf = content.contains("\r\n");
        for (old, new) in pairs.iter_mut() {
            if content.contains(old.as_str()) {
                continue;
            }
            let adapted_old = adapt_eol(old, file_is_crlf);
            if adapted_old != *old && content.contains(adapted_old.as_str()) {
                *new = adapt_eol(new, file_is_crlf);
                *old = adapted_old;
            }
        }

        for (i, (old, _)) in pairs.iter().enumerate() {
            let count = content.matches(old.as_str()).count();
            if count == 0 {
                let label = if pairs.len() == 1 {
                    "old_string".to_string()
                } else {
                    format!("edits[{}].old_string", i)
                };
                return Ok(ToolResult::err(format_err(
                    ToolErrorCode::MatchNotFound,
                    &format!("{} not found in file: {}", label, path.display()),
                    "Re-read the file with file_read and copy an exact unique snippet.",
                )));
            }
            if count > 1 {
                let label = if pairs.len() == 1 {
                    "old_string".to_string()
                } else {
                    format!("edits[{}].old_string", i)
                };
                return Ok(ToolResult::err(format_err(
                    ToolErrorCode::MatchAmbiguous,
                    &format!(
                        "{} appears {} times in file (must appear exactly once): {}",
                        label,
                        count,
                        path.display()
                    ),
                    "Include more surrounding lines in old_string to make it unique.",
                )));
            }
        }

        for i in 0..pairs.len() {
            for j in (i + 1)..pairs.len() {
                if pairs[i].0 == pairs[j].0 {
                    return Ok(ToolResult::err(format!(
                        "edits[{}] and edits[{}] have the same old_string — each must be unique",
                        i, j
                    )));
                }
            }
        }

        let mut offsets: Vec<(usize, usize, usize)> = Vec::with_capacity(pairs.len());
        for (idx, (old, _)) in pairs.iter().enumerate() {
            if let Some(pos) = content.find(old.as_str()) {
                offsets.push((pos, idx, old.len()));
            }
        }
        offsets.sort_by_key(|o| std::cmp::Reverse(o.0));

        let mut result = content.clone();
        for (pos, pair_idx, old_len) in offsets {
            result.replace_range(pos..pos + old_len, &pairs[pair_idx].1);
        }

        write_preserving_encoding(&path, &result, encoding, preserve_bom)?;

        let lines_after = result.lines().count();
        let line_delta = lines_after as i64 - lines_before as i64;
        let delta_str = if line_delta >= 0 {
            format!("+{}", line_delta)
        } else {
            format!("{}", line_delta)
        };

        if pairs.len() == 1 {
            Ok(ToolResult::ok(format!(
                "Edited file: {} ({} chars → {} chars, {} lines {}, encoding: {})",
                path.display(),
                pairs[0].0.len(),
                pairs[0].1.len(),
                lines_after,
                delta_str,
                encoding
            )))
        } else {
            Ok(ToolResult::ok(format!(
                "Edited file: {} ({} replacements applied, {} lines {}, encoding: {})",
                path.display(),
                pairs.len(),
                lines_after,
                delta_str,
                encoding
            )))
        }
    }
}

#[cfg(test)]
mod eol_tests {
    use super::adapt_eol;

    #[test]
    fn lf_snippet_becomes_crlf_for_crlf_file() {
        assert_eq!(adapt_eol("a\nb", true), "a\r\nb");
        assert_eq!(adapt_eol("a\r\nb", true), "a\r\nb");
    }

    #[test]
    fn crlf_snippet_becomes_lf_for_lf_file() {
        assert_eq!(adapt_eol("a\r\nb", false), "a\nb");
    }
}
