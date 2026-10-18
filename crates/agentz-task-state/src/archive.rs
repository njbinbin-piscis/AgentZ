//! Append-only source snapshots. No worktree or branch deletion is performed here.
use crate::{
    canonical_root, load, registered_worktree, validate_managed, ManagedSnapshot, TaskEnvironment,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Archive {
    pub schema_version: u32,
    pub archive_id: String,
    pub environment: TaskEnvironment,
    pub head_sha: String,
    pub tree_sha: String,
    pub snapshot_commit: String,
    pub snapshot_ref: String,
    pub omitted_paths: Vec<String>,
    pub created_at: String,
}

fn valid_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 80 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("invalid archive identity".into());
    }
    Ok(())
}
pub(crate) fn git(
    root: &Path,
    index: Option<&Path>,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let mut cmd = crate::git_process::command();
    cmd.arg("-C")
        .arg(root)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Read-only status queries must not refresh the user's on-disk index.
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    // The metadata commit needs no user's global Git identity or hooks.
    cmd.env("GIT_AUTHOR_NAME", "AgentZ Archive")
        .env("GIT_AUTHOR_EMAIL", "archive@agentz.invalid")
        .env("GIT_COMMITTER_NAME", "AgentZ Archive")
        .env("GIT_COMMITTER_EMAIL", "archive@agentz.invalid");
    if let Some(index) = index {
        // Git for Windows rejects the verbatim prefix in this environment variable.
        let value = index.to_string_lossy();
        let value = if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            value.strip_prefix(r"\\?\").unwrap_or(&value).into()
        };
        cmd.env("GIT_INDEX_FILE", value);
    }
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    let mut child = cmd.spawn().map_err(|e| format!("git launch failed: {e}"))?;
    if let Some(bytes) = input {
        if let Err(e) = child
            .stdin
            .take()
            .ok_or("missing Git stdin")?
            .write_all(bytes)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Git input failed: {e}"));
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout)
}
pub(crate) fn string(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes)
        .map(|s| s.trim().into())
        .map_err(|_| "unsupported non-UTF8 Git output".into())
}
fn paths(bytes: Vec<u8>) -> Result<BTreeSet<String>, String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| {
            std::str::from_utf8(s)
                .map(str::to_owned)
                .map_err(|_| "unsupported non-UTF8 Git path".into())
        })
        .collect()
}
fn runtime(path: &str) -> bool {
    path.split('/')
        .next()
        .is_some_and(|p| [".agentz", ".codez", ".piscis"].contains(&p))
}
fn source_paths(root: &Path) -> Result<(BTreeSet<String>, BTreeSet<String>), String> {
    let mut included = paths(git(root, None, &["ls-files", "--cached", "-z"], None)?)?;
    let mut omitted = paths(git(
        root,
        None,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ],
        None,
    )?)?;
    for path in paths(git(
        root,
        None,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        None,
    )?)? {
        if runtime(&path) {
            omitted.insert(path);
        } else {
            included.insert(path);
        }
    }
    Ok((included, omitted))
}
pub(crate) fn check_record(project: &Path, record: &TaskEnvironment) -> Result<(), String> {
    let root = Path::new(&record.workspace_root);
    let branch = string(git(
        root,
        None,
        &["rev-parse", "--abbrev-ref", "HEAD"],
        None,
    )?)?;
    let worktrees = String::from_utf8(git(
        project,
        None,
        &["worktree", "list", "--porcelain", "-z"],
        None,
    )?)
    .map_err(|_| "unsupported non-UTF8 worktree path")?;
    let common = |dir: &Path| -> Result<String, String> {
        let value = string(git(
            dir,
            None,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            None,
        )?)?;
        canonical_root(Path::new(&value))
    };
    validate_managed(
        record,
        &ManagedSnapshot {
            workspace_root: canonical_root(root)?,
            project_git_dir: common(project)?,
            workspace_git_dir: common(root)?,
            registered: registered_worktree(&worktrees, &record.workspace_root, &branch),
            branch,
        },
    )
}
struct Scratch {
    index: PathBuf,
    lock: PathBuf,
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.index);
        let _ = fs::remove_file(self.index.with_extension("index.lock"));
        let _ = fs::remove_file(&self.lock);
    }
}

