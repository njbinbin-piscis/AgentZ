/// File search tool — glob (find files by name pattern) and grep (search file contents).
/// Equivalent to Cursor's Glob and Grep tools.
use crate::agent::tool::{Tool, ToolContext, ToolResult};
use crate::path_text::{display_path_for_tool, display_path_under, os_text};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MAX_RESULTS: usize = 500;
const MAX_CONTENT_BYTES: usize = 200 * 1024; // 200 KB per file for grep
pub const EXCLUDED_SEARCH_DIRECTORY_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".cache",
    "__pycache__",
    ".vs",
];

/// Decodes text in any encoding `file_read` understands; `None` for binary files.
fn decode_text(bytes: &[u8]) -> Option<String> {
    let utf16 = bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]);
    if !utf16 && bytes[..bytes.len().min(8192)].contains(&0) {
        return None;
    }
    let (text, label) = super::file_read::decode_bytes(bytes);
    if label == "utf-8-lossy" {
        return None;
    }
    Some(text.strip_prefix('\u{FEFF}').map(str::to_owned).unwrap_or(text))
}

pub fn is_excluded_search_directory(name: &str) -> bool {
    EXCLUDED_SEARCH_DIRECTORY_NAMES
        .iter()
        .any(|excluded| excluded.eq_ignore_ascii_case(name))
}

pub struct FileSearchTool;

#[async_trait]
impl Tool for FileSearchTool {
    fn name(&self) -> &str {
        "file_search"
    }

    fn description(&self) -> &str {
        "Search for files by name pattern (glob) or search file contents by regex (grep). \
         Equivalent to the Glob and Grep tools used by Cursor AI. \
         \
         Actions: \
         - 'glob': Find files whose names match a pattern. Pattern supports * (any chars in name), \
           ** (any path segment), ? (single char). Example: '*.rs', '**/*.json', 'config*'. \
         - 'grep': Search file contents for a regex pattern. Returns matching lines with file path and line number. \
           Use 'include' to filter by file extension (e.g. '*.rs', '*.txt'). \
         \
         Tips: \
         - To find all .py files under C:\\MyApp: action=glob, pattern=**/*.py, path=C:\\MyApp \
         - To find all files containing 'TBRuntime': action=grep, pattern=TBRuntime, path=C:\\Tribon \
         - To search only in .ini files: action=grep, pattern=password, include=*.ini, path=C:\\"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["glob", "grep"],
                    "description": "'glob' = find files by name pattern. 'grep' = search file contents by regex."
                },
                "pattern": {
                    "type": "string",
                    "description": "For glob: file name pattern (e.g. '*.rs', '**/*.json'). For grep: regex pattern to search in file contents."
                },
                "path": {
                    "type": "string",
                    "description": "Root directory to search in. Relative paths are resolved from workspace root (e.g. 'src' searches inside workspace/src/). Defaults to workspace root if omitted."
                },
                "include": {
                    "type": "string",
                    "description": "For grep only: file name filter (e.g. '*.txt', '*.rs'). Only files matching this pattern will be searched."
                },
                "file_extensions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "For grep/glob: restrict search to files with these extensions (e.g. [\"rs\", \"toml\"]). More precise than `include`."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default 50, max 500)"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum directory depth to recurse (default 10)"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "For grep: case-sensitive search (default false)"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "For grep: number of lines of context to show around each match (default 0)"
                },
                "include_ignored": {
                    "type": "boolean",
                    "description": "When true, also search inside normally excluded directories (node_modules, target, .cache, __pycache__, .vs). Default false (.git is always skipped)."
                }
            },
            "required": ["action", "pattern"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let action = match input["action"].as_str() {
            Some(a) => a,
            None => return Ok(ToolResult::err("Missing required parameter: action")),
        };
        let pattern = match input["pattern"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::err("Missing required parameter: pattern")),
        };

        let root = if let Some(p) = input["path"].as_str() {
            if std::path::Path::new(p).is_absolute() {
                PathBuf::from(p)
            } else {
                ctx.workspace_root.join(p)
            }
        } else if !ctx.workspace_root.as_os_str().is_empty() {
            ctx.workspace_root.clone()
        } else {
            #[cfg(target_os = "windows")]
            {
                PathBuf::from("C:\\")
            }
            #[cfg(not(target_os = "windows"))]
            {
                PathBuf::from("/")
            }
        };

        if !root.exists() {
            return Ok(ToolResult::err(format!(
                "Path does not exist: {}",
                root.display()
            )));
        }

        let max_results = (input["max_results"].as_u64().unwrap_or(50) as usize).min(MAX_RESULTS);
        let max_depth = input["max_depth"].as_u64().unwrap_or(10) as usize;

        // Collect file_extensions filter (lowercase, no leading dot)
        let file_extensions: Vec<String> = input["file_extensions"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.trim_start_matches('.').to_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        let ext_filter: Option<&[String]> = if file_extensions.is_empty() {
            None
        } else {
            Some(&file_extensions)
        };

        let include_ignored = input["include_ignored"].as_bool().unwrap_or(false);
        let anchor = if ctx.workspace_root.as_os_str().is_empty() {
            root.clone()
        } else {
            ctx.workspace_root.clone()
        };

        match action {
            "glob" => self.do_glob(
                pattern,
                &root,
                &anchor,
                max_results,
                max_depth,
                ext_filter,
                include_ignored,
            ),
            "grep" => {
                let include = input["include"].as_str();
                let case_sensitive = input["case_sensitive"].as_bool().unwrap_or(false);
                let context_lines = input["context_lines"].as_u64().unwrap_or(0) as usize;
                self.do_grep(
                    pattern,
                    &root,
                    &anchor,
                    include,
                    ext_filter,
                    max_results,
                    max_depth,
                    case_sensitive,
                    context_lines,
                    include_ignored,
                )
            }
            _ => Ok(ToolResult::err(format!("Unknown action: {}", action))),
        }
    }
}

