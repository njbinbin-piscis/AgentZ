//! Discover and operate on multiple Git repositories under a workspace folder
//! (VS Code-style: parent folder without `.git` scans nested repos).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{BranchInfo, GitFileStatus};

fn status_char_to_string(c: char) -> String {
    match c {
        'M' => "modified",
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        'T' => "type_changed",
        _ => "unknown",
    }
    .to_string()
}

const GIT_DISCOVER_MAX_DEPTH: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepoSnapshot {
    /// Relative path from workspace root (`""` = workspace root is the repo).
    pub repo_root: String,
    /// Display label (folder name).
    pub name: String,
    pub files: Vec<GitFileStatus>,
    pub branches: Vec<BranchInfo>,
}

/// If the workspace root is a repo, return only it; otherwise scan nested folders.
pub fn discover_git_repos(workspace: &Path) -> Vec<PathBuf> {
    // Remote workspaces can't be scanned synchronously: use the list from the
    // last async discovery, else assume the root (git reports if it isn't).
    let ws = workspace.to_string_lossy();
    if crate::remote::is_remote(&ws) {
        return match crate::remote::cached_git_repos(&ws) {
            Some(rels) => rels
                .iter()
                .map(|r| if r.is_empty() { workspace.to_path_buf() } else { PathBuf::from(format!("{}/{}", ws.trim_end_matches('/'), r)) })
                .collect(),
            None => vec![workspace.to_path_buf()],
        };
    }
    if workspace.join(".git").exists() {
        return vec![workspace.to_path_buf()];
    }
    let mut repos = Vec::new();
    collect_nested_git_repos(workspace, 0, GIT_DISCOVER_MAX_DEPTH, &mut repos);
    repos.sort_by_key(|p| p.to_string_lossy().to_string());
    repos
}

fn collect_nested_git_repos(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    if dir.join(".git").exists() {
        out.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if crate::path_filter::is_ignored_dir_name(&name) {
            continue;
        }
        collect_nested_git_repos(&entry.path(), depth + 1, max_depth, out);
    }
}

pub fn repo_root_rel(workspace: &Path, repo: &Path) -> String {
    repo.strip_prefix(workspace)
        .unwrap_or(repo)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn repo_display_name(workspace: &Path, _repo: &Path, repo_root_rel: &str) -> String {
    if repo_root_rel.is_empty() {
        workspace
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repository".to_string())
    } else {
        repo_root_rel
            .rsplit('/')
            .next()
            .unwrap_or(repo_root_rel)
            .to_string()
    }
}

fn prefix_workspace_path(repo_root_rel: &str, path_in_repo: &str) -> String {
    if repo_root_rel.is_empty() {
        path_in_repo.to_string()
    } else {
        format!("{}/{}", repo_root_rel, path_in_repo)
    }
}

/// Resolve which git repo owns a workspace-relative file path.
pub fn resolve_git_context(
    workspace: &Path,
    workspace_rel_path: &str,
) -> Result<(PathBuf, String), String> {
    let path = workspace_rel_path.replace('\\', "/");
    let repos = discover_git_repos(workspace);
    if repos.is_empty() {
        return Err("no git repository found in workspace".into());
    }

    let sole_root_repo = repos.len() == 1;
    let mut best: Option<(PathBuf, String, usize)> = None;
    for repo in &repos {
        let rel = repo_root_rel(workspace, repo);
        let match_len = if rel.is_empty() {
            if sole_root_repo {
                usize::MAX
            } else {
                continue;
            }
        } else if path == rel || path.starts_with(&format!("{}/", rel)) {
            rel.len()
        } else {
            continue;
        };

        if best
            .as_ref()
            .map(|(_, _, l)| match_len > *l)
            .unwrap_or(true)
        {
            let path_in_repo = if rel.is_empty() {
                path.clone()
            } else {
                path.strip_prefix(&format!("{}/", rel))
                    .unwrap_or(&path)
                    .to_string()
            };
            best = Some((repo.clone(), path_in_repo, match_len));
        }
    }

    best.map(|(repo, path_in_repo, _)| (repo, path_in_repo))
        .ok_or_else(|| format!("no git repository owns path: {path}"))
}

/// Resolve a workspace-relative path within either the explicitly selected
/// repository or, when no repository was selected, the repository that owns
/// it.  Keeping the selection as part of this resolution prevents a nested
/// repository operation from silently being applied to a different checkout.
pub fn resolve_git_context_for_root(
    workspace: &Path,
    workspace_rel_path: &str,
    git_root: Option<&str>,
) -> Result<(PathBuf, String), String> {
    let path = normalize_workspace_rel_path(workspace_rel_path)?;
    let (inferred_root, path_in_repo) = resolve_git_context(workspace, &path)?;

    if let Some(selected) = git_root.filter(|root| !root.is_empty()) {
        let selected_root = resolve_git_dir(workspace, Some(selected), None)?;
        if selected_root != inferred_root {
            return Err(format!(
                "path '{path}' is not owned by selected git repository '{selected}'"
            ));
        }
    }

    Ok((inferred_root, path_in_repo))
}

/// Reject absolute and parent-traversal paths before joining them to a
/// workspace directory. Git commands always receive paths relative to their
/// resolved repository.
pub fn normalize_workspace_rel_path(path: &str) -> Result<String, String> {
    let normalized = path.replace('\\', "/");
    if normalized.is_empty() || normalized == "." {
        return Err("a concrete workspace-relative path is required".into());
    }
    let candidate = Path::new(&normalized);
    if candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(component, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))
        })
    {
        return Err(format!("path must stay within workspace: {path}"));
    }
    Ok(normalized)
}

