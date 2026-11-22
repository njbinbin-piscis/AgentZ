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

/// Tool-state directories and secret files that searches never descend into or read.
pub fn is_secret_or_metadata(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(n.as_str(), ".agentz" | ".codez" | ".piscis" | ".secret_key" | ".env") || n.starts_with(".env.")
}

/// Host-state entries inside `.agentz` that searches must not read even when the
/// agent explicitly targets its own working area: databases, config, credentials,
/// indexes and logs. Working documents (REPO_MEMORY.md, AGENT_CODING_BRIEF.md,
/// plans/*.md) are NOT host state and stay searchable.
pub fn is_agentz_host_state(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".db")
        || n.ends_with(".db-wal")
        || n.ends_with(".db-shm")
        || n.ends_with(".sqlite")
        || n.ends_with(".json")
        || n.ends_with(".log")
        || n.starts_with(".env")
        || n == ".secret_key"
}

pub fn is_excluded_search_directory(name: &str) -> bool {
    EXCLUDED_SEARCH_DIRECTORY_NAMES
        .iter()
        .any(|excluded| excluded.eq_ignore_ascii_case(name))
}

/// Detailed usage kept out of the always-sent minimal description; returned
/// only with a call-shape error.
const FILE_SEARCH_HELP: &str = "file_search usage:\n\
- action=glob: find files by NAME. pattern supports * (within a name), ** (any path segments), ? (one char). Examples: '*.rs', '**/*.json', 'config*'.\n\
- action=grep: search file CONTENTS by regex; returns file:line matches. Filter with include='*.rs' or file_extensions=['rs'].\n\
- Regex engine: classes, alternation, anchors, groups. NO look-around ((?=...) (?!...) (?<=...) (?<!...)) and NO backreferences. 'assignment, not comparison' = `=[^=]`.\n\
- path must be a DIRECTORY (default: workspace root). To search one file, pass its parent directory and set include to the file name, or use file_read.\n\
- Examples: {action:glob, pattern:'**/*.py', path:'src'}; {action:grep, pattern:'TBRuntime', path:'.'}; {action:grep, pattern:'password', include:'*.ini', path:'src'}.\n\
- Other options: max_results (50, max 500), max_depth (10), case_sensitive, context_lines, include_ignored.";

pub struct FileSearchTool;

#[async_trait]
impl Tool for FileSearchTool {
    fn name(&self) -> &str {
        "file_search"
    }

    fn description(&self) -> &str {
        "Search inside a directory. The path parameter accepts only a directory, never a file. \
         Equivalent to the Glob and Grep tools used by Cursor AI. \
         \
         Actions: \
         - 'glob': Find files whose names match a pattern. Pattern supports * (any chars in name), \
           ** (any path segment), ? (single char). Example: '*.rs', '**/*.json', 'config*'. \
         - 'grep': Search file contents for a regex pattern. Returns matching lines with file path and line number. \
           Use 'include' to filter by file extension (e.g. '*.rs', '*.txt'). \
           Regex syntax: the engine supports character classes, alternation, anchors and groups, but NOT look-around \
           ((?=...), (?!...), (?<=...), (?<!...)) or backreferences. For 'assignment not comparison' use `=[^=]`, not `=(?!=)`. \
         \
         path must be a directory. A file path is an error (`Path is not a directory`), not an empty result. \
         To search one file, pass its parent directory and set include to that file name, or use file_read. \
         \
         Tips: \
         - Find .py files: action=glob, pattern=**/*.py, path=src \
         - Find a string: action=grep, pattern=TBRuntime, path=. \
         - Limit by name: action=grep, pattern=password, include=*.ini, path=src \
         Absolute directories must still be inside the authorized workspace (Windows C:\\MyProject, Linux and macOS /home/me/proj)."
    }

