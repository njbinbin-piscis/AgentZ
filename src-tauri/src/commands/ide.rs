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
    /// sha256 (hex) of the exact bytes on disk that `content` was decoded from.
    /// The editor sends it back on save so a write can detect external changes.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Prefix of the error returned when a save would overwrite a file changed on disk.
pub const WRITE_CONFLICT_PREFIX: &str = "conflict:";

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
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
    show_hidden: Option<bool>,
) -> Result<Vec<FileNode>, String> {
    let show_hidden = show_hidden.unwrap_or(false);
    if crate::remote::is_remote(&project_dir) {
        let session = crate::remote::bind(&project_dir).await?;
        let v = session
            .call(
                "fs.tree",
                serde_json::json!({ "path": session.path, "depth": depth.unwrap_or(10) }),
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
        let mut nodes = build_file_tree(&root, &root, 0, max_depth, &ignore_patterns, show_hidden)
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

fn is_ignored(name: &str, path: &Path, patterns: &[String], show_hidden: bool) -> bool {
    if show_hidden {
        return crate::path_filter::is_always_hidden_tree_entry(name);
    }
    let rel = path.to_string_lossy().replace('\\', "/");
    crate::path_filter::is_ignored_tree_entry(name, &rel, patterns)
}

fn build_file_tree(
    dir: &Path,
    root: &Path,
    current_depth: usize,
    max_depth: usize,
    patterns: &[String],
    show_hidden: bool,
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

        if is_ignored(&name, &path, patterns, show_hidden) {
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
            let children = build_file_tree(&path, root, current_depth + 1, max_depth, patterns, show_hidden)?;
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
    _state: tauri::State<'_, crate::state::AppState>,
    path: String,
) -> Result<FileContent, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    if crate::remote::is_remote(&path) {
        let session = crate::remote::bind(&path).await?;
        let value = session
            .call("fs.readFile", serde_json::json!({ "path": session.path }))
            .await?;
        let encoded = value
            .get("base64")
            .and_then(|v| v.as_str())
            .ok_or("bad fs.readFile reply")?;
        let raw = STANDARD.decode(encoded).map_err(|e| e.to_string())?;
        let size = raw.len() as u64;
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
            sha256: None,
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
                    sha256: None,
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
                sha256: None,
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
            sha256: None,
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
        sha256: Some(sha256_hex(&raw)),
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
///
/// `expected_sha256` is the digest of the bytes the caller last read. When given, the write
/// is refused with a `conflict:` error if the file on disk no longer matches, so an external
/// change is never overwritten silently. Local writes go through a temporary file and an
/// atomic rename. Returns the digest of the bytes written; remote files return `None`
/// and do not check the expectation.
#[tauri::command]
pub async fn ide_write_file(
    _state: tauri::State<'_, crate::state::AppState>,
    path: String,
    content: String,
    expected_sha256: Option<String>,
) -> Result<Option<String>, String> {
    if crate::remote::is_remote(&path) {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let session = crate::remote::bind(&path).await?;
        session
            .call("fs.writeFile", serde_json::json!({ "path": session.path, "base64": STANDARD.encode(content.as_bytes()) }))
            .await?;
        return Ok(None);
    }
    let file_path = PathBuf::from(&path);
    if let Some(expected) = expected_sha256.as_deref() {
        ensure_unchanged_since(&file_path, expected)?;
    }
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }
    write_atomically(&file_path, content.as_bytes())
        .map(Some)
        .map_err(|e| format!("Failed to write: {e}"))
}

/// Refuse a save when the file no longer has the digest the editor last read.
fn ensure_unchanged_since(file_path: &Path, expected: &str) -> Result<(), String> {
    let shown = file_path.display();
    match std::fs::read(file_path) {
        Ok(current) if sha256_hex(&current) == expected => Ok(()),
        Ok(_) => Err(format!(
            "{WRITE_CONFLICT_PREFIX} \"{shown}\" changed on disk since it was opened; reload it before saving"
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "{WRITE_CONFLICT_PREFIX} \"{shown}\" was deleted on disk since it was opened"
        )),
        Err(e) => Err(format!("Failed to read current file: {e}")),
    }
}

/// Write `bytes` to a sibling temporary file, flush it, then rename it over `target`.
/// A reader sees either the old or the new complete file, never a partial write.
fn write_atomically(target: &Path, bytes: &[u8]) -> std::io::Result<String> {
    use std::io::Write;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let temp = target.with_file_name(format!(".{name}.{}.agentz-tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map(|_| sha256_hex(bytes))
}

#[cfg(test)]
mod write_conflict_tests {
    use super::{ensure_unchanged_since, sha256_hex, write_atomically, WRITE_CONFLICT_PREFIX};

    fn fixture(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("agentz-ide-write-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create fixture dir");
        dir.join(name)
    }

    #[test]
    fn save_is_refused_when_disk_changed_since_read() {
        let file = fixture("a.txt");
        std::fs::write(&file, b"original").unwrap();
        let read_digest = sha256_hex(b"original");

        std::fs::write(&file, b"changed by agent").unwrap();
        let err = ensure_unchanged_since(&file, &read_digest).unwrap_err();
        assert!(err.starts_with(WRITE_CONFLICT_PREFIX), "{err}");
        assert_eq!(std::fs::read(&file).unwrap(), b"changed by agent");
    }

    #[test]
    fn save_is_allowed_when_digest_matches() {
        let file = fixture("b.txt");
        std::fs::write(&file, b"same").unwrap();
        assert!(ensure_unchanged_since(&file, &sha256_hex(b"same")).is_ok());
    }

    #[test]
    fn save_is_refused_when_file_was_deleted() {
        let file = fixture("c.txt");
        let err = ensure_unchanged_since(&file, &sha256_hex(b"x")).unwrap_err();
        assert!(
            err.starts_with(WRITE_CONFLICT_PREFIX) && err.contains("deleted"),
            "{err}"
        );
    }

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_temp_file() {
        let file = fixture("d.txt");
        std::fs::write(&file, b"old").unwrap();
        let digest = write_atomically(&file, b"new content").unwrap();
        assert_eq!(digest, sha256_hex(b"new content"));
        assert_eq!(std::fs::read(&file).unwrap(), b"new content");
        let leftovers: Vec<_> = std::fs::read_dir(file.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".agentz-tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}

fn remote_rename_target(authority: &str, target: &str) -> Result<String, String> {
    if !crate::remote::is_remote(target) {
        return Ok(target.to_string());
    }
    let uri = crate::remote::vfs::parse(target).ok_or("invalid remote rename target URI")?;
    if !uri.authority.eq_ignore_ascii_case(authority) {
        return Err("remote rename target belongs to a different authority".into());
    }
    Ok(uri.path)
}

#[cfg(test)]
mod remote_file_binding_tests {
    use super::remote_rename_target;

    #[test]
    fn rename_rejects_other_authority_and_malformed_remote_uri() {
        assert!(remote_rename_target("host-a", "agentz-remote://host-b/file").is_err());
        assert!(remote_rename_target("host-a", "agentz-remote:///file").is_err());
        assert_eq!(
            remote_rename_target("host-a", "agentz-remote://HOST-A/file").unwrap(),
            "/file"
        );
        assert_eq!(remote_rename_target("host-a", "/file").unwrap(), "/file");
    }
}

/// Perform file actions: create_file, create_dir, delete, rename.
#[tauri::command]
pub async fn ide_file_action(
    path: String,
    action: String,
    new_path: Option<String>,
) -> Result<(), String> {
    if crate::remote::is_remote(&path) {
        let session = crate::remote::bind(&path).await?;
        let remote_path = &session.path;
        let (method, params) = match action.as_str() {
            "create_file" => (
                "fs.writeFile",
                serde_json::json!({ "path": remote_path, "text": "" }),
            ),
            "create_dir" => ("fs.mkdir", serde_json::json!({ "path": remote_path })),
            "delete" => (
                "fs.delete",
                serde_json::json!({ "path": remote_path, "recursive": true }),
            ),
            "rename" => {
                let target = new_path.ok_or("rename requires 'new_path' parameter")?;
                let to = remote_rename_target(&session.authority, &target)?;
                (
                    "fs.rename",
                    serde_json::json!({ "from": remote_path, "to": to }),
                )
            }
            _ => return Err(format!("Unknown action: {}", action)),
        };
        return session.call(method, params).await.map(|_| ());
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
    if crate::remote::is_remote(&project_dir) {
        let session = crate::remote::bind(&project_dir).await?;
        let v = session
            .call(
                "search",
                serde_json::json!({
                    "root": session.path,
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

type GitStatusRun = futures::future::Shared<
    futures::future::BoxFuture<'static, Result<std::sync::Arc<Vec<u8>>, String>>,
>;

/// At most one `git status` per repo at a time; concurrent callers share its
/// output. A refresh storm from the UI would otherwise pile up git processes.
fn git_status_runs() -> &'static std::sync::Mutex<HashMap<PathBuf, GitStatusRun>> {
    static RUNS: std::sync::OnceLock<std::sync::Mutex<HashMap<PathBuf, GitStatusRun>>> =
        std::sync::OnceLock::new();
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
                    let out =
                        run_git_cmd_bytes(&repo, &["status", "--porcelain=v1", "-z", "-uall"])
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
    if crate::remote::is_remote(&project_dir)
        && crate::remote::cached_git_repos(&project_dir).is_none()
    {
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
    if crate::remote::is_remote(&project_dir) {
        let session = crate::remote::bind(&project_dir).await?;
        return crate::remote::with_session(
            session,
            ide_git_diff_bound(project_dir, path, base, git_root),
        )
        .await;
    }
    ide_git_diff_bound(project_dir, path, base, git_root).await
}

async fn ide_git_diff_bound(
    project_dir: String,
    path: String,
    base: Option<String>,
    git_root: Option<String>,
) -> Result<DiffResult, String> {
    let remote = crate::remote::is_remote(&project_dir);
    let workspace = PathBuf::from(&project_dir);
    let (root, path_in_repo) =
        git_workspace::resolve_git_context_for_root(&workspace, &path, git_root.as_deref())?;

    // Get original content (from HEAD or specified base)
    let base_ref = base.as_deref().unwrap_or("HEAD");
    let original = if remote {
        // A missing base entry is legitimate for an added file. Distinguish it
        // from a failed Git read using a successful tree listing first.
        let entries = run_git_cmd_bytes(
            &root,
            &[
                "--literal-pathspecs",
                "ls-tree",
                "-z",
                "--name-only",
                base_ref,
                "--",
                &path_in_repo,
            ],
        )
        .await?;
        if tracked_listing_contains(&entries, &path_in_repo)? {
            run_git_cmd(&root, &["show", &format!("{}:{}", base_ref, path_in_repo)]).await?
        } else {
            String::new()
        }
    } else {
        run_git_cmd(&root, &["show", &format!("{}:{}", base_ref, path_in_repo)])
            .await
            .unwrap_or_default()
    };

    // Get current content
    let full_path = root.join(&path_in_repo);
    let modified = if crate::remote::is_remote(&full_path.to_string_lossy()) {
        let remote_path = crate::remote::resolve(&full_path.to_string_lossy())
            .await?
            .ok_or("remote diff file path is not remote")?;
        let reply =
            crate::remote::call("fs.readFile", serde_json::json!({ "path": remote_path })).await?;
        decode_remote_diff_text(&reply)?
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
    let diff_output = if remote {
        run_git_cmd(&root, &diff_args).await?
    } else {
        run_git_cmd(&root, &diff_args).await.unwrap_or_default()
    };

    let hunks = parse_diff_hunks(&diff_output);

    Ok(DiffResult {
        path,
        original,
        modified,
        hunks,
    })
}

fn decode_remote_diff_text(reply: &serde_json::Value) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let encoded = reply
        .get("base64")
        .and_then(|v| v.as_str())
        .ok_or("bad remote diff file reply: missing base64")?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|e| format!("bad remote diff file reply: {e}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod remote_diff_reply_tests {
    use super::decode_remote_diff_text;
    use serde_json::json;

    #[test]
    fn missing_or_corrupt_content_is_not_an_empty_success() {
        assert!(decode_remote_diff_text(&json!({})).is_err());
        assert!(decode_remote_diff_text(&json!({"base64": null})).is_err());
        assert!(decode_remote_diff_text(&json!({"base64": "%%%"})).is_err());
        assert_eq!(decode_remote_diff_text(&json!({"base64": ""})).unwrap(), "");
    }

    #[test]
    fn valid_remote_text_is_decoded() {
        assert_eq!(
            decode_remote_diff_text(&json!({"base64": "aGVsbG8="})).unwrap(),
            "hello"
        );
    }
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
        sha256: None,
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
    if crate::remote::is_remote(&project_dir) {
        let session = crate::remote::bind(&project_dir).await?;
        return crate::remote::with_session(
            session,
            ide_git_discard_bound(project_dir, path, git_root),
        )
        .await;
    }
    ide_git_discard_bound(project_dir, path, git_root).await
}

async fn ide_git_discard_bound(
    project_dir: String,
    path: String,
    git_root: Option<String>,
) -> Result<(), String> {
    let workspace = PathBuf::from(&project_dir);
    let (root, path_in_repo) =
        git_workspace::resolve_git_context_for_root(&workspace, &path, git_root.as_deref())?;

    // Only a successful, NUL-delimited listing can establish that a path is
    // untracked. Permission, transport and repository failures must not delete.
    let tracked_paths = run_git_cmd_bytes(
        &root,
        &["--literal-pathspecs", "ls-files", "-z", "--", &path_in_repo],
    )
    .await
    .map_err(|e| format!("git discard tracking check failed: {e}"))?;
    let tracked = tracked_listing_contains(&tracked_paths, &path_in_repo)?;

    if tracked {
        run_git_cmd(&root, &["checkout", "HEAD", "--", &path_in_repo])
            .await
            .map_err(|e| format!("git discard failed: {}", e))?;
    } else {
        // Untracked — delete the file (or directory) from the working tree.
        let abs = root.join(&path_in_repo);
        if crate::remote::is_remote(&abs.to_string_lossy()) {
            // Resolve and call inside the command's original session scope.
            // A reconnect must not turn a failed tracked-file probe into a
            // deletion on the replacement connection.
            let remote_path = crate::remote::resolve(&abs.to_string_lossy())
                .await?
                .ok_or("remote discard path is not remote")?;
            crate::remote::call(
                "fs.delete",
                serde_json::json!({ "path": remote_path, "recursive": true }),
            )
            .await?;
        } else if abs.is_dir() {
            std::fs::remove_dir_all(&abs).map_err(|e| format!("remove dir failed: {}", e))?;
        } else if abs.exists() {
            std::fs::remove_file(&abs).map_err(|e| format!("remove file failed: {}", e))?;
        }
    }
    Ok(())
}

fn tracked_listing_contains(output: &[u8], path: &str) -> Result<bool, String> {
    if output.is_empty() {
        return Ok(false);
    }
    if output.last() != Some(&0) {
        return Err("git discard tracking reply is not NUL-terminated".into());
    }
    let exact = path.as_bytes();
    let directory = format!("{}/", path.trim_end_matches('/'));
    let records: Vec<&[u8]> = output[..output.len() - 1].split(|b| *b == 0).collect();
    if records.iter().any(|record| record.is_empty()) {
        return Err("git discard tracking reply contains an empty path".into());
    }
    if records
        .iter()
        .any(|record| *record != exact && !record.starts_with(directory.as_bytes()))
    {
        return Err("git discard tracking reply contains an unrelated path".into());
    }
    Ok(true)
}

#[cfg(test)]
mod discard_tracking_tests {
    use super::tracked_listing_contains;

    #[test]
    fn nul_listing_preserves_unicode_newlines_and_literal_names() {
        assert!(
            tracked_listing_contains("目录/换\n行[1].rs\0".as_bytes(), "目录/换\n行[1].rs")
                .unwrap()
        );
        assert!(tracked_listing_contains(b"src/file.rs\0", "src").unwrap());
        assert!(tracked_listing_contains(b"other.rs\0", "file.rs").is_err());
        assert!(!tracked_listing_contains(b"", "file.rs").unwrap());
    }

    #[test]
    fn malformed_listing_cannot_authorize_deletion() {
        assert!(tracked_listing_contains(b"file.rs", "file.rs").is_err());
        assert!(tracked_listing_contains(b"\0", "file.rs").is_err());
        assert!(tracked_listing_contains(b"file.rs\0\0", "file.rs").is_err());
    }
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

/// Create a Git repository in the workspace root (`git init`).
/// Refuses when the folder is already inside a repository, including a parent
/// one that the panel does not list because discovery does not walk upward.
#[tauri::command]
pub async fn ide_git_init(project_dir: String) -> Result<(), String> {
    let workspace = PathBuf::from(&project_dir);
    let remote = crate::remote::is_remote(&project_dir);
    if !remote && !workspace.is_dir() {
        return Err("project directory does not exist".into());
    }
    if let Ok(top) = run_git_cmd(&workspace, &["rev-parse", "--show-toplevel"]).await {
        let top = top.trim();
        if !top.is_empty() {
            return Err(format!("GIT_ALREADY_INSIDE:{top}"));
        }
    }
    run_git_cmd(&workspace, &["init"])
        .await
        .map_err(|e| format!("git init failed: {}", e))?;
    if remote {
        crate::remote::refresh_git_repos(&project_dir).await?;
    }
    Ok(())
}

#[cfg(test)]
mod git_init_tests {
    use super::ide_git_init;

    fn scratch() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "agentz-git-init-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn init_creates_a_repo_and_refuses_to_nest_another() {
        let root = scratch();
        let root_s = root.to_string_lossy().to_string();
        ide_git_init(root_s).await.expect("init workspace");
        assert!(root.join(".git").is_dir());

        let child = root.join("nested");
        std::fs::create_dir_all(&child).unwrap();
        let err = ide_git_init(child.to_string_lossy().to_string())
            .await
            .expect_err("nested init");
        assert!(err.starts_with("GIT_ALREADY_INSIDE:"), "{err}");
        assert!(!child.join(".git").exists());

        let _ = std::fs::remove_dir_all(&root);
    }
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
    /// Contains the shell and its descendants; terminating it kills the whole tree.
    pub job: TerminalJob,
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

/// A racing create must never replace an owned PTY without its teardown path.
fn register_terminal_session(
    registry: &mut TerminalRegistry,
    terminal_id: String,
    session: TerminalSession,
) -> Result<(), TerminalSession> {
    if registry.sessions.contains_key(&terminal_id) {
        return Err(session);
    }
    registry.sessions.insert(terminal_id, session);
    Ok(())
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
    // A duplicate id must never replace a live session: replacing drops its
    // PTY handles without running the normal teardown path.
    if state
        .terminals
        .lock()
        .await
        .sessions
        .contains_key(&terminal_id)
    {
        return Err(format!("Terminal '{terminal_id}' already exists"));
    }
    let remote_session = if crate::remote::is_remote(&project_dir) {
        Some(crate::remote::bind(&project_dir).await?)
    } else {
        None
    };
    let root = PathBuf::from(&project_dir);
    if remote_session.is_none() && !root.exists() {
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
    let mut cmd = match &remote_session {
        Some(session) => {
            // The target was captured with the URI's authority and generation.
            // Never consult a replacement connection for this terminal's argv.
            let argv = session.target.interactive_shell_argv(&session.path);
            let mut c = CommandBuilder::new(&argv[0]);
            c.args(&argv[1..]);
            c
        }
        None => {
            let mut c = CommandBuilder::new(shell);
            c.cwd(&root);
            c
        }
    };
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;
    // Contain the shell right after spawn. If containment fails, the tree could not be
    // torn down later, so kill the shell now and report the error.
    let job = match contain_shell(&*child) {
        Ok(job) => job,
        Err(e) => {
            return Err(match child.kill() {
                Ok(()) => {
                    let _ = child.wait();
                    format!("Failed to contain shell process: {e}")
                }
                Err(kill_error) => format!(
                    "Failed to contain shell process: {e}; failed to kill uncontained shell: {kill_error}"
                ),
            });
        }
    };

    let (reader, writer) = acquire_terminal_io(pair.master.as_ref(), child.as_mut(), &job, None)?;

    let output_log = Arc::new(std::sync::Mutex::new(TerminalOutputLog::default()));

    let session = TerminalSession {
        child,
        writer: Some(writer),
        output: output_log.clone(),
        master: pair.master,
        job,
    };
    let duplicate = {
        let mut registry = state.terminals.lock().await;
        register_terminal_session(&mut registry, terminal_id.clone(), session).err()
    };
    if let Some(mut duplicate) = duplicate {
        return Err(terminate_failed_terminal_setup(
            duplicate.child.as_mut(),
            &duplicate.job,
            format!("Terminal '{terminal_id}' already exists"),
        ));
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
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Resize failed: {}", e))
}

/// Process containment for a terminal's shell. On Windows this is a Job Object with
/// kill-on-close: the shell and every process it spawned belong to the job, so
/// terminating the job reaches the whole tree even after the shell has exited.
/// Teardown needs no pid, which may already be dead or reused by then.
#[cfg(windows)]
pub struct TerminalJob(windows_sys::Win32::Foundation::HANDLE);

// SAFETY: a job handle is a kernel object reference usable from any thread. The
// wrapper owns the handle and closes it exactly once, in Drop.
#[cfg(windows)]
unsafe impl Send for TerminalJob {}
#[cfg(windows)]
unsafe impl Sync for TerminalJob {}

#[cfg(windows)]
impl TerminalJob {
    /// Create a kill-on-close job and assign the process with `pid` to it.
    fn adopt(pid: u32) -> Result<Self, String> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        // SAFETY: each handle is checked before use. The job is wrapped as soon as it is
        // created, so every later error path closes it through Drop.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(format!(
                    "CreateJobObjectW failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let job = TerminalJob(handle);

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if configured == 0 {
                return Err(format!(
                    "SetInformationJobObject failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                return Err(format!(
                    "OpenProcess({pid}) failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let assigned = AssignProcessToJobObject(job.0, process);
            let assign_error = std::io::Error::last_os_error();
            CloseHandle(process);
            if assigned == 0 {
                return Err(format!("AssignProcessToJobObject failed: {assign_error}"));
            }
            Ok(job)
        }
    }

    /// Terminate every process in the job, including the shell. An empty job succeeds.
    fn terminate(&self) -> Result<(), String> {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;

        // SAFETY: `self.0` is a valid job handle owned by `self`.
        if unsafe { TerminateJobObject(self.0, 1) } == 0 {
            return Err(format!(
                "TerminateJobObject failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    /// Number of processes still alive in the job (descendants included).
    fn active_processes(&self) -> Result<u32, String> {
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicAccountingInformation, QueryInformationJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        };

        // SAFETY: `info` is a plain-old-data struct sized for the requested class, and
        // `self.0` is a valid job handle owned by `self`.
        unsafe {
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = std::mem::zeroed();
            let mut returned: u32 = 0;
            if QueryInformationJobObject(
                self.0,
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                &mut returned,
            ) == 0
            {
                return Err(format!(
                    "QueryInformationJobObject failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(info.ActiveProcesses)
        }
    }
}

#[cfg(windows)]
impl Drop for TerminalJob {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this wrapper and closed only here.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// Non-Windows shells are torn down through their PTY child; there is no job object.
#[cfg(not(windows))]
pub struct TerminalJob;

#[cfg(not(windows))]
impl TerminalJob {
    fn terminate(&self) -> Result<(), String> {
        Ok(())
    }

    fn active_processes(&self) -> Result<u32, String> {
        Ok(0)
    }
}

/// Put a freshly spawned shell under process containment. A failure means the tree
/// cannot be torn down reliably, so the caller must kill the shell and report the error.
///
/// Known limit: assignment happens just after spawn. A process the shell starts before
/// assignment (for example from a profile script) is outside the job and is not reached
/// by `TerminateJobObject`. The window is milliseconds and the shell is idle at spawn.
/// Processes that break away from the job (`CREATE_BREAKAWAY_FROM_JOB`) are not covered.
#[cfg(windows)]
fn contain_shell(child: &dyn portable_pty::Child) -> Result<TerminalJob, String> {
    let pid = child
        .process_id()
        .ok_or_else(|| "shell has no process id".to_string())?;
    TerminalJob::adopt(pid)
}

#[cfg(not(windows))]
fn contain_shell(_child: &dyn portable_pty::Child) -> Result<TerminalJob, String> {
    Ok(TerminalJob)
}

/// Terminate a PTY session's shell and every process it spawned (dev servers,
/// watchers, ...). Idempotent. Returns `Err` when the tree may still be running, and the
/// caller keeps the session so the close can be retried. The job reaches descendants even
/// after the shell has exited, so a retry never depends on a stale pid.
fn terminate_terminal_tree(
    child: &mut dyn portable_pty::Child,
    job: &TerminalJob,
) -> Result<(), String> {
    const TEARDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

    job.terminate()?;
    if wait_for_tree_exit(child, job, TEARDOWN_WAIT)? {
        return Ok(());
    }

    // The job did not empty in time: kill the shell root directly and wait once more.
    if let Err(e) = child.kill() {
        if !matches!(child.try_wait(), Ok(Some(_))) {
            return Err(format!("failed to kill terminal shell: {e}"));
        }
    }
    if wait_for_tree_exit(child, job, TEARDOWN_WAIT)? {
        return Ok(());
    }
    Err(format!(
        "terminal process tree still running after {}s",
        TEARDOWN_WAIT.as_secs()
    ))
}

/// Preserve teardown failures on all post-spawn setup paths. Otherwise a failed
/// reader/writer acquisition or a racing duplicate can leave a shell running
/// while reporting only the original setup error.
fn terminate_failed_terminal_setup(
    child: &mut dyn portable_pty::Child,
    job: &TerminalJob,
    cause: String,
) -> String {
    match terminate_terminal_tree(child, job) {
        Ok(()) => cause,
        Err(teardown_error) => format!("{cause}; terminal cleanup failed: {teardown_error}"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TerminalIoFault {
    Reader,
    Writer,
}

type TerminalReader = Box<dyn std::io::Read + Send>;
type TerminalWriter = Box<dyn std::io::Write + Send>;

/// Keep both post-spawn acquisition failures on the same cleanup path.
/// Fault injection exercises the exact branch with a live contained PTY.
fn acquire_terminal_io(
    master: &dyn portable_pty::MasterPty,
    child: &mut dyn portable_pty::Child,
    job: &TerminalJob,
    fault: Option<TerminalIoFault>,
) -> Result<(TerminalReader, TerminalWriter), String> {
    let reader = if fault == Some(TerminalIoFault::Reader) {
        Err("injected reader failure".to_string())
    } else {
        master.try_clone_reader().map_err(|e| e.to_string())
    };
    let reader = reader.map_err(|e| {
        terminate_failed_terminal_setup(child, job, format!("Failed to clone PTY reader: {e}"))
    })?;
    let writer = if fault == Some(TerminalIoFault::Writer) {
        Err("injected writer failure".to_string())
    } else {
        master.take_writer().map_err(|e| e.to_string())
    };
    let writer = writer.map_err(|e| {
        terminate_failed_terminal_setup(child, job, format!("Failed to take PTY writer: {e}"))
    })?;
    Ok((reader, writer))
}

/// Poll until the shell has exited and the job holds no live process, or `timeout` passes.
/// Returns `Ok(true)` when the tree is gone.
fn wait_for_tree_exit(
    child: &mut dyn portable_pty::Child,
    job: &TerminalJob,
    timeout: std::time::Duration,
) -> Result<bool, String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let shell_exited = matches!(child.try_wait(), Ok(Some(_)));
        if shell_exited && job.active_processes()? == 0 {
            return Ok(true);
        }
        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

/// Destroy a terminal session. Idempotent: a missing session is already gone.
/// If the process tree cannot be terminated the session stays registered so
/// the close can be retried, and the error is returned to the caller.
#[tauri::command]
pub async fn ide_terminal_destroy(
    state: State<'_, AppState>,
    terminal_id: String,
) -> Result<(), String> {
    let mut registry = state.terminals.lock().await;
    let Some(session) = registry.sessions.get_mut(&terminal_id) else {
        return Ok(());
    };
    // Drop writer to signal EOF
    session.writer.take();
    terminate_terminal_tree(session.child.as_mut(), &session.job)?;
    registry.sessions.remove(&terminal_id);
    Ok(())
}

/// Number of live PTY terminal sessions.
#[tauri::command]
pub async fn ide_terminal_count(state: State<'_, AppState>) -> Result<usize, String> {
    let registry = state.terminals.lock().await;
    Ok(registry.sessions.len())
}

/// Aggregate owned PTY resources without exposing terminal IDs or shell output.
/// An unavailable job census is explicit; it must not be mistaken for zero.
#[derive(Debug, Serialize)]
pub struct TerminalResourceSnapshot {
    pub registered_sessions: usize,
    pub active_processes: Option<u64>,
    pub query_failures: usize,
}

fn terminal_resource_snapshot(registry: &TerminalRegistry) -> TerminalResourceSnapshot {
    // Only the Windows branch mutates these; other platforms keep the zero defaults.
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut active_processes = 0u64;
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut query_failures = 0usize;
    #[cfg(windows)]
    for session in registry.sessions.values() {
        match session.job.active_processes() {
            Ok(count) => active_processes += u64::from(count),
            Err(_) => query_failures += 1,
        }
    }
    #[cfg(not(windows))]
    let _ = registry;
    TerminalResourceSnapshot {
        registered_sessions: registry.sessions.len(),
        active_processes: if cfg!(windows) && query_failures == 0 {
            Some(active_processes)
        } else {
            None
        },
        query_failures,
    }
}

#[tauri::command]
pub async fn ide_terminal_resource_snapshot(
    state: State<'_, AppState>,
) -> Result<TerminalResourceSnapshot, String> {
    let registry = state.terminals.lock().await;
    Ok(terminal_resource_snapshot(&registry))
}

/// Terminate every terminal session (e.g. when closing the project).
/// Sessions whose process tree could not be terminated stay registered and
/// their failures are returned to the caller.
#[tauri::command]
pub async fn ide_terminal_destroy_all(state: State<'_, AppState>) -> Result<(), String> {
    let mut registry = state.terminals.lock().await;
    let mut failures = Vec::new();
    registry.sessions.retain(|id, session| {
        session.writer.take();
        match terminate_terminal_tree(session.child.as_mut(), &session.job) {
            Ok(()) => false,
            Err(e) => {
                failures.push(format!("terminal '{id}': {e}"));
                true
            }
        }
    });
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
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

#[cfg(test)]
mod terminal_tree_tests {
    use super::{
        acquire_terminal_io, contain_shell, register_terminal_session, terminal_resource_snapshot,
        terminate_failed_terminal_setup, terminate_terminal_tree, TerminalIoFault, TerminalJob,
        TerminalOutputLog, TerminalRegistry, TerminalSession,
    };
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    #[cfg(windows)]
    use std::time::{Duration, Instant};

    /// Open a real PTY and spawn `cmd` in it. The master must stay alive for
    /// as long as the session is used, exactly as `ide_terminal_create` does.
    fn spawn_pty(
        cmd: CommandBuilder,
    ) -> (
        Box<dyn portable_pty::Child + Send>,
        Box<dyn portable_pty::MasterPty + Send>,
    ) {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");
        let child = pair.slave.spawn_command(cmd).expect("spawn pty child");
        (child, pair.master)
    }

    /// Spawn a shell and contain it, as `ide_terminal_create` does.
    fn spawn_contained(
        cmd: CommandBuilder,
    ) -> (
        Box<dyn portable_pty::Child + Send>,
        Box<dyn portable_pty::MasterPty + Send>,
        TerminalJob,
    ) {
        let (child, master) = spawn_pty(cmd);
        let job = contain_shell(&*child).expect("contain shell");
        (child, master, job)
    }

    fn sleeping_session() -> TerminalSession {
        #[cfg(windows)]
        let mut cmd = CommandBuilder::new("powershell.exe");
        #[cfg(windows)]
        cmd.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
        #[cfg(not(windows))]
        let mut cmd = CommandBuilder::new("sh");
        #[cfg(not(windows))]
        cmd.args(["-c", "sleep 30"]);
        let (mut child, master, job) = spawn_contained(cmd);
        let (_reader, writer) =
            acquire_terminal_io(master.as_ref(), child.as_mut(), &job, None).unwrap();
        TerminalSession {
            child,
            writer: Some(writer),
            output: std::sync::Arc::new(std::sync::Mutex::new(TerminalOutputLog::default())),
            master,
            job,
        }
    }

    #[test]
    fn setup_failure_terminates_spawned_shell() {
        #[cfg(windows)]
        let mut cmd = CommandBuilder::new("powershell.exe");
        #[cfg(windows)]
        cmd.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
        #[cfg(not(windows))]
        let mut cmd = CommandBuilder::new("sh");
        #[cfg(not(windows))]
        cmd.args(["-c", "sleep 30"]);
        let (mut child, _master, job) = spawn_contained(cmd);
        let error =
            terminate_failed_terminal_setup(child.as_mut(), &job, "injected writer failure".into());
        assert_eq!(error, "injected writer failure");
        assert!(child.try_wait().expect("query shell exit").is_some());
        assert_eq!(job.active_processes().expect("query job"), 0);
    }

    #[test]
    fn reader_and_writer_acquisition_failures_reap_the_shell() {
        for fault in [TerminalIoFault::Reader, TerminalIoFault::Writer] {
            #[cfg(windows)]
            let mut cmd = CommandBuilder::new("powershell.exe");
            #[cfg(windows)]
            cmd.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
            #[cfg(not(windows))]
            let mut cmd = CommandBuilder::new("sh");
            #[cfg(not(windows))]
            cmd.args(["-c", "sleep 30"]);
            let (mut child, master, job) = spawn_contained(cmd);
            let error =
                match acquire_terminal_io(master.as_ref(), child.as_mut(), &job, Some(fault)) {
                    Err(error) => error,
                    Ok(_) => panic!("injected fault should fail"),
                };
            assert!(error.contains("injected"), "{error}");
            assert!(child.try_wait().expect("query shell exit").is_some());
            assert_eq!(job.active_processes().expect("query job"), 0);
        }
    }

    #[test]
    fn duplicate_registration_preserves_first_shell_and_reaps_second() {
        let mut registry = TerminalRegistry::new();
        register_terminal_session(&mut registry, "same-id".into(), sleeping_session())
            .unwrap_or_else(|_| panic!("first registration failed"));
        let live = terminal_resource_snapshot(&registry);
        assert_eq!(live.registered_sessions, 1);
        #[cfg(windows)]
        assert_eq!(live.active_processes, Some(1));
        let mut rejected =
            match register_terminal_session(&mut registry, "same-id".into(), sleeping_session()) {
                Err(rejected) => rejected,
                Ok(()) => panic!("duplicate replaced the first session"),
            };
        let error = terminate_failed_terminal_setup(
            rejected.child.as_mut(),
            &rejected.job,
            "Terminal 'same-id' already exists".into(),
        );
        assert!(error.contains("already exists"), "{error}");
        assert!(rejected
            .child
            .try_wait()
            .expect("query rejected shell")
            .is_some());
        assert_eq!(
            rejected.job.active_processes().expect("query rejected job"),
            0
        );
        assert_eq!(registry.sessions.len(), 1);
        let after_rejection = terminal_resource_snapshot(&registry);
        assert_eq!(after_rejection.registered_sessions, 1);
        #[cfg(windows)]
        assert_eq!(after_rejection.active_processes, Some(1));
        let first = registry.sessions.get_mut("same-id").unwrap();
        assert!(first.child.try_wait().expect("query first shell").is_none());
        terminate_terminal_tree(first.child.as_mut(), &first.job).expect("reap first shell");
        registry.sessions.remove("same-id");
        let closed = terminal_resource_snapshot(&registry);
        assert_eq!(closed.registered_sessions, 0);
        #[cfg(windows)]
        assert_eq!(closed.active_processes, Some(0));
    }

    #[test]
    fn simultaneous_terminal_registrations_keep_one_owned_shell() {
        use std::sync::{Arc, Barrier};

        let registry = Arc::new(tokio::sync::Mutex::new(TerminalRegistry::new()));
        let barrier = Arc::new(Barrier::new(2));
        let contenders: Vec<_> = (0..2)
            .map(|_| {
                let registry = Arc::clone(&registry);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let session = sleeping_session();
                    barrier.wait();
                    let mut guard = registry.blocking_lock();
                    let registered =
                        register_terminal_session(&mut guard, "race-id".into(), session);
                    drop(guard);
                    match registered {
                        Ok(()) => true,
                        Err(mut rejected) => {
                            terminate_failed_terminal_setup(
                                rejected.child.as_mut(),
                                &rejected.job,
                                "duplicate race".into(),
                            );
                            assert_eq!(rejected.job.active_processes().unwrap(), 0);
                            false
                        }
                    }
                })
            })
            .collect();
        let successes: usize = contenders
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum();
        assert_eq!(successes, 1);
        let mut guard = registry.blocking_lock();
        let live = terminal_resource_snapshot(&guard);
        assert_eq!(live.registered_sessions, 1);
        #[cfg(windows)]
        assert_eq!(live.active_processes, Some(1));
        let mut winner = guard.sessions.remove("race-id").unwrap();
        terminate_terminal_tree(winner.child.as_mut(), &winner.job).unwrap();
        assert_eq!(winner.job.active_processes().unwrap(), 0);
        assert_eq!(terminal_resource_snapshot(&guard).registered_sessions, 0);
    }

    #[test]
    fn terminate_is_idempotent_for_exited_shell() {
        #[cfg(windows)]
        let cmd = {
            let mut c = CommandBuilder::new("cmd.exe");
            c.args(["/C", "exit 0"]);
            c
        };
        #[cfg(not(windows))]
        let cmd = {
            let mut c = CommandBuilder::new("sh");
            c.args(["-c", "exit 0"]);
            c
        };
        let (mut child, _master, job) = spawn_contained(cmd);
        child.wait().expect("wait for shell exit");
        assert!(terminate_terminal_tree(child.as_mut(), &job).is_ok());
        assert!(terminate_terminal_tree(child.as_mut(), &job).is_ok());
    }

    #[cfg(windows)]
    fn pid_alive(pid: u32) -> bool {
        let filter = format!("PID eq {pid}");
        let out = piscis_kernel::proc::std_command("tasklist")
            .args(["/FI", filter.as_str(), "/FO", "CSV", "/NH"])
            .output()
            .expect("run tasklist");
        // A matching row is CSV with the PID quoted; the "no tasks" INFO line is not.
        String::from_utf8_lossy(&out.stdout).contains(&format!(",\"{pid}\""))
    }

    /// Real process-tree case on Windows: the PTY shell starts a grandchild
    /// (`ping`), we close the terminal and assert the grandchild is gone too.
    #[cfg(windows)]
    #[test]
    fn terminate_kills_descendant_processes_on_windows() {
        let pid_file =
            std::env::temp_dir().join(format!("agentz-term-{}.pid", uuid::Uuid::new_v4()));
        let script = format!(
            "$p = Start-Process -PassThru ping -ArgumentList '-n','300','127.0.0.1'; \
             Set-Content -LiteralPath '{}' -Value $p.Id; Start-Sleep -Seconds 300",
            pid_file.display()
        );
        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", script.as_str()]);
        let (mut child, _master, job) = spawn_contained(cmd);

        let deadline = Instant::now() + Duration::from_secs(60);
        let descendant = loop {
            if let Some(pid) = std::fs::read_to_string(&pid_file)
                .ok()
                .and_then(|t| t.trim().parse::<u32>().ok())
            {
                break pid;
            }
            assert!(
                Instant::now() < deadline,
                "shell never published a descendant pid"
            );
            std::thread::sleep(Duration::from_millis(200));
        };
        let _ = std::fs::remove_file(&pid_file);
        assert!(pid_alive(descendant), "descendant should run before close");

        let result = terminate_terminal_tree(child.as_mut(), &job);

        let deadline = Instant::now() + Duration::from_secs(10);
        while pid_alive(descendant) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        let survived = pid_alive(descendant);
        if survived {
            // Best-effort cleanup so a failing run does not leave `ping` behind.
            let pid = descendant.to_string();
            let _ = piscis_kernel::proc::std_command("taskkill")
                .args(["/F", "/PID", pid.as_str()])
                .output();
        }
        assert!(result.is_ok(), "terminate failed: {result:?}");
        assert!(
            matches!(child.try_wait(), Ok(Some(_))),
            "shell should have exited"
        );
        assert!(!survived, "descendant {descendant} survived terminal close");
    }

    /// Regression for the failed-close retry: the shell is gone (root killed, as after a
    /// partial taskkill) but a descendant survives. The job must still reach it, with
    /// no pid lookup involved.
    #[cfg(windows)]
    #[test]
    fn terminate_reaches_descendants_after_shell_exits() {
        let pid_file =
            std::env::temp_dir().join(format!("agentz-term-{}.pid", uuid::Uuid::new_v4()));
        let script = format!(
            "$p = Start-Process -PassThru ping -ArgumentList '-n','300','127.0.0.1'; \
             Set-Content -LiteralPath '{}' -Value $p.Id; Start-Sleep -Seconds 300",
            pid_file.display()
        );
        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", script.as_str()]);
        let (mut child, _master, job) = spawn_contained(cmd);

        let deadline = Instant::now() + Duration::from_secs(60);
        let descendant = loop {
            if let Some(pid) = std::fs::read_to_string(&pid_file)
                .ok()
                .and_then(|t| t.trim().parse::<u32>().ok())
            {
                break pid;
            }
            assert!(
                Instant::now() < deadline,
                "shell never published a descendant pid"
            );
            std::thread::sleep(Duration::from_millis(200));
        };
        let _ = std::fs::remove_file(&pid_file);

        // Kill only the shell, leaving the descendant running.
        child.kill().expect("kill shell root");
        child.wait().expect("reap shell root");
        assert!(
            pid_alive(descendant),
            "descendant should outlive the shell root"
        );

        let result = terminate_terminal_tree(child.as_mut(), &job);

        let deadline = Instant::now() + Duration::from_secs(10);
        while pid_alive(descendant) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        let survived = pid_alive(descendant);
        if survived {
            let pid = descendant.to_string();
            let _ = piscis_kernel::proc::std_command("taskkill")
                .args(["/F", "/PID", pid.as_str()])
                .output();
        }
        assert!(result.is_ok(), "terminate failed: {result:?}");
        assert!(
            !survived,
            "descendant {descendant} survived terminal close after shell exit"
        );
    }
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
    use std::sync::atomic::{AtomicBool, Ordering};

    // Remote: the server watches on its side; no local index/graph workers.
    if crate::remote::is_remote(&project_dir) {
        let session = crate::remote::bind(&project_dir).await?;
        return session
            .manager
            .watch_remote(&project_dir, &session.path)
            .await;
    }

    let root = PathBuf::from(&project_dir);
    if !root.exists() {
        return Err(format!("Directory not found: {}", project_dir));
    }

    // Serialize start/stop so concurrent mounts share one watcher and worker.
    let mut watchers = state.file_watchers.lock().await;
    if let Some(watcher) = watchers.get_mut(&project_dir) {
        return watcher.retain();
    }

    let active = Arc::new(AtomicBool::new(true));
    let worker_active = Arc::clone(&active);
    let index_root = root.clone();
    let index_worker = crate::index_worker::IndexWorker::start(move |batch| {
        if !worker_active.load(Ordering::Acquire) {
            return;
        }
        if batch.rebuild {
            if let Err(error) = crate::commands::codebase::build_index(&index_root) {
                tracing::warn!(%error, "Codebase rebuild failed");
            }
            crate::commands::graph_index::request_rebuild_if_active(
                index_root.clone(),
                &worker_active,
            );
        } else {
            for rel in batch.paths {
                if !worker_active.load(Ordering::Acquire) {
                    break;
                }
                if let Err(error) = crate::commands::codebase::index_file(&index_root, &rel) {
                    tracing::warn!(%error, path = %rel, "Incremental index failed");
                }
                crate::commands::graph::schedule_patch_if_active(
                    index_root.clone(),
                    rel,
                    &worker_active,
                );
            }
        }
    })
    .map_err(|e| format!("Failed to start index worker: {e}"))?;
    let app_clone = app.clone();
    let dir = project_dir.clone();
    let callback_active = Arc::clone(&active);

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if !callback_active.load(Ordering::Acquire) {
                return;
            }
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
                                notify::EventKind::Remove(_) => {
                                    // Atomic save (write temp + rename over the
                                    // target — our own ide_write_file included)
                                    // surfaces as a Remove event for the target
                                    // on Windows, but the file is still there.
                                    // Only a path that is actually gone counts
                                    // as a deletion; otherwise it is a modify.
                                    if path.exists() {
                                        "modified"
                                    } else {
                                        "deleted"
                                    }
                                }
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
    .map_err(|e| {
        active.store(false, Ordering::Release);
        format!("Failed to create watcher: {}", e)
    })?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| {
            active.store(false, Ordering::Release);
            format!("Failed to watch: {}", e)
        })?;

    crate::commands::graph_index::ensure_started(&root);

    // Store the watcher to keep it alive
    watchers.insert(
        project_dir,
        crate::state::OwnedFileWatcher::new(watcher, active),
    );

    Ok(())
}

/// Stop watching a project directory.
#[tauri::command]
pub async fn ide_stop_watcher(
    state: State<'_, AppState>,
    project_dir: String,
) -> Result<(), String> {
    if crate::remote::is_remote(&project_dir) {
        crate::remote::vfs::parse(&project_dir).ok_or("invalid remote workspace URI")?;
        // The broker uses the registration's authority/generation; stopping an
        // old watcher must not unwatch a replacement host's token.
        state.ext_host.unwatch_remote(&project_dir).await;
        return Ok(());
    }
    let mut watchers = state.file_watchers.lock().await;
    let removed = crate::state::release_file_watcher(&mut watchers, &project_dir);
    drop(watchers);
    if removed {
        crate::commands::graph_index::forget_project(Path::new(&project_dir));
    }
    Ok(())
}

/// Count host-owned local filesystem watchers without exposing project paths.
#[tauri::command]
pub async fn ide_watcher_count(state: State<'_, AppState>) -> Result<usize, String> {
    Ok(state.file_watchers.lock().await.len())
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Build a `git` command that never opens a console window on Windows.
///
/// Thin wrapper over [`piscis_kernel::proc::tokio_command`] kept as a named
/// helper so all git invocations remain grep-able.
fn new_git_cmd() -> Command {
    let mut cmd = tokio_command("git");
    cmd.args(GIT_UNQUOTED_PATHS);
    cmd
}

/// Non-ASCII paths are octal-escaped (`"\346\272\220.rs"`) in line-based git
/// output unless quoting is off; `-z` output is unaffected either way.
pub(crate) const GIT_UNQUOTED_PATHS: [&str; 2] = ["-c", "core.quotepath=false"];

fn with_unquoted_paths<'a>(args: &[&'a str]) -> Vec<&'a str> {
    GIT_UNQUOTED_PATHS.iter().copied().chain(args.iter().copied()).collect()
}

async fn run_git_cmd(dir: &Path, args: &[&str]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&run_git_cmd_bytes(dir, args).await?).to_string())
}

async fn run_git_cmd_bytes(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    if crate::remote::is_remote(&dir.to_string_lossy()) {
        let (session, remote_dir) = if let Some(session) = crate::remote::bound_session() {
            let path = crate::remote::resolve(&dir.to_string_lossy())
                .await?
                .ok_or("remote git working directory is not remote")?;
            (session, path)
        } else {
            let session = crate::remote::bind(&dir.to_string_lossy()).await?;
            let path = session.path.clone();
            (session, path)
        };
        let v = session.call(
            "exec",
            serde_json::json!({ "command": "git", "args": with_unquoted_paths(args), "cwd": remote_dir, "timeoutMs": 30_000 }),
        )
        .await?;
        if v.get("code").and_then(|c| c.as_i64()) != Some(0) {
            let stderr = v.get("stderr").and_then(|s| s.as_str()).unwrap_or_default();
            return Err(format!("git error: {}", stderr.trim()));
        }
        return Ok(v
            .get("stdout")
            .and_then(|s| s.as_str())
            .ok_or("bad remote git stdout reply")?
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
    if crate::remote::is_remote(&project_dir) {
        let uri = crate::remote::vfs::parse(&project_dir).ok_or("invalid remote workspace URI")?;
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