impl FileSearchTool {
    #[allow(clippy::too_many_arguments)]
    fn do_glob(
        &self,
        pattern: &str,
        root: &Path,
        workspace: &Path,
        max_results: usize,
        max_depth: usize,
        ext_filter: Option<&[String]>,
        include_ignored: bool,
    ) -> Result<ToolResult> {
        let regex_pat = glob_to_regex(pattern);
        let re = match regex::Regex::new(&regex_pat) {
            Ok(r) => r,
            Err(e) => {
                return Ok(ToolResult::err(format!(
                    "Invalid glob pattern '{}': {}",
                    pattern, e
                )))
            }
        };

        let mut matches: Vec<String> = Vec::new();
        let mut skipped_dirs: Vec<String> = Vec::new();
        walk_dir_opts(
            root,
            0,
            max_depth,
            include_ignored,
            &mut skipped_dirs,
            &mut |path: &Path| {
                if matches.len() >= max_results {
                    return false;
                }
                if path.is_file() {
                    // Apply extension filter
                    if let Some(exts) = ext_filter {
                        let file_ext = extension_text(path);
                        if !exts.iter().any(|e| e == &file_ext) {
                            return true;
                        }
                    }
                    let file_name = file_name_text(path);
                    let rel_str = display_path_under(path, root);
                    if re.is_match(&rel_str) || re.is_match(&file_name) {
                        matches.push(display_path_for_tool(path, workspace));
                    }
                }
                true
            },
        );

        matches.sort_by_key(|p| p.matches('/').count() + p.matches('\\').count());

        if matches.is_empty() {
            let ignored_note = format_skipped_dirs_note(&skipped_dirs);
            return Ok(ToolResult::ok(format!(
                "No files found matching '{}' under {}{}",
                pattern,
                display_path_for_tool(root, workspace),
                ignored_note
            )));
        }

        Ok(ToolResult::ok(format!(
            "Found {} file(s) matching '{}' under {}:\n{}",
            matches.len(),
            pattern,
            display_path_for_tool(root, workspace),
            matches.join("\n")
        )))
    }