    fn description_minimal(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed(
            "Search inside a directory (path must be a directory, default workspace root). \
             action=glob: find files by name pattern. action=grep: regex search of file contents \
             (no look-around/backreferences); filter with include. Detailed usage is returned if a call is malformed.",
        )
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
                    "description": "DIRECTORY ONLY. A file path is invalid and returns `Path is not a directory`. Pass the parent directory, and to limit the search to one file set include to that file name (or use file_read). Relative paths resolve from the workspace root. Defaults to the workspace root when omitted."
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
            None => return Ok(ToolResult::usage_err("Missing required parameter: action", FILE_SEARCH_HELP)),
        };
        let pattern = match input["pattern"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::usage_err("Missing required parameter: pattern", FILE_SEARCH_HELP)),
        };

        let mut root = if let Some(p) = input["path"].as_str() {
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
        // A file passed as `path` is a common slip. For grep, do what was meant:
        // search that one file (parent directory + include = its name).
        let mut file_note: Option<String> = None;
        let mut include_override: Option<String> = None;
        if action == "grep" && root.is_file() {
            if let (Some(parent), Some(name)) = (root.parent(), root.file_name()) {
                file_note = Some(format!(
                    "[path was a file; searched only {} (path must be a directory, so the parent was searched with include={}).]\n",
                    display_path_for_tool(&root, &ctx.workspace_root),
                    name.to_string_lossy()
                ));
                include_override = Some(name.to_string_lossy().into_owned());
                root = parent.to_path_buf();
            }
        }
        if !root.is_dir() {
            let shown = display_path_for_tool(&root, &ctx.workspace_root);
            return Ok(ToolResult::usage_err(
                format!("Path is not a directory: {shown}."),
                FILE_SEARCH_HELP,
            ));
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

        let result = match action {
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
                let include = include_override.as_deref().or(input["include"].as_str());
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
            _ => Ok(ToolResult::usage_err(format!("Unknown action: {}", action), FILE_SEARCH_HELP)),
        };
        match (result, file_note) {
            (Ok(mut res), Some(note)) if !res.is_error => {
                res.content = format!("{note}{}", res.content);
                Ok(res)
            }
            (result, _) => result,
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
                return Ok(ToolResult::usage_err(
                    format!("Invalid glob pattern '{}': {}", pattern, e),
                    FILE_SEARCH_HELP,
                ))
            }
        };

        let mut matches: Vec<String> = Vec::new();
        let mut skipped_dirs: Vec<String> = Vec::new();
        let mut truncated = false;
        walk_dir_opts(
            root,
            0,
            max_depth,
            include_ignored,
            &mut skipped_dirs,
            &mut |path: &Path| {
                if matches.len() >= max_results {
                    truncated = true;
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

        let mut body = format!(
            "Found {} file(s) matching '{}' under {}:\n{}",
            matches.len(),
            pattern,
            display_path_for_tool(root, workspace),
            matches.join("\n")
        );
        if truncated {
            body.push_str(&format!(
                "\n\n[Results truncated at {max_results}: more files match. Narrow with path or a more specific pattern, \
                 or raise max_results (max {MAX_RESULTS}). Do not re-run the same glob expecting different results.]"
            ));
        }
        Ok(ToolResult::ok(body))
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
                    return Ok(ToolResult::usage_err(
                        format!(
                            "Invalid regex '{}': {}{}",
                            pattern,
                            e,
                            regex_rewrite_hint(&e.to_string())
                        ),
                        FILE_SEARCH_HELP,
                    ))
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
        let mut truncated = false;
        let mut files_scanned = 0usize;
        let mut binary_skipped = 0usize;

        walk_dir_opts(
            root,
            0,
            max_depth,
            include_ignored,
            &mut skipped_dirs,
            &mut |path: &Path| {
                if total_matches >= max_results {
                    // The walk stopped with files left unsearched.
                    truncated = true;
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
                        None => {
                            binary_skipped += 1;
                            return true;
                        }
                    },
                    Err(_) => return true,
                };
                files_scanned += 1;

                let lines: Vec<&str> = content.lines().collect();
                let mut file_matches: Vec<String> = Vec::new();

                for (i, line) in lines.iter().enumerate() {
                    if total_matches + file_matches.len() >= max_results {
                        if lines[i..].iter().any(|l| re.is_match(l)) {
                            truncated = true;
                        }
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
            // An empty result is only conclusive when the model knows what was
            // searched. Without this it re-runs variants of the pattern, or reads
            // files one by one, to find out whether anything was skipped.
            let mut coverage = format!(
                " Searched {} text file(s) (max depth {}, case-{}sensitive).",
                files_scanned,
                max_depth,
                if case_sensitive { "" } else { "in" }
            );
            if binary_skipped > 0 {
                coverage.push_str(&format!(" {binary_skipped} binary/undecodable file(s) were not searched."));
            }
            if !skipped.is_empty() {
                coverage.push_str(&format!(
                    " {} file(s) over the size limit were not searched: {}.",
                    skipped.len(),
                    skipped.join(" ")
                ));
            }
            if files_scanned == 0 {
                coverage.push_str(" No files matched the path/filter, so the pattern was never tried; check path, include and file_extensions.");
            } else {
                coverage.push_str(" The search is complete: this is a definitive 'not found' for this pattern and scope. Widen the pattern or path rather than repeating it.");
            }
            return Ok(ToolResult::ok(format!(
                "No matches found for '{}' under {}{}{}.{}",
                pattern,
                display_path_for_tool(root, workspace),
                filter_note,
                ignored_note,
                coverage
            )));
        }

        let mut body = format!(
            "Found {} match(es) for '{}' under {}:\n{}",
            total_matches,
            pattern,
            display_path_for_tool(root, workspace),
            results.join("\n")
        );
        if truncated {
            body.push_str(&format!(
                "\n\n[Results truncated at {max_results}: more matches exist. The list above is the first {max_results} in path order. \
                 Narrow with path, include or file_extensions, use a more specific pattern, or raise max_results (max {MAX_RESULTS}). \
                 Do not re-run the same search expecting different results.]"
            ));
        }
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
    // Directory order is arbitrary and differs between runs and platforms. A
    // result list cut at `max_results` must be the same list every time, or the
    // model cannot tell a narrower query from a reshuffled one and keeps retrying.
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let is_dir = path.is_dir();
        let entry_name = path.file_name().map(os_text).unwrap_or_default();
        // `.agentz` is the agent's own working area: searches may descend into it
        // (repo memory, plans) but never read host-state entries inside. Other
        // tool-state/secret directories stay fully skipped.
        if is_secret_or_metadata(&entry_name) && !entry_name.eq_ignore_ascii_case(".agentz") {
            continue;
        }
        let in_agentz = dir.file_name().map(os_text).as_deref() == Some(".agentz");
        if in_agentz && is_agentz_host_state(&entry_name) {
            continue;
        }
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

/// #30: the `regex` crate has no look-around/backreferences. When a pattern fails
/// for that reason, append a concrete rewrite hint so the agent does not burn
/// rounds approximating the same intent with character classes.
fn regex_rewrite_hint(err: &str) -> String {
    let lower = err.to_ascii_lowercase();
    if lower.contains("look-around") || lower.contains("look-ahead") || lower.contains("look-behind")
    {
        return " The regex engine does not support look-around. Rewrite without it: \
                e.g. 'assignment not comparison' → search `=[^=]` (or search `=` then filter); \
                'word not followed by X' → match the word and exclude X in a second pass."
            .to_string();
    }
    if lower.contains("backreference") {
        return " The regex engine does not support backreferences. Match the repeated \
                structure with an explicit character class or alternation instead."
            .to_string();
    }
    String::new()
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

    #[tokio::test]
    async fn glob_with_a_file_path_is_an_error_not_no_matches() {
        use crate::agent::tool::{Tool, ToolContext, ToolSettings};
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let root =
            std::env::temp_dir().join(format!("file-search-not-dir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).expect("create file search fixture");
        let file = root.join("notes.md");
        std::fs::write(&file, "hello needle").expect("write file");
        let ctx = ToolContext {
            session_id: "file-search".into(),
            workspace_root: root.clone(),
            bypass_permissions: false,
            settings: Arc::new(ToolSettings::default()),
            max_iterations: None,
            memory_owner_id: "test".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        };
        let file_path = file.to_string_lossy().to_string();
        let tool = FileSearchTool;
        for (action, pattern) in [("glob", "*.md")] {
            let result = tool
                .call(
                    json!({ "action": action, "pattern": pattern, "path": file_path }),
                    &ctx,
                )
                .await
                .unwrap();
            assert!(result.is_error, "{action}: {}", result.content);
            assert!(
                result.content.contains("Path is not a directory"),
                "{action}: {}",
                result.content
            );
            assert!(
                !result.content.contains("No matches"),
                "{action}: {}",
                result.content
            );
            assert!(
                !result.content.contains("No files found"),
                "{action}: {}",
                result.content
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// #28: searching the agent's own `.agentz` working area finds working
    /// documents (repo memory, plans) but never descends into host-state files.
    #[tokio::test]
    async fn grep_agentz_finds_working_docs_not_host_state() {
        use crate::agent::tool::{Tool, ToolContext, ToolSettings};
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let root =
            std::env::temp_dir().join(format!("file-search-agentz-{}", uuid::Uuid::new_v4()));
        let az = root.join(".agentz");
        std::fs::create_dir_all(az.join("plans")).unwrap();
        std::fs::write(az.join("REPO_MEMORY.md"), "needle working doc").unwrap();
        std::fs::write(az.join("plans").join("p.md"), "needle plan").unwrap();
        std::fs::write(az.join("journal.db"), "needle secret state").unwrap();
        let ctx = ToolContext {
            session_id: "file-search".into(),
            workspace_root: root.clone(),
            bypass_permissions: false,
            settings: Arc::new(ToolSettings::default()),
            max_iterations: None,
            memory_owner_id: "test".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        };
        let result = FileSearchTool
            .call(
                json!({ "action": "grep", "pattern": "needle", "path": ".agentz" }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("REPO_MEMORY.md"), "{}", result.content);
        assert!(result.content.contains("p.md"), "{}", result.content);
        assert!(!result.content.contains("journal.db"), "{}", result.content);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// #30: a look-around regex must return the engine limitation PLUS a concrete
    /// rewrite hint, not a bare error that sends the agent into近似-search rounds.
    #[tokio::test]
    async fn grep_lookaround_error_includes_rewrite_hint() {
        use crate::agent::tool::{Tool, ToolContext, ToolSettings};
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let root =
            std::env::temp_dir().join(format!("file-search-lookaround-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.rs"), "let x = 1;").unwrap();
        let ctx = ToolContext {
            session_id: "file-search".into(),
            workspace_root: root.clone(),
            bypass_permissions: false,
            settings: Arc::new(ToolSettings::default()),
            max_iterations: None,
            memory_owner_id: "test".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        };
        let result = FileSearchTool
            .call(
                json!({ "action": "grep", "pattern": r"=(?!=)", "path": "." }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(result.is_error, "{}", result.content);
        // Still reports the limitation...
        assert!(result.content.contains("look-around"), "{}", result.content);
        // ...but now also gives a usable rewrite path.
        assert!(result.content.contains("=[^=]"), "{}", result.content);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn search_ctx(root: &std::path::Path) -> crate::agent::tool::ToolContext {
        use crate::agent::tool::{ToolContext, ToolSettings};
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        ToolContext {
            session_id: "file-search".into(),
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

    /// A list cut at max_results must say so, otherwise the model re-runs the
    /// search (or reads files) to find out whether it saw everything.
    #[tokio::test]
    async fn grep_and_glob_say_when_results_were_truncated() {
        let root = std::env::temp_dir().join(format!("file-search-trunc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        for i in 0..6 {
            std::fs::write(root.join(format!("f{i}.txt")), "needle\n").unwrap();
        }
        let ctx = search_ctx(&root);
        let grep = FileSearchTool
            .call(json!({"action": "grep", "pattern": "needle", "path": ".", "max_results": 3}), &ctx)
            .await
            .unwrap();
        assert!(grep.content.contains("Found 3 match"), "{}", grep.content);
        assert!(grep.content.contains("truncated"), "{}", grep.content);
        // Deterministic: the same first three files every time.
        assert!(grep.content.contains("f0.txt") && grep.content.contains("f2.txt"), "{}", grep.content);
        assert!(!grep.content.contains("f5.txt"), "{}", grep.content);
        let glob = FileSearchTool
            .call(json!({"action": "glob", "pattern": "*.txt", "path": ".", "max_results": 3}), &ctx)
            .await
            .unwrap();
        assert!(glob.content.contains("truncated"), "{}", glob.content);
        // Complete results carry no truncation note.
        let all = FileSearchTool
            .call(json!({"action": "grep", "pattern": "needle", "path": ".", "max_results": 50}), &ctx)
            .await
            .unwrap();
        assert!(!all.content.contains("truncated"), "{}", all.content);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// "No matches" must say what was searched so it reads as a conclusive answer.
    #[tokio::test]
    async fn grep_no_match_reports_what_was_searched() {
        let root = std::env::temp_dir().join(format!("file-search-empty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.txt"), "hello\n").unwrap();
        std::fs::write(root.join("b.bin"), [0u8, 1, 2, 3]).unwrap();
        let ctx = search_ctx(&root);
        let res = FileSearchTool
            .call(json!({"action": "grep", "pattern": "absent", "path": "."}), &ctx)
            .await
            .unwrap();
        assert!(res.content.contains("No matches found"), "{}", res.content);
        assert!(res.content.contains("Searched 1 text file"), "{}", res.content);
        assert!(res.content.contains("binary"), "{}", res.content);
        assert!(res.content.contains("definitive"), "{}", res.content);
        let none = FileSearchTool
            .call(json!({"action": "grep", "pattern": "x", "path": ".", "include": "*.nothing"}), &ctx)
            .await
            .unwrap();
        assert!(none.content.contains("never tried"), "{}", none.content);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A malformed call carries the detailed usage (kept out of the minimal
    /// description) and is classified correctable; an environment failure is not.
    #[tokio::test]
    async fn call_errors_carry_help_but_environment_errors_do_not() {
        let root = std::env::temp_dir().join(format!("file-search-help-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let ctx = search_ctx(&root);
        let bad = FileSearchTool
            .call(json!({"action": "grep", "pattern": "(?=x)", "path": "."}), &ctx)
            .await
            .unwrap();
        assert!(bad.is_error);
        assert!(bad.content.contains("[usage_help]") && bad.content.contains("NO look-around"), "{}", bad.content);
        assert!(crate::agent::message_utils::is_correctable_failure(&bad.content));
        let missing = FileSearchTool
            .call(json!({"action": "glob", "pattern": "*", "path": "nope"}), &ctx)
            .await
            .unwrap();
        assert!(missing.is_error);
        assert!(!crate::agent::message_utils::is_correctable_failure(&missing.content));
        assert!(FileSearchTool.description_minimal().len() < FileSearchTool.description().len() / 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A file passed as `path` to grep is searched directly instead of failing.
    #[tokio::test]
    async fn grep_with_a_file_path_searches_that_file() {
        let root = std::env::temp_dir().join(format!("file-search-fp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.txt"), "needle here\n").unwrap();
        std::fs::write(root.join("b.txt"), "needle there\n").unwrap();
        let ctx = search_ctx(&root);
        let res = FileSearchTool
            .call(json!({"action": "grep", "pattern": "needle", "path": "a.txt"}), &ctx)
            .await
            .unwrap();
        assert!(!res.is_error, "{}", res.content);
        assert!(res.content.contains("path was a file"), "{}", res.content);
        assert!(res.content.contains("a.txt") && !res.content.contains("b.txt:"), "{}", res.content);
        let _ = std::fs::remove_dir_all(&root);
    }}