/// Resolve git directory for an operation (explicit `git_root` or infer from file path).
pub fn resolve_git_dir(
    workspace: &Path,
    git_root: Option<&str>,
    workspace_file_path: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(rel) = git_root.filter(|s| !s.is_empty()) {
        let root = workspace.join(rel);
        if !crate::remote::is_remote(&root.to_string_lossy()) && !root.join(".git").exists() {
            return Err(format!("not a git repository: {rel}"));
        }
        return Ok(root);
    }
    if let Some(path) = workspace_file_path {
        return Ok(resolve_git_context(workspace, path)?.0);
    }
    let repos = discover_git_repos(workspace);
    match repos.len() {
        0 => Err("no git repository found in workspace".into()),
        1 => Ok(repos[0].clone()),
        _ => Err("multiple git repositories: specify git_root".into()),
    }
}

pub fn parse_git_status_output(output: &[u8], repo_root_rel: &str) -> Vec<GitFileStatus> {
    let mut statuses = Vec::new();
    let mut records = output.split(|byte| *byte == b'\0');
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let index_status = record[0] as char;
        let worktree_status = record[1] as char;
        if record[2] != b' ' {
            continue;
        }
        let path_in_repo = String::from_utf8_lossy(&record[3..]).to_string();
        let path = prefix_workspace_path(repo_root_rel, &path_in_repo);

        if index_status != ' ' && index_status != '?' {
            statuses.push(GitFileStatus {
                path: path.clone(),
                status: status_char_to_string(index_status),
                staged: true,
            });
        }
        if worktree_status != ' ' && worktree_status != '?' {
            statuses.push(GitFileStatus {
                path: path.clone(),
                status: status_char_to_string(worktree_status),
                staged: false,
            });
        }
        if index_status == '?' && worktree_status == '?' {
            statuses.push(GitFileStatus {
                path,
                status: "untracked".to_string(),
                staged: false,
            });
        }

        // With `--porcelain=v1 -z`, rename/copy source paths are emitted as
        // an additional NUL-delimited record. It is metadata, not another
        // status entry, so consume it before reading the next XY record.
        if matches!(index_status, 'R' | 'C') || matches!(worktree_status, 'R' | 'C') {
            let _ = records.next();
        }
    }
    statuses
}

pub fn parse_git_branches_output(output: &str) -> Vec<BranchInfo> {
    let mut branches = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() >= 2 {
            let name = parts[0].to_string();
            let is_current = parts[1] == "*";
            let last_commit = parts.get(2).map(|s| s.to_string());
            let last_commit_time = parts.get(3).map(|s| s.to_string());
            branches.push(BranchInfo {
                is_koi: name.starts_with("koi/"),
                name,
                is_current,
                last_commit,
                last_commit_time,
            });
        }
    }
    branches.sort_by(|a, b| {
        if a.is_current != b.is_current {
            return if a.is_current {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        a.name.cmp(&b.name)
    });
    branches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_repos_discovered_when_workspace_not_git() {
        let base = std::env::temp_dir().join(format!("agentz_git_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("alpha/.git")).unwrap();
        std::fs::create_dir_all(base.join("beta/.git")).unwrap();
        std::fs::create_dir_all(base.join("node_modules/pkg/.git")).unwrap();

        let repos = discover_git_repos(&base);
        let roots: Vec<String> = repos.iter().map(|p| repo_root_rel(&base, p)).collect();
        assert!(roots.contains(&"alpha".to_string()));
        assert!(roots.contains(&"beta".to_string()));
        assert!(!roots.contains(&"node_modules/pkg".to_string()));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn porcelain_z_preserves_newlines_and_rename_records() {
        let output = b" M ordinary.txt\0R  renamed.txt\0original.txt\0?? line\nbreak.txt\0";
        let statuses = parse_git_status_output(output, "nested");
        assert_eq!(statuses.len(), 3);
        assert_eq!(statuses[0].path, "nested/ordinary.txt");
        assert_eq!(statuses[1].path, "nested/renamed.txt");
        assert_eq!(statuses[2].path, "nested/line\nbreak.txt");
    }

    #[test]
    fn rejects_workspace_path_traversal() {
        assert!(normalize_workspace_rel_path("../outside.txt").is_err());
        assert!(normalize_workspace_rel_path("/outside.txt").is_err());
        assert_eq!(normalize_workspace_rel_path("nested/file.txt").unwrap(), "nested/file.txt");
    }
}
