//! IDE-domain commands — file tree, file I/O, git integration, terminal PTY,
//! and file-change event bridge for the embedded Monaco Editor IDE.
//!
//! All commands are registered as Tauri commands by `app::bootstrap`.

use piscis_kernel::proc::tokio_command;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write as StdWrite;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio::process::Command;
use tokio::time::timeout;

use crate::lsp::manager::LspManager;
use crate::state::AppState;

// ─── Types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
    pub children: Option<Vec<FileNode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub encoding: String,
    pub is_binary: bool,
    pub size: u64,
    pub language: Option<String>,
    /// When set, the frontend should render a media preview (`data:` URL).
    #[serde(default)]
    pub preview_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub text: String,
    pub context_before: Option<String>,
    pub context_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFileStatus {
    pub path: String,
    pub status: String, // modified, added, deleted, untracked, renamed
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub path: String,
    pub original: String,
    pub modified: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_koi: bool,
    pub last_commit: Option<String>,
    pub last_commit_time: Option<String>,
}

// ─── File Tree ─────────────────────────────────────────────────────────────

/// List files in a project directory as a tree structure.
/// Respects .gitignore patterns. Returns nested FileNode tree.
#[tauri::command]
pub async fn ide_list_files(
    project_dir: String,
    depth: Option<usize>,
) -> Result<Vec<FileNode>, String> {
    if let Some(remote_root) = crate::remote::resolve(&project_dir).await? {
        let v = crate::remote::call(
            "fs.tree",
            serde_json::json!({ "path": remote_root, "depth": depth.unwrap_or(10) }),
        )
        .await?;
        let mut nodes: Vec<FileNode> = serde_json::from_value(v).map_err(|e| e.to_string())?;
        sort_file_nodes(&mut nodes);
        return Ok(nodes);
    }
    let root = PathBuf::from(&project_dir);
    if !root.exists() {
        return Err(format!("Directory not found: {}", project_dir));
    }
    let max_depth = depth.unwrap_or(10);
    tokio::task::spawn_blocking(move || {
        let ignore_patterns = load_gitignore_patterns(&root);
        let mut nodes = build_file_tree(&root, &root, 0, max_depth, &ignore_patterns)
            .map_err(|e| format!("Failed to list files: {}", e))?;
        sort_file_nodes(&mut nodes);
        Ok(nodes)
    })
    .await
    .map_err(|e| format!("File listing task failed: {e}"))?
}

fn load_gitignore_patterns(root: &Path) -> Vec<String> {
    let gitignore = root.join(".gitignore");
    if gitignore.exists() {
        std::fs::read_to_string(&gitignore)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(|l| l.trim().to_string())
            .collect()
    } else {
        vec![]
    }
}

fn is_ignored(name: &str, path: &Path, patterns: &[String]) -> bool {
    let rel = path.to_string_lossy().replace('\\', "/");
    crate::path_filter::is_ignored_tree_entry(name, &rel, patterns)
}

fn build_file_tree(
    dir: &Path,
    root: &Path,
    current_depth: usize,
    max_depth: usize,
    patterns: &[String],
) -> std::io::Result<Vec<FileNode>> {
    if current_depth >= max_depth {
        return Ok(vec![]);
    }

    let mut nodes = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(vec![]),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();

        if is_ignored(&name, &path, patterns) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let modified = metadata.modified().ok().map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.to_rfc3339()
        });

        if metadata.is_dir() {
            let children = build_file_tree(&path, root, current_depth + 1, max_depth, patterns)?;
            nodes.push(FileNode {
                name,
                path: relative,
                is_dir: true,
                size: 0,
                modified,
                children: Some(children),
            });
        } else {
            nodes.push(FileNode {
                name,
                path: relative,
                is_dir: false,
                size: metadata.len(),
                modified,
                children: None,
            });
        }
    }

    Ok(nodes)
}

fn sort_file_nodes(nodes: &mut [FileNode]) {
    nodes.sort_by(|a, b| {
        // Directories first, then alphabetical
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    for node in nodes.iter_mut() {
        if let Some(ref mut children) = node.children {
            sort_file_nodes(children);
        }
    }
}

// ─── File Read / Write ─────────────────────────────────────────────────────

/// Read a file's content with encoding detection.
#[tauri::command]
pub async fn ide_read_file(
    state: tauri::State<'_, crate::state::AppState>,
    path: String,
) -> Result<FileContent, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    if let Some(remote_path) = super::remote::resolve_remote(&state, &path).await? {
        let (raw, size) = super::remote::read_remote_text(&state, &remote_path).await?;
        let (content, is_binary) = match String::from_utf8(raw) {
            Ok(text) => (text, false),
            Err(_) => (String::new(), true),
        };
        return Ok(FileContent {
            path,
            content,
            encoding: if is_binary { "binary" } else { "utf-8" }.to_string(),
            is_binary,
            size,
            language: None,
            preview_data: None,
        });
    }

    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    let metadata = std::fs::metadata(&file_path).map_err(|e| e.to_string())?;
    if metadata.is_dir() {
        return Err(format!("Cannot preview directory: {}", path));
    }
    let size = metadata.len();

    let raw = crate::bounded_read::read(&file_path, 10 * 1024 * 1024)
        .map_err(|e| format!("Cannot preview {path} (maximum 10 MiB): {e}"))?;

    // Image preview — common raster formats + SVG.
    if let Some(mime) = preview_mime_for_path(&file_path) {
        if mime == "image/svg+xml" {
            if let Ok(text) = std::str::from_utf8(&raw) {
                let encoded = STANDARD.encode(text.as_bytes());
                return Ok(FileContent {
                    path: path.clone(),
                    content: String::new(),
                    encoding: "svg".to_string(),
                    is_binary: true,
                    size,
                    language: None,
                    preview_data: Some(format!("data:image/svg+xml;base64,{encoded}")),
                });
            }
        } else {
            let encoded = STANDARD.encode(&raw);
            return Ok(FileContent {
                path: path.clone(),
                content: String::new(),
                encoding: "binary".to_string(),
                is_binary: true,
                size,
                language: None,
                preview_data: Some(format!("data:{mime};base64,{encoded}")),
            });
        }
    }

    // Binary detection: read first 8KB and check for null bytes
    let is_binary = raw[..raw.len().min(8192)].contains(&0);

    if is_binary {
        return Ok(FileContent {
            path: path.clone(),
            content: format!("[Binary file, {} bytes]", size),
            encoding: "binary".to_string(),
            is_binary: true,
            size,
            language: None,
            preview_data: None,
        });
    }

    // Encoding detection
    let (content, encoding) = decode_content(&raw);

    let language = detect_language(&file_path);

    Ok(FileContent {
        path,
        content,
        encoding,
        is_binary: false,
        size,
        language,
        preview_data: None,
    })
}

fn preview_mime_for_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "ico" => Some("image/x-icon"),
        "svg" => Some("image/svg+xml"),
        "pdf" => Some("application/pdf"),
        _ => None,
    }
}