fn file_bytes(root: &Path, path: &str) -> Result<Option<Vec<u8>>, String> {
    let absolute = root.join(path);
    match fs::symlink_metadata(&absolute) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
            fs::read(absolute).map(Some).map_err(|e| e.to_string())
        }
        Ok(_) => Err(format!(
            "unsupported archive entry (symlink/submodule/directory): {path}"
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot snapshot {path}: {e}")),
    }
}

pub(crate) fn capture_source(
    root: &Path,
    head_sha: &str,
    index: &Path,
) -> Result<(String, Vec<String>), String> {
    let (included, omitted) = source_paths(root)?;
    // Independent tree includes HEAD paths so staged removals are captured too.
    git(root, Some(index), &["read-tree", head_sha], None)?;
    let baseline = paths(git(
        root,
        Some(index),
        &["ls-files", "--cached", "-z"],
        None,
    )?)?;
    let all: BTreeSet<_> = included.union(&baseline).cloned().collect();
    let stage = git(root, Some(index), &["ls-files", "--stage", "-z"], None)?;
    let mut modes = BTreeMap::new();
    for entry in stage.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = std::str::from_utf8(entry).map_err(|_| "unsupported index path")?;
        let (meta, path) = entry.split_once('\t').ok_or("invalid index entry")?;
        modes.insert(
            path.to_string(),
            meta.split_whitespace()
                .next()
                .ok_or("invalid mode")?
                .to_string(),
        );
    }
    let mut captured = BTreeMap::new();
    let mut input = Vec::new();
    let zero = "0".repeat(head_sha.len());
    for path in &all {
        let bytes = file_bytes(root, path)?;
        let (mode, oid) = if let Some(bytes) = &bytes {
            let mode = modes.get(path).map(String::as_str).unwrap_or("100644");
            if mode != "100644" && mode != "100755" {
                return Err(format!("unsupported archive mode: {path}"));
            }
            (
                mode,
                string(git(
                    root,
                    None,
                    &["hash-object", "-w", "--stdin", "--no-filters"],
                    Some(bytes),
                )?)?,
            )
        } else {
            ("0", zero.clone())
        };
        input.extend_from_slice(format!("{mode} {oid}\t{path}").as_bytes());
        input.push(0);
        captured.insert(path.clone(), bytes);
    }
    if !input.is_empty() {
        git(
            root,
            Some(index),
            &["update-index", "-z", "--index-info"],
            Some(&input),
        )?;
    }
    let tree_sha = string(git(root, Some(index), &["write-tree"], None)?)?;
    // Refuse a source that changed during capture. Keep original files/index intact.
    if string(git(root, None, &["rev-parse", "HEAD"], None)?)? != head_sha
        || source_paths(root)? != (included, omitted.clone())
    {
        return Err("workspace changed during archive capture; retry without cleanup".into());
    }
    for (path, bytes) in &captured {
        if &file_bytes(root, path)? != bytes {
            return Err(format!(
                "file changed during snapshot: {path}; retry without cleanup"
            ));
        }
    }
    Ok((tree_sha, omitted.into_iter().collect()))
}

