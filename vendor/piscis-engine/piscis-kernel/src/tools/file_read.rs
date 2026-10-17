use crate::agent::tool::{Tool, ToolContext, ToolResult};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::borrow::Cow;

const MAX_TEXT_BYTES: u64 = 256 * 1024; // 256 KB
const MAX_IMAGE_BYTES: u64 = 4 * 1024 * 1024; // 4 MB

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];

/// Decode raw file bytes to a String (also used by file_edit).
///
/// Priority:
/// 1. Strip UTF-8 BOM if present, decode as UTF-8.
/// 2. Try UTF-8 (no BOM).
/// 3. Try GBK/GB18030 (common on Chinese Windows systems).
/// 4. Lossy UTF-8 as last resort.
pub fn decode_bytes(bytes: &[u8]) -> (String, &'static str) {
    // UTF-16 LE/BE — convert via encoding_rs
    if bytes.starts_with(UTF16_LE_BOM) {
        let (cow, _, had_errors) = encoding_rs::UTF_16LE.decode(&bytes[UTF16_LE_BOM.len()..]);
        if !had_errors {
            return (cow.into_owned(), "utf-16-le");
        }
    }
    if bytes.starts_with(UTF16_BE_BOM) {
        let (cow, _, had_errors) = encoding_rs::UTF_16BE.decode(&bytes[UTF16_BE_BOM.len()..]);
        if !had_errors {
            return (cow.into_owned(), "utf-16-be");
        }
    }

    // UTF-8 with BOM
    let payload = if bytes.starts_with(UTF8_BOM) {
        &bytes[UTF8_BOM.len()..]
    } else {
        bytes
    };

    // Try strict UTF-8 first
    if let Ok(s) = std::str::from_utf8(payload) {
        let label = if bytes.starts_with(UTF8_BOM) {
            "utf-8-bom"
        } else {
            "utf-8"
        };
        return (s.to_owned(), label);
    }

    // Fall back to GBK (covers GB2312, GB18030 subset)
    let (cow, _, had_errors) = encoding_rs::GBK.decode(bytes);
    if !had_errors {
        return (cow.into_owned(), "gbk");
    }

    // Last resort: lossy UTF-8
    (String::from_utf8_lossy(bytes).into_owned(), "utf-8-lossy")
}

