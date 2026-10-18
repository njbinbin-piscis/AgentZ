//! Read-only review of the complete current checkout relative to a frozen base.
use serde::Serialize;
use std::{collections::BTreeMap, io::Write, path::Path, process::Stdio};

#[derive(Debug, Serialize)]
pub struct ChangeError {
    pub code: &'static str,
    pub message: String,
}
impl ChangeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn environment(message: String) -> Self {
        Self::new("environment_invalid", message)
    }
    pub fn worker(message: String) -> Self {
        Self::new("worker_failed", message)
    }
}
impl std::fmt::Display for ChangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for ChangeError {}

#[derive(Debug, Serialize)]
pub struct Change {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub binary: bool,
    pub sources: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ChangeSet {
    pub head_sha: String,
    pub base_sha: String,
    pub changes: Vec<Change>,
    pub excluded_runtime_paths: Vec<String>,
    pub has_uncommitted: bool,
}

fn runtime_path(path: &str) -> bool {
    path.split('/')
        .next()
        .is_some_and(|p| [".agentz", ".codez", ".piscis"].contains(&p))
}

#[derive(Debug, Serialize)]
pub struct FileView {
    pub path: String,
    pub original: Option<String>,
    pub modified: Option<String>,
    pub binary: bool,
    pub review_version: String,
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, ChangeError> {
    let output = crate::git_process::command()
        .arg("--literal-pathspecs")
        .arg("-C")
        .arg(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .output()
        .map_err(|e| ChangeError::new("git_launch_failed", format!("git launch failed: {e}")))?;
    if !output.status.success() {
        return Err(ChangeError::new(
            "git_failed",
            format!(
                "git {} failed: {}",
                args.first().unwrap_or(&""),
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    Ok(output.stdout)
}

fn blob_id(root: &Path, bytes: &[u8]) -> Result<String, ChangeError> {
    let mut child = crate::git_process::command()
        .arg("-C")
        .arg(root)
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ChangeError::new("git_launch_failed", e.to_string()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| ChangeError::new("worker_failed", "missing Git stdin"))?
        .write_all(bytes)
        .map_err(|e| ChangeError::new("git_failed", e.to_string()))?;
    let out = child
        .wait_with_output()
        .map_err(|e| ChangeError::new("git_failed", e.to_string()))?;
    if !out.status.success() {
        return Err(ChangeError::new(
            "git_failed",
            String::from_utf8_lossy(&out.stderr),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().into())
}

/// A captured file view; version identifies the actual bytes shown, not just HEAD.
pub fn file_view(root: &Path, base: &str, path: &str) -> Result<FileView, ChangeError> {
    let set = collect(root, base)?;
    let change = set.changes.iter().find(|c| c.path == path).ok_or_else(|| {
        ChangeError::new(
            "path_not_in_change_set",
            "path is not a current task change",
        )
    })?;
    let original_path = change.old_path.as_deref().unwrap_or(path);
    let tree = git(root, &["ls-tree", "-z", &set.base_sha, "--", original_path])?;
    let entry = fields(&tree)?.first().copied();
    let original = if let Some(entry) = entry {
        let metadata = entry
            .split_once('\t')
            .ok_or_else(|| ChangeError::new("invalid_git_output", "invalid tree entry"))?
            .0;
        let oid = metadata
            .split_whitespace()
            .last()
            .ok_or_else(|| ChangeError::new("invalid_git_output", "missing blob id"))?;
        Some(git(root, &["cat-file", "blob", oid])?)
    } else {
        None
    };
    let absolute = root.join(path);
    let modified = match std::fs::symlink_metadata(&absolute) {
        Ok(meta) if meta.file_type().is_symlink() => Some(
            std::fs::read_link(&absolute)
                .map_err(|e| ChangeError::new("file_read_failed", e.to_string()))?
                .to_str()
                .ok_or_else(|| ChangeError::new("unsupported_path", "non-UTF8 symlink target"))?
                .as_bytes()
                .to_vec(),
        ),
        Ok(meta) if meta.is_file() => Some(
            std::fs::read(&absolute)
                .map_err(|e| ChangeError::new("file_read_failed", e.to_string()))?,
        ),
        Ok(_) => {
            return Err(ChangeError::new(
                "unsupported_file",
                "directory/submodule preview is not supported",
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(ChangeError::new("file_read_failed", e.to_string())),
    };
    let mut version = set.base_sha;
    for bytes in [&original, &modified] {
        version.push(':');
        version.push_str(&match bytes {
            Some(b) => blob_id(root, b)?,
            None => "absent".into(),
        });
    }
    let binary = change.binary
        || [&original, &modified]
            .into_iter()
            .flatten()
            .any(|bytes| bytes.contains(&0) || std::str::from_utf8(bytes).is_err());
    Ok(FileView {
        path: path.into(),
        binary,
        review_version: version,
        original: if binary {
            None
        } else {
            original.map(|b| String::from_utf8(b).expect("UTF8 checked"))
        },
        modified: if binary {
            None
        } else {
            modified.map(|b| String::from_utf8(b).expect("UTF8 checked"))
        },
    })
}

fn fields(bytes: &[u8]) -> Result<Vec<&str>, ChangeError> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| {
            std::str::from_utf8(s)
                .map_err(|_| ChangeError::new("unsupported_path", "unsupported non-UTF8 Git path"))
        })
        .collect()
}

fn statuses(bytes: &[u8]) -> Result<Vec<(String, Option<String>, String)>, ChangeError> {
    let fields = fields(bytes)?;
    let mut entries = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        let status = fields[i];
        let rename = status.starts_with('R') || status.starts_with('C');
        let count = if rename { 3 } else { 2 };
        if i + count > fields.len() {
            return Err(ChangeError::new(
                "invalid_git_output",
                "truncated Git name-status output",
            ));
        }
        entries.push((
            fields[i + count - 1].into(),
            rename.then(|| fields[i + 1].into()),
            status.into(),
        ));
        i += count;
    }
    Ok(entries)
}

/// Includes committed, staged, unstaged and nonignored untracked paths. Git errors
/// are failures, never a misleading empty list. Reads only; index is unchanged.
pub fn collect(root: &Path, base: &str) -> Result<ChangeSet, ChangeError> {
    let head_sha = String::from_utf8(git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?)
        .map_err(|_| ChangeError::new("invalid_git_output", "invalid HEAD SHA"))?
        .trim()
        .to_owned();
    // Resolve first so an option-like/invalid ref cannot become a diff option.
    let sha = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{base}^{{commit}}"),
        ],
    )?;
    let sha = String::from_utf8(sha)
        .map_err(|_| ChangeError::new("invalid_git_output", "invalid Git SHA"))?
        .trim()
        .to_owned();
    let mut changes = BTreeMap::new();
    let mut has_uncommitted = false;
    for (path, old_path, status) in statuses(&git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--name-status",
            "-z",
            "--find-renames",
            &sha,
            "--",
        ],
    )?)? {
        changes.insert(
            path.clone(),
            Change {
                path,
                old_path,
                status,
                binary: false,
                sources: Vec::new(),
            },
        );
    }
    for (source, args) in [
        (
            "committed",
            vec![
                "diff",
                "--no-ext-diff",
                "--name-status",
                "-z",
                &sha,
                "HEAD",
                "--",
            ],
        ),
        (
            "staged",
            vec![
                "diff",
                "--no-ext-diff",
                "--name-status",
                "-z",
                "--cached",
                "--",
            ],
        ),
        (
            "unstaged",
            vec!["diff", "--no-ext-diff", "--name-status", "-z", "--"],
        ),
    ] {
        for (path, _, _) in statuses(&git(root, &args)?)? {
            if source != "committed" && !runtime_path(&path) {
                has_uncommitted = true;
            }
            if let Some(change) = changes.get_mut(&path) {
                change.sources.push(source.into());
            }
        }
    }
    // numstat uses '-' for binary according to Git's configured attributes.
    let stat = git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--numstat",
            "-z",
            "--no-renames",
            &sha,
            "--",
        ],
    )?;
    for row in fields(&stat)? {
        let mut columns = row.splitn(3, '\t');
        let added = columns.next();
        let deleted = columns.next();
        let path = columns.next();
        if let Some(change) = path.and_then(|p| changes.get_mut(p)) {
            change.binary = added == Some("-") || deleted == Some("-");
        }
    }
    let untracked = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let mut excluded_runtime_paths = Vec::new();
    changes.retain(|path, _| {
        let runtime = runtime_path(path);
        if runtime {
            excluded_runtime_paths.push(path.clone());
        }
        !runtime
    });
    for path in fields(&untracked)? {
        if runtime_path(path) {
            excluded_runtime_paths.push(path.into());
            continue;
        }
        has_uncommitted = true;
        let content = std::fs::read(root.join(path)).map_err(|e| {
            ChangeError::new("file_read_failed", format!("cannot read {path}: {e}"))
        })?;
        changes.insert(
            path.into(),
            Change {
                path: path.into(),
                old_path: None,
                status: "?".into(),
                binary: content.contains(&0) || std::str::from_utf8(&content).is_err(),
                sources: vec!["untracked".into()],
            },
        );
    }
    Ok(ChangeSet {
        head_sha: {
            let current =
                String::from_utf8(git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?)
                    .map_err(|_| ChangeError::new("invalid_git_output", "invalid HEAD SHA"))?;
            if current.trim() != head_sha {
                return Err(ChangeError::new(
                    "head_changed",
                    "task changed during review collection; refresh",
                ));
            }
            head_sha
        },
        base_sha: sha,
        changes: changes.into_values().collect(),
        excluded_runtime_paths,
        has_uncommitted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_checkout_and_special_paths() {
        let root = std::env::temp_dir().join(format!("agentz-changes-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-b", "main"]).unwrap();
        git(&root, &["config", "user.email", "fixture@example.invalid"]).unwrap();
        git(&root, &["config", "user.name", "Fixture"]).unwrap();
        git(&root, &["config", "core.autocrlf", "false"]).unwrap();
        for p in [
            "committed.txt",
            "staged.txt",
            "dirty.txt",
            "delete.txt",
            "rename.txt",
            "binary.bin",
            "canceled.txt",
        ] {
            // Distinct blobs make rename attribution unambiguous.
            std::fs::write(root.join(p), format!("{p}: base\n")).unwrap();
        }
        git(&root, &["add", "."]).unwrap();
        git(&root, &["commit", "-m", "base"]).unwrap();
        let base = String::from_utf8(git(&root, &["rev-parse", "HEAD"]).unwrap()).unwrap();
        std::fs::write(root.join("committed.txt"), "committed\n").unwrap();
        std::fs::write(root.join("canceled.txt"), "committed change\n").unwrap();
        git(&root, &["add", "."]).unwrap();
        git(&root, &["commit", "-m", "agent"]).unwrap();
        std::fs::write(root.join("canceled.txt"), "canceled.txt: base\n").unwrap();
        let net = collect(&root, base.trim()).unwrap();
        assert_eq!(net.changes.len(), 1);
        assert!(
            net.has_uncommitted,
            "a reverted net diff still leaves HEAD changes that must block merging"
        );
        std::fs::write(root.join("staged.txt"), "staged\n").unwrap();
        git(&root, &["add", "staged.txt"]).unwrap();
        std::fs::write(root.join("dirty.txt"), "dirty\n").unwrap();
        std::fs::remove_file(root.join("delete.txt")).unwrap();
        git(&root, &["mv", "rename.txt", "重命名 space.txt"]).unwrap();
        std::fs::write(root.join("binary.bin"), [0, 1, 2]).unwrap();
        std::fs::write(root.join("新增 space.txt"), "new\n").unwrap();
        std::fs::write(root.join("new.bin"), [0, 2]).unwrap();
        std::fs::create_dir(root.join(".agentz")).unwrap();
        std::fs::write(root.join(".agentz/journal.db"), "private runtime").unwrap();
        let index_before = git(&root, &["diff", "--cached", "--raw", "-z"]).unwrap();
        let set = collect(&root, base.trim()).unwrap();
        assert_eq!(set.changes.len(), 8);
        assert_eq!(set.excluded_runtime_paths, [".agentz/journal.db"]);
        let find = |name: &str| {
            set.changes
                .iter()
                .find(|c| c.path == name)
                .unwrap_or_else(|| panic!("missing {name}: {:?}", set.changes))
        };
        assert_eq!(find("committed.txt").sources, ["committed"]);
        assert_eq!(find("staged.txt").sources, ["staged"]);
        assert_eq!(find("dirty.txt").sources, ["unstaged"]);
        assert_eq!(find("delete.txt").status, "D");
        assert_eq!(
            find("重命名 space.txt").old_path.as_deref(),
            Some("rename.txt")
        );
        assert!(find("binary.bin").binary);
        assert!(find("new.bin").binary);
        assert_eq!(find("新增 space.txt").sources, ["untracked"]);
        assert_eq!(
            index_before,
            git(&root, &["diff", "--cached", "--raw", "-z"]).unwrap()
        );
        assert!(collect(&root, "nonexistent-ref").is_err());
        assert!(collect(&root, "--help").is_err());
        let view = file_view(&root, base.trim(), "dirty.txt").unwrap();
        assert_eq!(view.original.as_deref(), Some("dirty.txt: base\n"));
        assert_eq!(view.modified.as_deref(), Some("dirty\n"));
        std::fs::write(root.join("dirty.txt"), "another edit\n").unwrap();
        assert_ne!(
            view.review_version,
            file_view(&root, base.trim(), "dirty.txt")
                .unwrap()
                .review_version
        );
        let new = file_view(&root, base.trim(), "新增 space.txt").unwrap();
        assert!(new.original.is_none());
        assert_eq!(new.modified.as_deref(), Some("new\n"));
        let deleted = file_view(&root, base.trim(), "delete.txt").unwrap();
        assert!(deleted.original.is_some());
        assert!(deleted.modified.is_none());
        let renamed = file_view(&root, base.trim(), "重命名 space.txt").unwrap();
        assert_eq!(renamed.original, renamed.modified);
        let binary = file_view(&root, base.trim(), "binary.bin").unwrap();
        assert!(binary.binary);
        assert!(binary.modified.is_none());
        assert_eq!(
            file_view(&root, base.trim(), "../outside")
                .unwrap_err()
                .code,
            "path_not_in_change_set"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn nul_parser_preserves_tabs_and_newlines() {
        let entries = statuses(b"R100\0old\tname\0new\nname\0M\0tab\tname\0").unwrap();
        assert_eq!(entries[0].0, "new\nname");
        assert_eq!(entries[0].1.as_deref(), Some("old\tname"));
        assert_eq!(entries[1].0, "tab\tname");
        assert!(statuses(b"R100\0old\0").is_err());
    }
}