    #[allow(clippy::too_many_arguments)]
    fn do_grep(
        &self,
        pattern: &str,
        root: &Path,
        workspace: &Path,
        include: Option<&str>,
        ext_filter: Option<&[String]>,
        max_results: usize,
        max_depth: usize,
        case_sensitive: bool,
        context_lines: usize,
        include_ignored: bool,
    ) -> Result<ToolResult> {
        let re = {
            let pat = if case_sensitive {
                pattern.to_string()
            } else {
                format!("(?i){}", pattern)
            };
            match regex::Regex::new(&pat) {
                Ok(r) => r,
                Err(e) => {
                    return Ok(ToolResult::err(format!(
                        "Invalid regex '{}': {}",
                        pattern, e
                    )))
                }
            }
        };

        // Build include filter regex if provided
        let include_re = include.and_then(|inc| {
            let pat = glob_to_regex(inc);
            regex::Regex::new(&pat).ok()
        });

        let mut results: Vec<String> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        let mut skipped_dirs: Vec<String> = Vec::new();
        let mut total_matches = 0usize;

        walk_dir_opts(
            root,
            0,
            max_depth,
            include_ignored,
            &mut skipped_dirs,
            &mut |path: &Path| {
                if total_matches >= max_results {
                    return false;
                }
                if !path.is_file() {
                    return true;
                }

                let fname = file_name_text(path);

                // Apply extension filter (takes priority over include pattern)
                if let Some(exts) = ext_filter {
                    let file_ext = extension_text(path);
                    if !exts.iter().any(|e| e == &file_ext) {
                        return true;
                    }
                } else if let Some(ref inc_re) = include_re {
                    // Fall back to include glob pattern
                    if !inc_re.is_match(&fname) {
                        return true;
                    }
                }

                let meta = match std::fs::metadata(path) {
                    Ok(m) => m,
                    Err(_) => return true,
                };
                if meta.len() > MAX_CONTENT_BYTES as u64 * 4 {
                    skipped.push(format!(
                        "[skipped: {} ({:.0}KB)]",
                        display_path_for_tool(path, workspace),
                        meta.len() as f64 / 1024.0
                    ));
                    return true;
                }

                let content = match std::fs::read(path) {
                    Ok(bytes) => match decode_text(&bytes) {
                        Some(c) => c,
                        None => return true,
                    },
                    Err(_) => return true,
                };

                let lines: Vec<&str> = content.lines().collect();
                let mut file_matches: Vec<String> = Vec::new();

                for (i, line) in lines.iter().enumerate() {
                    if total_matches + file_matches.len() >= max_results {
                        break;
                    }
                    if re.is_match(line) {
                        let line_num = i + 1;
                        let shown = display_path_for_tool(path, workspace);
                        if context_lines == 0 {
                            file_matches.push(format!(
                                "  {shown}:{line_num}:{}",
                                line.trim_end()
                            ));
                        } else {
                            let start = i.saturating_sub(context_lines);
                            let end = (i + context_lines + 1).min(lines.len());
                            for (j, ctx_line) in lines[start..end].iter().enumerate() {
                                let actual_line = start + j + 1;
                                let marker = if actual_line == line_num { ">" } else { " " };
                                file_matches.push(format!(
                                    "  {shown}:{marker}{actual_line}: {}",
                                    ctx_line.trim_end()
                                ));
                            }
                            file_matches.push(String::new());
                        }
                    }
                }

                if !file_matches.is_empty() {
                    total_matches += file_matches.len();
                    results.push(format!("{}:", display_path_for_tool(path, workspace)));
                    results.extend(file_matches);
                }

                true
            },
        );

        if results.is_empty() {
            let filter_note = match (ext_filter, include) {
                (Some(exts), _) => format!(" (extensions: {})", exts.join(", ")),
                (None, Some(i)) => format!(" (filter: {})", i),
                _ => String::new(),
            };
            let ignored_note = format_skipped_dirs_note(&skipped_dirs);
            return Ok(ToolResult::ok(format!(
                "No matches found for '{}' under {}{}{}",
                pattern,
                display_path_for_tool(root, workspace),
                filter_note,
                ignored_note
            )));
        }

        let mut body = format!(
            "Found {} match(es) for '{}' under {}:\n{}",
            total_matches,
            pattern,
            display_path_for_tool(root, workspace),
            results.join("\n")
        );
        if !skipped.is_empty() {
            body.push_str("\n\nSkipped large files:\n");
            body.push_str(&skipped.join("\n"));
        }
        Ok(ToolResult::ok(body))
    }
}

fn file_name_text(path: &Path) -> String {
    path.file_name().map(os_text).unwrap_or_default()
}

fn extension_text(path: &Path) -> String {
    path.extension()
        .map(os_text)
        .unwrap_or_default()
        .to_lowercase()
}

fn format_skipped_dirs_note(skipped_dirs: &[String]) -> String {
    if skipped_dirs.is_empty() {
        String::new()
    } else {
        format!(
            " (skipped excluded directories: {}; pass include_ignored=true to search inside them)",
            skipped_dirs.join(", ")
        )
    }
}