fn decode_content(raw: &[u8]) -> (String, String) {
    // Check BOM
    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        // UTF-8 BOM
        let s = String::from_utf8_lossy(&raw[3..]).to_string();
        return (s, "utf-8-bom".to_string());
    }
    if raw.starts_with(&[0xFF, 0xFE]) {
        // UTF-16 LE
        let s = encoding_rs::UTF_16LE.decode(&raw[2..]).0.to_string();
        return (s, "utf-16le".to_string());
    }
    if raw.starts_with(&[0xFE, 0xFF]) {
        // UTF-16 BE
        let s = encoding_rs::UTF_16BE.decode(&raw[2..]).0.to_string();
        return (s, "utf-16be".to_string());
    }

    // Try UTF-8 first
    match std::str::from_utf8(raw) {
        Ok(s) => (s.to_string(), "utf-8".to_string()),
        Err(_) => {
            // Fallback to GBK (common in Chinese projects)
            let (s, _, _) = encoding_rs::GBK.decode(raw);
            (s.to_string(), "gbk".to_string())
        }
    }
}

fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;
    let lang = match ext {
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "rs" => "rust",
        "py" | "pyi" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "r" => "r",
        "lua" => "lua",
        "sh" | "bash" | "zsh" => "shellscript",
        "ps1" | "psm1" | "psd1" => "powershell",
        "bat" | "cmd" => "bat",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "less" => "less",
        "md" | "markdown" => "markdown",
        "sql" => "sql",
        "graphql" | "gql" => "graphql",
        "dockerfile" => "dockerfile",
        "makefile" => "makefile",
        "cmake" => "cmake",
        "proto" => "protobuf",
        _ => return None,
    };
    Some(lang.to_string())
}

/// Write content to a file. Creates parent directories if needed.
#[tauri::command]
pub async fn ide_write_file(
    state: tauri::State<'_, crate::state::AppState>,
    path: String,
    content: String,
) -> Result<(), String> {
    if let Some(remote_path) = super::remote::resolve_remote(&state, &path).await? {
        return super::remote::write_remote(&state, &remote_path, content.as_bytes()).await;
    }
    let file_path = PathBuf::from(&path);
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }
    std::fs::write(&file_path, content.as_bytes()).map_err(|e| format!("Failed to write: {}", e))
}

/// Perform file actions: create_file, create_dir, delete, rename.
#[tauri::command]
pub async fn ide_file_action(
    path: String,
    action: String,
    new_path: Option<String>,
) -> Result<(), String> {
    if let Some(remote_path) = crate::remote::resolve(&path).await? {
        let (method, params) = match action.as_str() {
            "create_file" => ("fs.writeFile", serde_json::json!({ "path": remote_path, "text": "" })),
            "create_dir" => ("fs.mkdir", serde_json::json!({ "path": remote_path })),
            "delete" => ("fs.delete", serde_json::json!({ "path": remote_path, "recursive": true })),
            "rename" => {
                let target = new_path.ok_or("rename requires 'new_path' parameter")?;
                let to = crate::remote::resolve(&target).await?.unwrap_or(target);
                ("fs.rename", serde_json::json!({ "from": remote_path, "to": to }))
            }
            _ => return Err(format!("Unknown action: {}", action)),
        };
        return crate::remote::call(method, params).await.map(|_| ());
    }
    let file_path = PathBuf::from(&path);

    match action.as_str() {
        "create_file" => {
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directories: {}", e))?;
            }
            std::fs::write(&file_path, "").map_err(|e| format!("Failed to create file: {}", e))
        }
        "create_dir" => std::fs::create_dir_all(&file_path)
            .map_err(|e| format!("Failed to create directory: {}", e)),
        "delete" => {
            if file_path.is_dir() {
                std::fs::remove_dir_all(&file_path)
                    .map_err(|e| format!("Failed to delete directory: {}", e))
            } else {
                std::fs::remove_file(&file_path)
                    .map_err(|e| format!("Failed to delete file: {}", e))
            }
        }
        "rename" => {
            let target = new_path.ok_or("rename requires 'new_path' parameter")?;
            let target_path = PathBuf::from(&target);
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directories: {}", e))?;
            }
            std::fs::rename(&file_path, &target_path)
                .map_err(|e| format!("Failed to rename: {}", e))
        }
        _ => Err(format!("Unknown action: {}", action)),
    }
}