/// Preserve HEAD ancestry plus all current tracked and supported new source bytes.
/// Any omitted file means this snapshot alone is NOT a cleanup authorization.
pub fn create(project: &Path, task: &str, id: &str, created_at: &str) -> Result<Archive, String> {
    valid_id(id)?;
    let environment = load(project, task)?.ok_or("task environment missing")?;
    check_record(project, &environment)?;
    let directory = project.join(".agentz/archives");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let directory = fs::canonicalize(directory).map_err(|e| e.to_string())?;
    let metadata = directory.join(format!("{id}.json"));
    if metadata.exists() {
        return Err("archive identity already exists".into());
    }
    let lock = directory.join(format!("{id}.lock"));
    let lock_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|e| format!("archive busy: {e}"))?;
    let index = directory.join(format!("{id}.index"));
    drop(lock_file);
    if index.exists() || index.with_extension("index.lock").exists() {
        let _ = fs::remove_file(&lock);
        return Err("stale archive index; retain it for diagnosis".into());
    }
    let scratch = Scratch { index, lock };
    let root = Path::new(&environment.workspace_root);
    let head_sha = string(git(root, None, &["rev-parse", "--verify", "HEAD"], None)?)?;
    let (tree_sha, omitted) = capture_source(root, &head_sha, &scratch.index)?;
    check_record(project, &environment)?;
    if load(project, task)?.as_ref() != Some(&environment) {
        return Err("task binding changed during archive capture".into());
    }
    let snapshot_commit = string(git(
        root,
        None,
        &["commit-tree", &tree_sha, "-p", &head_sha],
        Some(b"AgentZ recoverable task snapshot\n"),
    )?)?;
    let snapshot_ref = format!("refs/agentz/archives/{id}");
    let zero = "0".repeat(head_sha.len());
    git(
        project,
        None,
        &["update-ref", &snapshot_ref, &snapshot_commit, &zero],
        None,
    )?;
    let archive = Archive {
        schema_version: 1,
        archive_id: id.into(),
        environment,
        head_sha,
        tree_sha,
        snapshot_commit,
        snapshot_ref,
        omitted_paths: omitted.into_iter().collect(),
        created_at: created_at.into(),
    };
    let temporary = directory.join(format!("{id}.json.tmp"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| {
            format!(
                "snapshot ref {} retained; metadata create failed: {e}",
                archive.snapshot_ref
            )
        })?;
    file.write_all(&serde_json::to_vec_pretty(&archive).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&temporary, metadata)
        .map_err(|e| format!("snapshot ref retained; metadata publish failed: {e}"))?;
    Ok(archive)
}

pub fn read(project: &Path, id: &str) -> Result<Archive, String> {
    valid_id(id)?;
    let bytes = fs::read(project.join(".agentz/archives").join(format!("{id}.json")))
        .map_err(|e| e.to_string())?;
    let archive: Archive =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid archive: {e}"))?;
    if archive.schema_version != 1
        || archive.archive_id != id
        || archive.environment.project_root != canonical_root(project)?
        || archive.snapshot_ref != format!("refs/agentz/archives/{id}")
    {
        return Err("archive identity/schema mismatch".into());
    }
    if string(git(
        project,
        None,
        &["rev-parse", "--verify", &archive.snapshot_ref],
        None,
    )?)? != archive.snapshot_commit
        || string(git(
            project,
            None,
            &[
                "rev-parse",
                &format!("{}^{{tree}}", archive.snapshot_commit),
            ],
            None,
        )?)? != archive.tree_sha
    {
        return Err("archive snapshot integrity mismatch".into());
    }
    Ok(archive)
}

/// Restore into a NEW checkout; never overwrite the original or an existing path.
pub fn restore(project: &Path, id: &str, destination: &Path) -> Result<Archive, String> {
    let archive = read(project, id)?;
    if !destination.is_absolute() || destination.exists() {
        return Err("restore destination already exists".into());
    }
    materialize(project, &archive.snapshot_commit, destination, None)?;
    Ok(archive)
}

