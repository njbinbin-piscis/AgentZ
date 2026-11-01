/// File list tool — structured directory listing returned as JSON.
/// Unlike `shell cmd dir`, this returns machine-readable structured data the LLM can parse directly.
use crate::agent::tool::{Tool, ToolContext, ToolResult};
use crate::path_text::{display_path, display_path_under, os_text};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use std::time::UNIX_EPOCH;

const MAX_ENTRIES: usize = 500;

pub struct FileListTool;

#[async_trait]
impl Tool for FileListTool {
    fn name(&self) -> &str {
        "file_list"
    }

    fn description(&self) -> &str {
        "List directory contents as structured JSON. Returns file names, sizes, modification times, \
         and whether each entry is a file or directory. \
         Much easier for AI to parse than 'shell cmd dir' output. \
         Use recursive=true with max_depth to explore directory trees. \
         \
         Examples: \
         - List C:\\Tribon\\M3: path=C:\\Tribon\\M3 \
         - List all subdirs of C:\\: path=C:\\, recursive=false \
         - Explore a project tree: path=C:\\MyProject, recursive=true, max_depth=3"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to list. Relative paths are resolved from workspace root (e.g. 'src' lists the src/ directory inside the workspace). Defaults to workspace root if omitted."
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to list subdirectories recursively (default false)"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum recursion depth when recursive=true (default 3, max 10)"
                },
                "include_hidden": {
                    "type": "boolean",
                    "description": "Include hidden files and directories (default false)"
                },
                "dirs_only": {
                    "type": "boolean",
                    "description": "Only list directories, not files (default false)"
                }
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

        let path = if std::path::Path::new(path_str).is_absolute() {
            std::path::PathBuf::from(path_str)
        } else {
            ctx.workspace_root.join(path_str)
        };
        if !path.exists() {
            return Ok(ToolResult::err(format!(
                "Path does not exist: {}",
                display_path(&path)
            )));
        }
        if !path.is_dir() {
            return Ok(ToolResult::err(format!(
                "Not a directory: {}",
                display_path(&path)
            )));
        }

        let recursive = input["recursive"].as_bool().unwrap_or(false);
        let max_depth = (input["max_depth"].as_u64().unwrap_or(3) as usize).min(10);
        let include_hidden = input["include_hidden"].as_bool().unwrap_or(false);
        let dirs_only = input["dirs_only"].as_bool().unwrap_or(false);

        let mut entries: Vec<Value> = Vec::new();
        let mut read_error: Option<String> = None;
        collect_entries(
            &path,
            &path,
            0,
            if recursive { max_depth } else { 0 },
            include_hidden,
            dirs_only,
            &mut entries,
            &mut read_error,
        );

        if entries.is_empty() {
            if let Some(error) = read_error {
                return Ok(ToolResult::err(format!(
                    "Failed to list {}: {error}",
                    display_path(&path)
                )));
            }
            if dirs_only {
                return Ok(ToolResult::ok(format!(
                    "无子目录（文件数未知）: {}",
                    display_path(&path)
                )));
            }
            return Ok(ToolResult::ok(format!(
                "Directory is empty: {}",
                display_path(&path)
            )));
        }

        let total = entries.len();
        let truncated = total >= MAX_ENTRIES;
        let entries = if truncated {
            &entries[..MAX_ENTRIES]
        } else {
            &entries[..]
        };

        let result = json!({
            "path": display_path(&path),
            "total_entries": total,
            "truncated": truncated,
            "entries": entries
        });

        Ok(ToolResult::ok(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        ))
    }
}

fn collect_entries(
    root: &Path,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    include_hidden: bool,
    dirs_only: bool,
    out: &mut Vec<Value>,
    read_error: &mut Option<String>,
) {
    if out.len() >= MAX_ENTRIES {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(error) => {
            if depth == 0 {
                *read_error = Some(error.to_string());
            }
            return;
        }
    };

    let mut sorted: Vec<_> = entries.flatten().collect();
    // Directories first, then files, both sorted alphabetically
    sorted.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    for entry in sorted {
        if out.len() >= MAX_ENTRIES {
            break;
        }

        let path = entry.path();
        let name = os_text(&entry.file_name());

        // Skip hidden files unless requested
        if !include_hidden && name.starts_with('.') {
            continue;
        }

        // DirEntry already carries the find-data attributes. A second
        // metadata call can fail on cloud placeholders and long names, which
        // made real directories look like files and then disappear under dirs_only.
        let is_dir = entry
            .file_type()
            .map(|kind| kind.is_dir())
            .unwrap_or_else(|_| path.is_dir());

        if dirs_only && !is_dir {
            continue;
        }

        let meta = std::fs::metadata(&path).ok();
        let size = meta
            .as_ref()
            .and_then(|m| if is_dir { None } else { Some(m.len()) });
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| {
                // Format as YYYY-MM-DD HH:MM
                let secs = d.as_secs();
                let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0)
                    .unwrap_or_default();
                dt.format("%Y-%m-%d %H:%M").to_string()
            });

        let mut entry_json = json!({
            "name": name,
            "path": display_path(&path),
            "relative_path": display_path_under(&path, root),
            "type": if is_dir { "directory" } else { "file" },
        });

        if let Some(s) = size {
            entry_json["size_bytes"] = json!(s);
            // Human-readable size
            entry_json["size"] = json!(human_size(s));
        }
        if let Some(m) = modified {
            entry_json["modified"] = json!(m);
        }

        out.push(entry_json);

        // Recurse into directories
        if is_dir && depth < max_depth {
            // Skip common noise dirs
            if !matches!(
                name.as_str(),
                ".git" | "node_modules" | "target" | ".cache" | "__pycache__" | ".vs"
            ) {
                collect_entries(
                    root,
                    &path,
                    depth + 1,
                    max_depth,
                    include_hidden,
                    dirs_only,
                    out,
                    read_error,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool::{Tool, ToolContext, ToolSettings};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn ctx(root: &Path) -> ToolContext {
        ToolContext {
            session_id: "file-list".into(),
            workspace_root: root.to_path_buf(),
            bypass_permissions: false,
            settings: Arc::new(ToolSettings::default()),
            max_iterations: None,
            memory_owner_id: "test".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        }
    }

    #[tokio::test]
    async fn dirs_only_names_unicode_files_instead_of_calling_the_directory_empty() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("表驱动项目二");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("协议台账.txt"), "x").unwrap();
        let result = FileListTool
            .call(
                json!({ "path": "表驱动项目二", "dirs_only": true, "max_depth": 2 }),
                &ctx(temp.path()),
            )
            .await
            .unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("无子目录（文件数未知）"), "{}", result.content);
        assert!(!result.content.contains("协议台账.txt"), "{}", result.content);
        assert!(!result.content.contains("Directory is empty"), "{}", result.content);
    }

    #[tokio::test]
    async fn unicode_file_is_listed() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("中文目录");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("中文文件.md"), "x").unwrap();
        let result = FileListTool
            .call(json!({ "path": "中文目录" }), &ctx(temp.path()))
            .await
            .unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("中文文件.md"), "{}", result.content);
    }
}

fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