/// Full-text search across project files using ripgrep if available,
/// falling back to a simple Rust implementation.
#[tauri::command]
pub async fn ide_search_files(
    project_dir: String,
    query: String,
    file_pattern: Option<String>,
    case_sensitive: Option<bool>,
    whole_word: Option<bool>,
    use_regex: Option<bool>,
    exclude_pattern: Option<String>,
) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    if let Some(remote_root) = crate::remote::resolve(&project_dir).await? {
        let v = crate::remote::call(
            "search",
            serde_json::json!({
                "root": remote_root,
                "query": query,
                "filePattern": file_pattern,
                "excludePattern": exclude_pattern,
                "caseSensitive": case_sensitive.unwrap_or(false),
                "wholeWord": whole_word.unwrap_or(false),
                "useRegex": use_regex.unwrap_or(false),
                "maxResults": 1000,
            }),
        )
        .await?;
        return serde_json::from_value(v).map_err(|e| e.to_string());
    }
    let root = PathBuf::from(&project_dir);
    if !root.exists() {
        return Err(format!("Directory not found: {}", project_dir));
    }

    let case = case_sensitive.unwrap_or(false);
    let word = whole_word.unwrap_or(false);
    let regex = use_regex.unwrap_or(false);
    let max_results = 1000;
    let mut results = Vec::new();

    // Try ripgrep first
    let rg_result = try_ripgrep(
        &root,
        &query,
        file_pattern.as_deref(),
        exclude_pattern.as_deref(),
        case,
        word,
        regex,
        max_results,
    )
    .await;
    match rg_result {
        Ok(rg_results) => {
            eprintln!(
                "[ide_search] ripgrep ok: {} results for {:?} in {}",
                rg_results.len(),
                query,
                project_dir,
            );
            return Ok(rg_results);
        }
        Err(e) => {
            eprintln!(
                "[ide_search] ripgrep unavailable ({}) — using built-in fallback",
                e,
            );
        }
    }

    // Fallback: simple Rust search
    search_dir_recursive(
        &root,
        &root,
        &query,
        case,
        &file_pattern,
        &mut results,
        max_results,
    )?;
    eprintln!(
        "[ide_search] fallback found {} results for {:?} in {}",
        results.len(),
        query,
        project_dir,
    );
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
async fn try_ripgrep(
    root: &Path,
    query: &str,
    file_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
    max_results: usize,
) -> Result<Vec<SearchResult>, String> {
    let mut cmd = tokio_command("rg");
    cmd.arg("--json")
        .arg("--max-count")
        .arg("50")
        .arg("--max-filesize")
        .arg("1M");

    if !case_sensitive {
        cmd.arg("-i");
    }
    if whole_word {
        cmd.arg("-w");
    }
    if !use_regex {
        // Treat the query as a literal string, not a regex.
        cmd.arg("-F");
    }
    if let Some(pat) = file_pattern {
        if !pat.trim().is_empty() {
            cmd.arg("--glob").arg(pat);
        }
    }
    if let Some(ex) = exclude_pattern {
        if !ex.trim().is_empty() {
            // Support comma/space separated exclude globs.
            for g in ex.split([',', ' ']).filter(|s| !s.trim().is_empty()) {
                let g = g.trim();
                let neg = if g.starts_with('!') {
                    g.to_string()
                } else {
                    format!("!{g}")
                };
                cmd.arg("--glob").arg(neg);
            }
        }
    }
    cmd.arg("--").arg(query).arg(root.as_os_str());
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = timeout(Duration::from_secs(30), cmd.output())
        .await
        .map_err(|_| "Search timed out")?
        .map_err(|e| format!("ripgrep failed: {}", e))?;

    if !output.status.success() && output.status.code() != Some(1) {
        return Err("ripgrep error".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in stdout.lines() {
        if results.len() >= max_results {
            break;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val["type"].as_str() == Some("match") {
                let data = &val["data"];
                let path = data["path"]["text"].as_str().unwrap_or("");
                let line_num = data["line_number"].as_u64().unwrap_or(0) as usize;
                let text = data["lines"]["text"]
                    .as_str()
                    .unwrap_or("")
                    .trim_end()
                    .to_string();

                // 1-based column from the first submatch byte offset (0 if none).
                let column = data["submatches"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|m| m["start"].as_u64())
                    .map(|s| s as usize + 1)
                    .unwrap_or(0);

                // Extract relative path
                let rel_path = path
                    .strip_prefix(&root.to_string_lossy().to_string())
                    .unwrap_or(path)
                    .trim_start_matches('/')
                    .to_string();

                results.push(SearchResult {
                    path: rel_path,
                    line: line_num,
                    column,
                    text,
                    context_before: None,
                    context_after: None,
                });
            }
        }
    }

    Ok(results)
}

fn search_dir_recursive(
    dir: &Path,
    root: &Path,
    query: &str,
    case_sensitive: bool,
    file_pattern: &Option<String>,
    results: &mut Vec<SearchResult>,
    max_results: usize,
) -> Result<(), String> {
    if results.len() >= max_results {
        return Ok(());
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    let query_cmp = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    for entry in entries {
        if results.len() >= max_results {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();

        // Skip common non-source dirs and hidden dirs
        if matches!(
            name.as_str(),
            ".git"
                | "node_modules"
                | "__pycache__"
                | "target"
                | "dist"
                | "build"
                | ".koi-worktrees"
                | ".qoder"
                | ".next"
                | ".turbo"
                | ".cache"
                | ".idea"
                | ".vscode"
        ) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.is_dir() {
            search_dir_recursive(
                &path,
                root,
                query,
                case_sensitive,
                file_pattern,
                results,
                max_results,
            )?;
        } else if metadata.len() < 1_000_000 {
            // Skip files > 1MB
            if let Some(ref pat) = file_pattern {
                let pat_clean = pat.trim_start_matches('*');
                if !name.ends_with(pat_clean) {
                    continue;
                }
            }

            if let Ok(content) = std::fs::read_to_string(&path) {
                let rel_path = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                for (line_num, line_text) in content.lines().enumerate() {
                    if results.len() >= max_results {
                        break;
                    }
                    let line_cmp = if case_sensitive {
                        line_text.to_string()
                    } else {
                        line_text.to_lowercase()
                    };
                    if let Some(byte_idx) = line_cmp.find(&query_cmp) {
                        results.push(SearchResult {
                            path: rel_path.clone(),
                            line: line_num + 1,
                            column: byte_idx + 1,
                            text: line_text.to_string(),
                            context_before: None,
                            context_after: None,
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

#[path = "git_workspace.rs"]
mod git_workspace;
pub use git_workspace::GitRepoSnapshot;

// ─── Git Operations ────────────────────────────────────────────────────────

type GitStatusRun =
    futures::future::Shared<futures::future::BoxFuture<'static, Result<std::sync::Arc<Vec<u8>>, String>>>;

/// At most one `git status` per repo at a time; concurrent callers share its
/// output. A refresh storm from the UI would otherwise pile up git processes.
fn git_status_runs() -> &'static std::sync::Mutex<HashMap<PathBuf, GitStatusRun>> {
    static RUNS: std::sync::OnceLock<std::sync::Mutex<HashMap<PathBuf, GitStatusRun>>> = std::sync::OnceLock::new();
    RUNS.get_or_init(Default::default)
}

async fn git_status_at(repo: &Path, workspace: &Path) -> Result<Vec<GitFileStatus>, String> {
    use futures::FutureExt;
    let key = repo.to_path_buf();
    let run = {
        let mut runs = git_status_runs().lock().unwrap();
        runs.entry(key.clone())
            .or_insert_with(|| {
                let repo = key.clone();
                async move {
                    let out = run_git_cmd_bytes(&repo, &["status", "--porcelain=v1", "-z", "-uall"])
                        .await
                        .map(std::sync::Arc::new)
                        .map_err(|e| format!("git status failed: {}", e));
                    git_status_runs().lock().unwrap().remove(&repo);
                    out
                }
                .boxed()
                .shared()
            })
            .clone()
    };
    let output = run.await?;
    let rel = git_workspace::repo_root_rel(workspace, repo);
    Ok(git_workspace::parse_git_status_output(&output, &rel))
}

async fn git_branches_at(repo: &Path) -> Result<Vec<BranchInfo>, String> {
    let output = run_git_cmd(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:short)|%(HEAD)|%(subject)|%(creatordate:iso)",
            "refs/heads/",
        ],
    )
    .await
    .map_err(|e| format!("git branch list failed: {}", e))?;
    Ok(git_workspace::parse_git_branches_output(&output))
}

/// Discover all git repositories under a workspace (nested repos when root has no `.git`).
#[tauri::command]
pub async fn ide_git_workspace_status(project_dir: String) -> Result<Vec<GitRepoSnapshot>, String> {
    if crate::remote::is_remote(&project_dir) {
        crate::remote::refresh_git_repos(&project_dir).await?;
    }
    let workspace = PathBuf::from(&project_dir);
    let repos = git_workspace::discover_git_repos_async(workspace.clone()).await;
    let mut snapshots = Vec::new();
    for repo in repos {
        let rel = git_workspace::repo_root_rel(&workspace, &repo);
        let name = git_workspace::repo_display_name(&workspace, &repo, &rel);
        let files = git_status_at(&repo, &workspace).await?;
        let branches = git_branches_at(&repo).await?;
        snapshots.push(GitRepoSnapshot {
            repo_root: rel,
            name,
            files,
            branches,
        });
    }
    Ok(snapshots)
}

/// Get git status for all files in the project directory (flattened across repos).
#[tauri::command]
pub async fn ide_git_status(project_dir: String) -> Result<Vec<GitFileStatus>, String> {
    if crate::remote::is_remote(&project_dir) && crate::remote::cached_git_repos(&project_dir).is_none() {
        crate::remote::refresh_git_repos(&project_dir).await?;
    }
    let workspace = PathBuf::from(&project_dir);
    let repos = git_workspace::discover_git_repos_async(workspace.clone()).await;
    let mut all = Vec::new();
    for repo in repos {
        all.extend(git_status_at(&repo, &workspace).await?);
    }
    Ok(all)
}

/// Get diff for a specific file (working tree vs HEAD or vs a specific ref).
#[tauri::command]
pub async fn ide_git_diff(
    project_dir: String,
    path: String,
    base: Option<String>,
    git_root: Option<String>,
) -> Result<DiffResult, String> {
    let workspace = PathBuf::from(&project_dir);
    let (root, path_in_repo) = git_workspace::resolve_git_context_for_root(
        &workspace,
        &path,
        git_root.as_deref(),
    )?;

    // Get original content (from HEAD or specified base)
    let base_ref = base.as_deref().unwrap_or("HEAD");
    let original = run_git_cmd(&root, &["show", &format!("{}:{}", base_ref, path_in_repo)])
        .await
        .unwrap_or_default();

    // Get current content
    let full_path = root.join(&path_in_repo);
    let modified = if let Some(remote_path) = crate::remote::resolve(&full_path.to_string_lossy()).await? {
        crate::remote::call("fs.readFile", serde_json::json!({ "path": remote_path }))
            .await
            .ok()
            .and_then(|v| v.get("base64").and_then(|b| b.as_str()).map(str::to_string))
            .and_then(|b| {
                use base64::Engine as _;
                base64::engine::general_purpose::STANDARD.decode(b).ok()
            })
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    } else if full_path.exists() {
        std::fs::read_to_string(&full_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Get unified diff
    let diff_args = if base.is_some() {
        vec!["diff", base_ref, "--", &path_in_repo]
    } else {
        vec!["diff", "HEAD", "--", &path_in_repo]
    };
    let diff_output = run_git_cmd(&root, &diff_args).await.unwrap_or_default();

    let hunks = parse_diff_hunks(&diff_output);

    Ok(DiffResult {
        path,
        original,
        modified,
        hunks,
    })
}

fn parse_diff_hunks(diff: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;
    let mut hunk_content = String::new();

    for line in diff.lines() {
        if line.starts_with("@@") {
            // Save previous hunk
            if let Some(mut hunk) = current_hunk.take() {
                hunk.content = hunk_content.clone();
                hunks.push(hunk);
                hunk_content.clear();
            }

            // Parse @@ -old_start,old_lines +new_start,new_lines @@
            let parts: Vec<&str> = line.split("@@").collect();
            if parts.len() >= 2 {
                let range = parts[1].trim();
                let ranges: Vec<&str> = range.split_whitespace().collect();
                if ranges.len() >= 2 {
                    let old = parse_range(ranges[0]);
                    let new = parse_range(ranges[1]);
                    current_hunk = Some(DiffHunk {
                        old_start: old.0,
                        old_lines: old.1,
                        new_start: new.0,
                        new_lines: new.1,
                        content: String::new(),
                    });
                }
            }
        } else if current_hunk.is_some()
            && (line.starts_with('+') || line.starts_with('-') || line.starts_with(' '))
        {
            hunk_content.push_str(line);
            hunk_content.push('\n');
        }
    }

    if let Some(mut hunk) = current_hunk {
        hunk.content = hunk_content;
        hunks.push(hunk);
    }

    hunks
}

fn parse_range(s: &str) -> (usize, usize) {
    let s = s.trim_start_matches('-').trim_start_matches('+');
    let parts: Vec<&str> = s.split(',').collect();
    let start = parts[0].parse().unwrap_or(0);
    let lines = parts.get(1).and_then(|l| l.parse().ok()).unwrap_or(1);
    (start, lines)
}

/// List all branches for the workspace git root (or empty when none / ambiguous).
#[tauri::command]
pub async fn ide_git_branches(
    project_dir: String,
    git_root: Option<String>,
) -> Result<Vec<BranchInfo>, String> {
    let workspace = PathBuf::from(&project_dir);
    let root = git_workspace::resolve_git_dir(&workspace, git_root.as_deref(), None)?;
    git_branches_at(&root).await
}

/// Get file content at a specific git ref (for diff comparison).
#[tauri::command]
pub async fn ide_git_file_at_ref(
    project_dir: String,
    path: String,
    git_ref: String,
    git_root: Option<String>,
) -> Result<FileContent, String> {
    let workspace = PathBuf::from(&project_dir);
    let (root, path_in_repo) =
        git_workspace::resolve_git_context_for_root(&workspace, &path, git_root.as_deref())?;
    let content = run_git_cmd(&root, &["show", &format!("{}:{}", git_ref, path_in_repo)])
        .await
        .map_err(|e| format!("git show failed: {}", e))?;

    let language = detect_language(&PathBuf::from(&path));

    Ok(FileContent {
        path: format!("{}@{}", path, git_ref),
        content,
        encoding: "utf-8".to_string(),
        is_binary: false,
        size: 0,
        language,
        preview_data: None,
    })
}

/// Stage files for commit (`git add`).
/// Pass `"."` as path to stage all changes.
#[tauri::command]
pub async fn ide_git_add(
    project_dir: String,
    path: String,
    git_root: Option<String>,
) -> Result<(), String> {
    let workspace = PathBuf::from(&project_dir);
    let (root, path_in_repo) =
        git_workspace::resolve_git_context_for_root(&workspace, &path, git_root.as_deref())?;
    run_git_cmd(&root, &["add", &path_in_repo])
        .await
        .map_err(|e| format!("git add failed: {}", e))?;
    Ok(())
}

/// Discard local changes to a file. For tracked files this restores the file
/// to HEAD (dropping both staged and worktree changes); untracked files are
/// removed from disk. Mirrors VS Code's "Discard Changes".
#[tauri::command]
pub async fn ide_git_discard(
    project_dir: String,
    path: String,
    git_root: Option<String>,
) -> Result<(), String> {
    let workspace = PathBuf::from(&project_dir);
    let (root, path_in_repo) =
        git_workspace::resolve_git_context_for_root(&workspace, &path, git_root.as_deref())?;

    // Is the path tracked? `git ls-files --error-unmatch` exits non-zero for
    // untracked paths (run_git_cmd returns Err in that case).
    let tracked = run_git_cmd(&root, &["ls-files", "--error-unmatch", "--", &path_in_repo])
        .await
        .is_ok();

    if tracked {
        run_git_cmd(&root, &["checkout", "HEAD", "--", &path_in_repo])
            .await
            .map_err(|e| format!("git discard failed: {}", e))?;
    } else {
        // Untracked — delete the file (or directory) from the working tree.
        let abs = root.join(&path_in_repo);
        if let Some(remote_path) = crate::remote::resolve(&abs.to_string_lossy()).await? {
            crate::remote::call("fs.delete", serde_json::json!({ "path": remote_path, "recursive": true }))
                .await?;
        } else if abs.is_dir() {
            std::fs::remove_dir_all(&abs).map_err(|e| format!("remove dir failed: {}", e))?;
        } else if abs.exists() {
            std::fs::remove_file(&abs).map_err(|e| format!("remove file failed: {}", e))?;
        }
    }
    Ok(())
}

/// Unstage files (`git reset HEAD -- <path>`).
#[tauri::command]
pub async fn ide_git_reset(
    project_dir: String,
    path: String,
    git_root: Option<String>,
) -> Result<(), String> {
    let workspace = PathBuf::from(&project_dir);
    let (root, path_in_repo) =
        git_workspace::resolve_git_context_for_root(&workspace, &path, git_root.as_deref())?;
    run_git_cmd(&root, &["reset", "HEAD", "--", &path_in_repo])
        .await
        .map_err(|e| format!("git reset failed: {}", e))?;
    Ok(())
}

/// Stage all changes in the working tree (`git add -A`).
/// Unlike `git add .`, `-A` also picks up deletions and changes outside the cwd.
#[tauri::command]
pub async fn ide_git_add_all(project_dir: String, git_root: Option<String>) -> Result<(), String> {
    let workspace = PathBuf::from(&project_dir);
    let root = git_workspace::resolve_git_dir(&workspace, git_root.as_deref(), None)?;
    run_git_cmd(&root, &["add", "-A"])
        .await
        .map_err(|e| format!("git add -A failed: {}", e))?;
    Ok(())
}

/// Unstage everything in the index (`git reset HEAD --`).
#[tauri::command]
pub async fn ide_git_reset_all(
    project_dir: String,
    git_root: Option<String>,
) -> Result<(), String> {
    let workspace = PathBuf::from(&project_dir);
    let root = git_workspace::resolve_git_dir(&workspace, git_root.as_deref(), None)?;
    run_git_cmd(&root, &["reset", "HEAD", "--"])
        .await
        .map_err(|e| format!("git reset all failed: {}", e))?;
    Ok(())
}

/// Commit staged changes with a message.
#[tauri::command]
pub async fn ide_git_commit(
    project_dir: String,
    message: String,
    git_root: Option<String>,
) -> Result<String, String> {
    let workspace = PathBuf::from(&project_dir);
    let root = git_workspace::resolve_git_dir(&workspace, git_root.as_deref(), None)?;
    let output = run_git_cmd(&root, &["commit", "-m", &message])
        .await
        .map_err(|e| format!("git commit failed: {}", e))?;
    Ok(output)
}

/// Checkout (switch to) a branch. Refuses if there are uncommitted changes
/// that would be overwritten (git handles that itself — the error is surfaced).
#[tauri::command]
pub async fn ide_git_checkout(
    project_dir: String,
    branch: String,
    git_root: Option<String>,
) -> Result<String, String> {
    let workspace = PathBuf::from(&project_dir);
    let root = git_workspace::resolve_git_dir(&workspace, git_root.as_deref(), None)?;
    let output = run_git_cmd(&root, &["checkout", &branch])
        .await
        .map_err(|e| format!("git checkout failed: {}", e))?;
    Ok(output)
}

/// Create a new branch from the current HEAD and switch to it.
#[tauri::command]
pub async fn ide_git_create_branch(
    project_dir: String,
    branch: String,
    git_root: Option<String>,
) -> Result<String, String> {
    let workspace = PathBuf::from(&project_dir);
    let root = git_workspace::resolve_git_dir(&workspace, git_root.as_deref(), None)?;
    let output = run_git_cmd(&root, &["checkout", "-b", &branch])
        .await
        .map_err(|e| format!("git create branch failed: {}", e))?;
    Ok(output)
}

// ─── Terminal (PTY) ────────────────────────────────────────────────────────

pub use crate::terminal_log::TerminalOutputLog;
use crate::terminal_log::TERMINAL_BUFFER_MAX_LINES;

/// Global terminal session registry.
pub struct TerminalRegistry {
    pub sessions: HashMap<String, TerminalSession>,
}

pub struct TerminalSession {
    pub child: Box<dyn portable_pty::Child + Send>,
    pub writer: Option<Box<dyn StdWrite + Send>>,
    pub output: Arc<std::sync::Mutex<TerminalOutputLog>>,
    /// Kept for resizing; dropping it closes the PTY.
    pub master: Box<dyn portable_pty::MasterPty + Send>,
}

impl TerminalRegistry {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

impl Default for TerminalRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a new PTY terminal session with the given working directory.
/// Output is streamed via `ide-terminal-output` Tauri events.
#[tauri::command]
pub async fn ide_terminal_create(
    app: AppHandle,
    state: State<'_, AppState>,
    terminal_id: String,
    project_dir: String,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<(), String> {
    let remote_cwd = crate::remote::resolve(&project_dir).await?;
    let remote_target = match &remote_cwd {
        Some(_) => Some(state.ext_host.target().await.ok_or("remote is not connected")?),
        None => None,
    };
    let root = PathBuf::from(&project_dir);
    if remote_cwd.is_none() && !root.exists() {
        return Err(format!("Directory not found: {}", project_dir));
    }

    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: rows.unwrap_or(24),
            cols: cols.unwrap_or(80),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    // On Windows the $SHELL variable is not set (and bash requires WSL).
    // Fall back to PowerShell which is always available on Windows 10/11.
    #[cfg(windows)]
    let shell = "powershell.exe".to_string();
    #[cfg(not(windows))]
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
    let mut cmd = match (&remote_target, &remote_cwd) {
        (Some(target), Some(cwd)) => {
            let argv = target.interactive_shell_argv(cwd);
            let mut c = CommandBuilder::new(&argv[0]);
            c.args(&argv[1..]);
            c
        }
        _ => {
            let mut c = CommandBuilder::new(shell);
            c.cwd(&root);
            c
        }
    };
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {}", e))?;

    let output_log = Arc::new(std::sync::Mutex::new(TerminalOutputLog::default()));

    // Register the session
    {
        let mut registry = state.terminals.lock().await;
        registry.sessions.insert(
            terminal_id.clone(),
            TerminalSession {
                child,
                writer: Some(writer),
                output: output_log.clone(),
                master: pair.master,
            },
        );
    }

    // Spawn output reader task
    let app_clone = app.clone();
    let tid = terminal_id.clone();
    tokio::task::spawn_blocking(move || {
        let mut reader = reader;
        let mut buf = vec![0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    if let Ok(mut log) = output_log.lock() {
                        log.append(&data);
                    }
                    let _ = app_clone.emit(
                        "ide-terminal-output",
                        serde_json::json!({ "id": tid, "data": data }),
                    );
                }
                Err(_) => break,
            }
        }
    });

    Ok(())
}

/// Write data (keystrokes) to a terminal session.
#[tauri::command]
pub async fn ide_terminal_write(
    state: State<'_, AppState>,
    terminal_id: String,
    data: String,
) -> Result<(), String> {
    let mut registry = state.terminals.lock().await;
    let session = registry
        .sessions
        .get_mut(&terminal_id)
        .ok_or_else(|| format!("Terminal '{}' not found", terminal_id))?;

    if let Some(ref mut writer) = session.writer {
        writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Write failed: {}", e))?;
        writer.flush().map_err(|e| format!("Flush failed: {}", e))?;
    }

    Ok(())
}

/// Resize a terminal session's PTY.
#[tauri::command]
pub async fn ide_terminal_resize(
    state: State<'_, AppState>,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    if cols == 0 || rows == 0 {
        return Ok(());
    }
    let registry = state.terminals.lock().await;
    let session = registry
        .sessions
        .get(&terminal_id)
        .ok_or_else(|| format!("Terminal '{}' not found", terminal_id))?;
    session
        .master
        .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("Resize failed: {}", e))
}

/// Destroy a terminal session.
#[tauri::command]
pub async fn ide_terminal_destroy(
    state: State<'_, AppState>,
    terminal_id: String,
) -> Result<(), String> {
    let mut registry = state.terminals.lock().await;
    if let Some(mut session) = registry.sessions.remove(&terminal_id) {
        // Drop writer to signal EOF
        session.writer.take();
        // Kill the process
        let _ = session.child.kill();
        Ok(())
    } else {
        Err(format!("Terminal '{}' not found", terminal_id))
    }
}

/// Number of live PTY terminal sessions.
#[tauri::command]
pub async fn ide_terminal_count(state: State<'_, AppState>) -> Result<usize, String> {
    let registry = state.terminals.lock().await;
    Ok(registry.sessions.len())
}

/// Terminate every terminal session (e.g. when closing the project).
#[tauri::command]
pub async fn ide_terminal_destroy_all(state: State<'_, AppState>) -> Result<(), String> {
    let mut registry = state.terminals.lock().await;
    for (_, mut session) in registry.sessions.drain() {
        session.writer.take();
        let _ = session.child.kill();
    }
    Ok(())
}

/// Whether the PTY shell process for a session is still running.
#[tauri::command]
pub async fn ide_terminal_is_alive(
    state: State<'_, AppState>,
    terminal_id: String,
) -> Result<bool, String> {
    let mut registry = state.terminals.lock().await;
    let session = registry
        .sessions
        .get_mut(&terminal_id)
        .ok_or_else(|| format!("Terminal '{}' not found", terminal_id))?;
    match session.child.try_wait() {
        Ok(None) => Ok(true),
        Ok(Some(_)) => Ok(false),
        Err(e) => Err(format!("Failed to check terminal status: {e}")),
    }
}

/// Read recent output from a terminal session (tail or grep within a tail window).
#[tauri::command]
pub async fn ide_terminal_read(
    state: State<'_, AppState>,
    terminal_id: Option<String>,
    lines: Option<usize>,
    grep: Option<String>,
    grep_lines: Option<usize>,
) -> Result<String, String> {
    let registry = state.terminals.lock().await;
    let tid = if let Some(id) = terminal_id.filter(|s| !s.trim().is_empty()) {
        id
    } else {
        registry
            .sessions
            .keys()
            .next()
            .cloned()
            .ok_or_else(|| "No terminal sessions are running".to_string())?
    };
    let session = registry
        .sessions
        .get(&tid)
        .ok_or_else(|| format!("Terminal '{tid}' not found"))?;
    let log = session
        .output
        .lock()
        .map_err(|e| format!("terminal output lock poisoned: {e}"))?;
    let out = if let Some(pattern) = grep.filter(|s| !s.is_empty()) {
        let window = grep_lines
            .unwrap_or(100)
            .clamp(1, TERMINAL_BUFFER_MAX_LINES);
        log.grep_in_tail(&pattern, window)
    } else {
        let n = lines.unwrap_or(50).clamp(1, TERMINAL_BUFFER_MAX_LINES);
        log.tail(n)
    };
    if out.is_empty() {
        Ok(format!("[Terminal '{tid}' — no output captured yet]"))
    } else {
        Ok(format!("--- terminal:{tid} ---\n{out}"))
    }
}

/// Store a user-selected terminal excerpt for `@terminal-snippet(id)` expansion.
#[tauri::command]
pub async fn terminal_snippet_put(
    state: State<'_, AppState>,
    text: String,
) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("terminal snippet text is empty".to_string());
    }
    let id = uuid::Uuid::new_v4().to_string();
    state
        .terminal_snippets
        .lock()
        .await
        .insert(id.clone(), trimmed.to_string());
    Ok(id)
}

// ─── File Watcher (Event Bridge) ───────────────────────────────────────────

/// Start watching a project directory for file changes.
/// Emits `ide-file-changed` events when files are modified externally
/// (e.g., by Koi agents writing via file_write tool).
#[tauri::command]
pub async fn ide_start_watcher(
    app: AppHandle,
    state: State<'_, AppState>,
    project_dir: String,
) -> Result<(), String> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

    // Remote: the server watches on its side; no local index/graph workers.
    if let Some(remote_root) = crate::remote::resolve(&project_dir).await? {
        return state.ext_host.watch_remote(&project_dir, &remote_root).await;
    }

    let root = PathBuf::from(&project_dir);
    if !root.exists() {
        return Err(format!("Directory not found: {}", project_dir));
    }

    // Serialize start/stop so concurrent mounts cannot create duplicate workers.
    let mut watchers = state.file_watchers.lock().await;
    if watchers.contains_key(&project_dir) {
        return Ok(());
    }

    let index_root = root.clone();
    let index_worker = crate::index_worker::IndexWorker::start(move |batch| {
        if batch.rebuild {
            if let Err(error) = crate::commands::codebase::build_index(&index_root) {
                tracing::warn!(%error, "Codebase rebuild failed");
            }
            crate::commands::graph_index::request_rebuild(index_root.clone());
        } else {
            for rel in batch.paths {
                if let Err(error) = crate::commands::codebase::index_file(&index_root, &rel) {
                    tracing::warn!(%error, path = %rel, "Incremental index failed");
                }
                crate::commands::graph::schedule_patch(index_root.clone(), rel);
            }
        }
    })
    .map_err(|e| format!("Failed to start index worker: {e}"))?;
    let app_clone = app.clone();
    let dir = project_dir.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Filter to relevant events
                match event.kind {
                    notify::EventKind::Modify(_)
                    | notify::EventKind::Create(_)
                    | notify::EventKind::Remove(_) => {
                        for path in &event.paths {
                            let rel = path
                                .strip_prefix(&dir)
                                .unwrap_or(path)
                                .to_string_lossy()
                                .to_string();

                            // Normalize to forward slashes. On Windows the native
                            // separator is `\\`, but the IDE's `tab.path` always
                            // uses `/` (that's how `openFile` stores it, how
                            // `FileTree` reports node paths, and how `ideApi.readFile`
                            // builds the full path). Emitting the raw OS path
                            // meant `tab.path === evt.path` silently failed on
                            // Windows and the editor never reloaded files that
                            // agents / external tools changed.
                            let rel_norm = rel.replace('\\', "/");
                            if !crate::path_filter::should_watch_path(&rel_norm) {
                                continue;
                            }

                            let kind = match event.kind {
                                notify::EventKind::Create(_) => "created",
                                notify::EventKind::Modify(_) => "modified",
                                notify::EventKind::Remove(_) => "deleted",
                                _ => "unknown",
                            };

                            let _ = app_clone.emit(
                                "ide-file-changed",
                                serde_json::json!({
                                    "project_dir": dir,
                                    "path": rel_norm,
                                    "kind": kind,
                                }),
                            );

                            // Incrementally update the codebase index so
                            // @codebase / codebase_search stay fresh (best-effort).
                            if crate::path_filter::should_index_path(&rel_norm) {
                                index_worker.enqueue(rel_norm.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        },
        Config::default(),
    )
    .map_err(|e| format!("Failed to create watcher: {}", e))?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch: {}", e))?;

    crate::commands::graph_index::ensure_started(&root);

    // Store the watcher to keep it alive
    watchers.insert(project_dir, watcher);

    Ok(())
}

/// Stop watching a project directory.
#[tauri::command]
pub async fn ide_stop_watcher(
    state: State<'_, AppState>,
    project_dir: String,
) -> Result<(), String> {
    if crate::remote::is_remote(&project_dir) {
        state.ext_host.unwatch_remote(&project_dir).await;
        return Ok(());
    }
    let mut watchers = state.file_watchers.lock().await;
    watchers.remove(&project_dir);
    Ok(())
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Build a `git` command that never opens a console window on Windows.
///
/// Thin wrapper over [`piscis_kernel::proc::tokio_command`] kept as a named
/// helper so all git invocations remain grep-able.
fn new_git_cmd() -> Command {
    tokio_command("git")
}

async fn run_git_cmd(dir: &Path, args: &[&str]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&run_git_cmd_bytes(dir, args).await?).to_string())
}

async fn run_git_cmd_bytes(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    if let Some(remote_dir) = crate::remote::resolve(&dir.to_string_lossy()).await? {
        let v = crate::remote::call(
            "exec",
            serde_json::json!({ "command": "git", "args": args, "cwd": remote_dir, "timeoutMs": 30_000 }),
        )
        .await?;
        if v.get("code").and_then(|c| c.as_i64()) != Some(0) {
            let stderr = v.get("stderr").and_then(|s| s.as_str()).unwrap_or_default();
            return Err(format!("git error: {}", stderr.trim()));
        }
        return Ok(v
            .get("stdout")
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .as_bytes()
            .to_vec());
    }
    let output = timeout(
        Duration::from_secs(30),
        new_git_cmd()
            .args(args)
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| "git command timed out")?
    .map_err(|e| format!("git command failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git error: {}", stderr.trim()));
    }

    Ok(output.stdout)
}

// ─── LSP Commands ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct LspLanguageInfo {
    pub language_id: String,
    pub name: String,
    pub extensions: Vec<String>,
    pub server_command: String,
    pub available: bool,
}

/// List all supported LSP languages with their availability status.
#[tauri::command]
pub async fn ide_lsp_list_languages() -> Result<Vec<LspLanguageInfo>, String> {
    Ok(LspManager::supported_languages()
        .into_iter()
        .map(|l| LspLanguageInfo {
            language_id: l.language_id,
            name: l.name,
            extensions: l.extensions,
            server_command: l.server_command,
            available: l.available,
        })
        .collect())
}

/// Start an LSP server for the given project directory and language.
/// Returns the WebSocket port the Monaco Editor can connect to.
#[tauri::command]
pub async fn ide_lsp_start(
    state: tauri::State<'_, crate::state::AppState>,
    project_dir: String,
    language: String,
) -> Result<u16, String> {
    if let Some(uri) = crate::remote::vfs::parse(&project_dir) {
        let mgr = crate::remote::manager().ok_or("remote broker unavailable")?;
        let target = mgr.wait_for_authority(&uri.authority).await?;
        return crate::remote::lsp::start(target, &project_dir, &language).await;
    }
    state.lsp_manager.start(&project_dir, &language).await
}

/// Stop an LSP session for the given project + language.
#[tauri::command]
pub async fn ide_lsp_stop(
    state: tauri::State<'_, crate::state::AppState>,
    project_dir: String,
    language: String,
) -> Result<(), String> {
    if crate::remote::is_remote(&project_dir) {
        crate::remote::lsp::stop(&project_dir, &language);
        return Ok(());
    }
    state.lsp_manager.stop(&project_dir, &language).await
}