pub(crate) fn materialize(
    project: &Path,
    commit: &str,
    destination: &Path,
    branch: Option<&str>,
) -> Result<(), String> {
    if !destination.is_absolute() || destination.exists() {
        return Err("destination must be new and absolute".into());
    }
    let tree = git(project, None, &["ls-tree", "-rz", commit], None)?;
    let mut files = Vec::new();
    for row in tree.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let row = std::str::from_utf8(row).map_err(|_| "unsupported tree path")?;
        let (meta, path) = row.split_once('\t').ok_or("invalid tree entry")?;
        let mut columns = meta.split_whitespace();
        let mode = columns.next().ok_or("missing tree mode")?;
        if mode != "100644" && mode != "100755" {
            return Err("unsupported restore mode".into());
        }
        if Path::new(path)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || path
                .split(['/', '\\'])
                .any(|s| s.eq_ignore_ascii_case(".git"))
        {
            return Err("unsafe archive path".into());
        }
        let oid = columns.nth(1).ok_or("missing tree object")?;
        files.push((
            path.to_string(),
            mode.to_string(),
            git(project, None, &["cat-file", "blob", oid], None)?,
        ));
    }
    let target = destination
        .to_str()
        .ok_or("unsupported non-UTF8 destination")?;
    let mut args = vec!["worktree", "add", "--no-checkout"];
    if let Some(branch) = branch {
        args.extend(["-b", branch]);
    } else {
        args.push("--detach");
    }
    args.extend([target, commit]);
    git(project, None, &args, None)?;
    git(destination, None, &["read-tree", commit], None)?;
    // Write raw blobs rather than invoking checkout/smudge or line-ending filters.
    for (path, mode, bytes) in &files {
        let absolute = destination.join(path);
        fs::create_dir_all(absolute.parent().ok_or("invalid restore path")?)
            .map_err(|e| e.to_string())?;
        fs::write(&absolute, bytes).map_err(|e| format!("restore retained; write failed: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                &absolute,
                fs::Permissions::from_mode(if mode == "100755" { 0o755 } else { 0o644 }),
            )
            .map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        let _ = mode;
        if fs::read(&absolute).map_err(|e| e.to_string())? != *bytes {
            return Err("restored bytes mismatch; checkout retained".into());
        }
    }
    if string(git(destination, None, &["rev-parse", "HEAD^{tree}"], None)?)?
        != string(git(
            project,
            None,
            &["rev-parse", &format!("{commit}^{{tree}}")],
            None,
        )?)?
    {
        return Err("restored tree mismatch; checkout retained for diagnosis".into());
    }
    Ok(())
}