/// Build a structural outline (functions, classes, etc.) with line numbers.
fn build_file_outline(content: &str) -> String {
    let mut lines_out: Vec<String> = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        let is_symbol = trimmed.starts_with("pub fn")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn")
            || trimmed.starts_with("pub async fn")
            || trimmed.starts_with("pub struct")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("pub enum")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("impl ")
            || trimmed.starts_with("pub trait")
            || trimmed.starts_with("trait ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("export class")
            || trimmed.starts_with("export function")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("type ")
            || trimmed.starts_with("interface ");
        if is_symbol {
            lines_out.push(format!("{:6}|{}", i + 1, trimmed));
        }
    }
    if lines_out.is_empty() {
        "  (no outline symbols detected — use offset/limit to read content)".to_string()
    } else {
        lines_out.join("\n")
    }
}

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a known file. Returns text with line numbers, or base64 for images. \
         IMPORTANT: This tool reads FILE CONTENT only — do NOT use it to list directory contents. \
         To list files in a directory, use file_list or shell with 'dir C:\\SomePath /b'. \
         If you get 'permission denied', use shell with 'Get-Content \"path\"' or 'type \"path\"' instead. \
         Paths: relative paths (e.g. src/auth/auth.service.ts) are resolved from workspace root — prefer relative paths when reading files inside the workspace. \
         Use offset/limit for large files to avoid reading the whole file at once.\n\
         \n\
         Encoding: the tool auto-detects and transparently handles UTF-8 BOM, UTF-16 LE/BE, and GBK/GB18030. \
         When the file is not plain UTF-8, the result header will include '[encoding: gbk]' or similar. \
         You MUST pay attention to this label: if a file is GBK-encoded (common in legacy Chinese projects, \
         system logs, .ini/.cfg files from older Windows software), the content you receive has been \
         decoded to Unicode for you to read — but if you need to write it back, you must be aware that \
         the original encoding is GBK, not UTF-8. Use shell with PowerShell to write GBK files: \
         `[System.IO.File]::WriteAllText(path, content, [System.Text.Encoding]::GetEncoding('gbk'))`. \
         For UTF-8-BOM files, file_write and file_edit will automatically preserve the BOM on write-back."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file. Relative paths (e.g. src/auth/auth.service.ts) are resolved from workspace root — prefer relative paths when working inside the workspace. Use absolute paths for files outside the workspace."
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-indexed). Use with limit to read large files in chunks."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read. Omit to read the whole file (up to 256KB)."
                },
                "mode": {
                    "type": "string",
                    "enum": ["content", "outline"],
                    "description": "content (default): numbered lines. outline: symbol list with line numbers only."
                }
            },
            "required": ["path"]
        })
    }

    fn description_minimal(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Read a file's contents. Returns numbered text, or base64 for images. \
             Relative paths resolve from workspace root. Use offset/limit for large files. \
             Does NOT list directory contents — use file_list for that. \
             Auto-detects UTF-8/UTF-16/GBK and reports encoding in the result header.",
        )
    }

    fn input_schema_minimal(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":   { "type": "string" },
                "offset": { "type": "integer", "minimum": 1 },
                "limit":  { "type": "integer", "minimum": 1 }
            },
            "required": ["path"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path_str = match input["path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::err("Missing required parameter: path")),
        };

        // Resolve path relative to workspace
        let path = if std::path::Path::new(path_str).is_absolute() {
            std::path::PathBuf::from(path_str)
        } else {
            ctx.workspace_root.join(path_str)
        };

        if !path.exists() {
            return Ok(ToolResult::err(crate::tools::output::format_err(
                crate::tools::output::ToolErrorCode::FileNotFound,
                &format!("File not found: {}", path.display()),
                "Use file_list or file_search glob to verify the path.",
            )));
        }

        let metadata = std::fs::metadata(&path)?;

        // Determine file type by extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let is_image = matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
        );

        if is_image {
            if metadata.len() > MAX_IMAGE_BYTES {
                return Ok(ToolResult::err(format!(
                    "Image too large ({} bytes, max {} bytes)",
                    metadata.len(),
                    MAX_IMAGE_BYTES
                )));
            }
            let bytes = std::fs::read(&path)?;
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
            let media_type = match ext.as_str() {
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                _ => "image/png",
            };
            return Ok(ToolResult::ok(format!(
                "Image file: {} ({} bytes)\nbase64:{};{}",
                path.display(),
                bytes.len(),
                media_type,
                b64
            )));
        }

        let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = input["limit"].as_u64().map(|l| l as usize);
        let mode = input["mode"].as_str().unwrap_or("content");

        let raw = std::fs::read(&path).map_err(|e| {
            let hint = if e.kind() == std::io::ErrorKind::PermissionDenied {
                format!(
                    "Failed to read file: {} (os error 5 - 拒绝访问)\n\
                     提示：该文件受系统权限保护，无法直接读取。\
                     请改用 shell 工具（如 `Get-Content` 或 `type`）以当前用户权限读取，\
                     或确认文件路径是否正确。",
                    path.display()
                )
            } else {
                format!("Failed to read file: {}", e)
            };
            anyhow::anyhow!("{}", hint)
        })?;

        let (content, encoding) = decode_bytes(&raw);
        let encoding_note = if encoding != "utf-8" {
            format!(" [encoding: {}]", encoding)
        } else {
            String::new()
        };

        if mode == "outline" {
            let outline = build_file_outline(&content);
            return Ok(ToolResult::ok(format!(
                "Outline: {}{} ({} bytes)\n\n{}",
                path.display(),
                encoding_note,
                metadata.len(),
                outline
            )));
        }

        // Large file without chunk params: outline + preview instead of hard reject.
        if metadata.len() > MAX_TEXT_BYTES && limit.is_none() && offset <= 1 {
            let lines: Vec<&str> = content.lines().collect();
            let preview_end = 100.min(lines.len());
            let preview: String = lines[..preview_end]
                .iter()
                .enumerate()
                .map(|(i, line)| format!("{:6}|{}", i + 1, line))
                .collect::<Vec<_>>()
                .join("\n");
            let outline = build_file_outline(&content);
            let hint = format!(
                "file_read path={} offset={} limit=200",
                path_str,
                preview_end + 1
            );
            return Ok(ToolResult::ok_with_meta(
                format!(
                    "File too large for full read ({} bytes). Outline + first {} lines:\n\n--- outline ---\n{}\n\n--- preview ---\n{}",
                    metadata.len(),
                    preview_end,
                    outline,
                    preview
                ),
                crate::tools::output::ToolMeta {
                    truncated: true,
                    total_bytes: metadata.len() as usize,
                    shown_bytes: preview.len(),
                    hint: Some(hint),
                },
            ));
        }

        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();
        let start = (offset - 1).min(total);
        let end = match limit {
            Some(l) => (start + l).min(total),
            None => total,
        };

        let numbered: String = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:6}|{}", start + i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        let mut hint = None;
        if end < total {
            hint = Some(format!(
                "file_read path={} offset={} limit={}",
                path_str,
                end + 1,
                limit.unwrap_or(200)
            ));
        }

        let body = format!(
            "File: {}{} ({} lines total, showing lines {}-{})\n\n{}",
            path.display(),
            encoding_note,
            total,
            start + 1,
            end,
            numbered
        );

        if hint.is_some() {
            Ok(ToolResult::ok_with_meta(
                body,
                crate::tools::output::ToolMeta {
                    truncated: end < total,
                    total_bytes: content.len(),
                    shown_bytes: numbered.len(),
                    hint,
                },
            ))
        } else {
            Ok(ToolResult::ok(body))
        }
    }
}