/// Walk a directory recursively, calling `visitor` for each entry.
/// Returns early if visitor returns false.
#[allow(dead_code)]
fn walk_dir<F: FnMut(&Path) -> bool>(dir: &Path, depth: usize, max_depth: usize, visitor: &mut F) {
    let mut skipped = Vec::new();
    walk_dir_opts(dir, depth, max_depth, false, &mut skipped, visitor);
}

fn walk_dir_opts<F: FnMut(&Path) -> bool>(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    include_ignored: bool,
    skipped_dirs: &mut Vec<String>,
    visitor: &mut F,
) {
    if depth > max_depth {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = path.is_dir();
        if is_dir {
            if let Some(dir_name) = path.file_name().map(os_text) {
                let dir_name = dir_name.as_str();
                if dir_name.eq_ignore_ascii_case(".git") {
                    continue;
                }
                if !include_ignored && is_excluded_search_directory(dir_name) {
                    if !skipped_dirs.iter().any(|d| d.eq_ignore_ascii_case(dir_name)) {
                        skipped_dirs.push(dir_name.to_string());
                    }
                    continue;
                }
            }
        }
        if !visitor(&path) {
            return;
        }
        if is_dir {
            walk_dir_opts(&path, depth + 1, max_depth, include_ignored, skipped_dirs, visitor);
        }
    }
}

/// Convert a glob pattern to a regex string.
/// Supports: * (match any chars except /), ** (match any path), ? (match single char)
fn glob_to_regex(glob: &str) -> String {
    let mut regex = String::from("(?i)^");
    let chars: Vec<char> = glob.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                regex.push_str(".*");
                i += 2;
                // Skip trailing slash after **
                if i < chars.len() && (chars[i] == '/' || chars[i] == '\\') {
                    i += 1;
                }
            }
            '*' => {
                regex.push_str("[^/\\\\]*");
                i += 1;
            }
            '?' => {
                regex.push('.');
                i += 1;
            }
            '.' => {
                regex.push_str("\\.");
                i += 1;
            }
            c => {
                // Escape other regex metacharacters
                if "()[]{}+^$|\\".contains(c) {
                    regex.push('\\');
                }
                regex.push(c);
                i += 1;
            }
        }
    }
    regex.push('$');
    regex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_dir_skips_excluded_directories_before_visiting_entries() {
        struct TempRoot(PathBuf);
        impl Drop for TempRoot {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let root =
            std::env::temp_dir().join(format!("file-search-excludes-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).expect("create file search fixture");
        let _root = TempRoot(root.clone());
        std::fs::create_dir(root.join("src")).expect("create included directory");
        std::fs::write(root.join("src/visible.rs"), "visible").expect("write visible file");
        for excluded in EXCLUDED_SEARCH_DIRECTORY_NAMES {
            let directory = root.join(excluded);
            std::fs::create_dir(&directory).expect("create excluded directory");
            std::fs::write(directory.join("hidden.rs"), "must not visit")
                .expect("write excluded file");
        }

        let mut visited = Vec::new();
        walk_dir(&root, 0, 10, &mut |path| {
            visited.push(path.to_path_buf());
            true
        });

        assert!(visited.iter().any(|path| path.ends_with("visible.rs")));
        for excluded in EXCLUDED_SEARCH_DIRECTORY_NAMES {
            assert!(
                !visited
                    .iter()
                    .any(|path| path.components().any(|component| {
                        component
                            .as_os_str()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(excluded)
                    })),
                "excluded directory {excluded} or its contents reached the visitor"
            );
        }
    }

    #[test]
    fn include_ignored_searches_excluded_dirs_and_default_reports_skipped_dirs() {
        let root = std::env::temp_dir().join(format!(
            "file-search-include-ignored-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("node_modules/pkg/index.d.ts"), "export type Foo = string;")
            .unwrap();

        let tool = FileSearchTool;
        let res_default = tool
            .do_grep("export type Foo", &root, &root, None, None, 10, 5, false, 0, false)
            .unwrap();
        assert!(res_default.content.contains("No matches found"));
        assert!(res_default.content.contains("skipped excluded directories: node_modules"));

        let res_ignored = tool
            .do_grep("export type Foo", &root, &root, None, None, 10, 5, false, 0, true)
            .unwrap();
        assert!(res_ignored.content.contains("Found 1 match"));
        assert!(res_ignored.content.contains("index.d.ts"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