/// A source snapshot cannot authorize loss of omitted ignored/runtime files.
pub fn require_no_omissions(archive: &Archive) -> Result<(), String> {
    if !archive.omitted_paths.is_empty() {
        return Err("archive omitted files; preserve them before cleanup".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register;
    #[test]
    fn restores_source_snapshot_without_touching_user_index_or_branch() {
        let root = std::env::temp_dir().join(format!("agentz-archive-{}", uuid::Uuid::new_v4()));
        let project = root.join("project");
        let workspace = root.join("task");
        fs::create_dir_all(&project).unwrap();
        git(&project, None, &["init", "-b", "main"], None).unwrap();
        fs::write(project.join(".gitignore"), "ignored.txt\n").unwrap();
        for path in ["dirty.txt", "staged.txt", "deleted.txt", "renamed.txt"] {
            fs::write(project.join(path), format!("base {path}\n")).unwrap();
        }
        git(&project, None, &["add", "."], None).unwrap();
        git(&project, None, &["commit", "-m", "base"], None).unwrap();
        let base = string(git(&project, None, &["rev-parse", "HEAD"], None).unwrap()).unwrap();
        git(
            &project,
            None,
            &["worktree", "add", "-b", "task", workspace.to_str().unwrap()],
            None,
        )
        .unwrap();
        let record = TaskEnvironment {
            schema_version: 1,
            task_id: "task".into(),
            environment_id: "env".into(),
            project_root: canonical_root(&project).unwrap(),
            workspace_root: canonical_root(&workspace).unwrap(),
            kind: "managed-worktree".into(),
            branch: Some("task".into()),
            base_ref: "main".into(),
            base_sha: base,
            created_at: "test".into(),
        };
        register(&project, &record).unwrap();
        fs::write(workspace.join("committed.txt"), "committed\n").unwrap();
        git(&workspace, None, &["add", "."], None).unwrap();
        git(&workspace, None, &["commit", "-m", "agent"], None).unwrap();
        fs::write(workspace.join("staged.txt"), "staged\n").unwrap();
        git(&workspace, None, &["add", "staged.txt"], None).unwrap();
        fs::write(workspace.join("dirty.txt"), "dirty\n").unwrap();
        fs::remove_file(workspace.join("deleted.txt")).unwrap();
        git(
            &workspace,
            None,
            &["mv", "renamed.txt", "改名 space.txt"],
            None,
        )
        .unwrap();
        fs::write(workspace.join("新增 space.bin"), [0, 1, 2, 255]).unwrap();
        fs::write(workspace.join("ignored.txt"), "must not lose\n").unwrap();
        fs::create_dir(workspace.join(".agentz")).unwrap();
        fs::write(workspace.join(".agentz/journal.db"), "runtime").unwrap();
        let before_index = git(&workspace, None, &["diff", "--cached", "--binary"], None).unwrap();
        let before_head = git(&workspace, None, &["rev-parse", "HEAD"], None).unwrap();
        let index_path = string(
            git(
                &workspace,
                None,
                &["rev-parse", "--git-path", "index"],
                None,
            )
            .unwrap(),
        )
        .unwrap();
        let index_path = workspace.join(index_path);
        let index_bytes = fs::read(&index_path).unwrap();
        let archive = create(&project, "task", "fixture", "2026-10-05").unwrap();
        assert_eq!(
            before_index,
            git(&workspace, None, &["diff", "--cached", "--binary"], None).unwrap()
        );
        assert_eq!(
            before_head,
            git(&workspace, None, &["rev-parse", "HEAD"], None).unwrap()
        );
        assert_eq!(index_bytes, fs::read(&index_path).unwrap());
        assert_eq!(archive.omitted_paths, [".agentz/journal.db", "ignored.txt"]);
        assert!(require_no_omissions(&archive).is_err());
        assert!(create(&project, "task", "fixture", "again").is_err());
        assert!(create(&project, "task", "../escape", "again").is_err());
        let archives = project.join(".agentz/archives");
        fs::write(archives.join("stale.index"), "retain diagnostic index").unwrap();
        assert!(create(&project, "task", "stale", "again").is_err());
        assert_eq!(
            fs::read_to_string(archives.join("stale.index")).unwrap(),
            "retain diagnostic index"
        );
        fs::create_dir(archives.join("metadata-failure.json.tmp")).unwrap();
        let failure = create(&project, "task", "metadata-failure", "again").unwrap_err();
        assert!(failure.contains("refs/agentz/archives/metadata-failure"));
        assert!(!archives.join("metadata-failure.json").exists());
        assert_eq!(index_bytes, fs::read(&index_path).unwrap());
        assert!(workspace.join("新增 space.bin").exists());
        assert!(git(
            &project,
            None,
            &["rev-parse", "refs/agentz/archives/metadata-failure"],
            None
        )
        .is_ok());
        let retained = root.join("retained-original");
        fs::rename(&workspace, &retained).unwrap();
        let restored = root.join("restored");
        restore(&project, "fixture", &restored).unwrap();
        for path in [
            "committed.txt",
            "staged.txt",
            "dirty.txt",
            "改名 space.txt",
            "新增 space.bin",
        ] {
            assert_eq!(
                fs::read(retained.join(path)).unwrap(),
                fs::read(restored.join(path)).unwrap()
            );
        }
        assert!(!restored.join("deleted.txt").exists());
        assert!(!restored.join("renamed.txt").exists());
        fs::rename(&retained, &workspace).unwrap();
        assert!(restore(&project, "fixture", &workspace).is_err());
        assert_eq!(
            fs::read_to_string(workspace.join("ignored.txt")).unwrap(),
            "must not lose\n"
        );
        git(
            &project,
            None,
            &["update-ref", &archive.snapshot_ref, &archive.head_sha],
            None,
        )
        .unwrap();
        assert!(read(&project, "fixture").is_err());
        assert!(restore(&project, "fixture", &root.join("invalid-restore")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
