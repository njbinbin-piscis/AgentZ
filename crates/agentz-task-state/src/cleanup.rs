//! Relocate, never recursively delete. The complete checkout is the retention vault.
use crate::{
    archive::{self, check_record, git, string},
    canonical_root, load,
};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

#[derive(Debug, Serialize)]
pub struct CleanupError {
    pub stage: &'static str,
    pub message: String,
    pub archive_id: Option<String>,
    pub retained_root: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub schema_version: u32,
    pub archive_id: String,
    pub task_id: String,
    pub source_root: String,
    pub retained_root: String,
    pub snapshot_commit: String,
    pub branch: String,
    pub branch_action: String,
    pub state: String,
}
fn error(
    stage: &'static str,
    message: impl Into<String>,
    id: Option<&str>,
    root: Option<&str>,
) -> CleanupError {
    CleanupError {
        stage,
        message: message.into(),
        archive_id: id.map(str::to_owned),
        retained_root: root.map(str::to_owned),
    }
}
fn persist(project: &Path, receipt: &Receipt) -> Result<(), String> {
    let dir = project.join(".agentz/archives");
    let file = dir.join(format!("{}.cleanup.json", receipt.archive_id));
    let tmp = dir.join(format!(
        "{}.cleanup.{}.tmp",
        receipt.archive_id, receipt.state
    ));
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    out.write_all(&serde_json::to_vec_pretty(receipt).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    out.sync_all().map_err(|e| e.to_string())?;
    drop(out);
    fs::rename(tmp, file).map_err(|e| e.to_string())
}

/// Caller holds an exclusive EnvironmentAccess lease for the complete operation.
pub fn retain_task(
    project: &Path,
    task_id: &str,
    requested_root: &str,
    requested_branch: &str,
    id: &str,
    at: &str,
) -> Result<Receipt, CleanupError> {
    if task_id.is_empty()
        || !task_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(error("identity", "invalid task identity", None, None));
    }
    let record = load(project, task_id)
        .map_err(|e| error("identity", e, None, None))?
        .ok_or_else(|| {
            error(
                "identity",
                "task binding is required; legacy paths cannot authorize cleanup",
                None,
                None,
            )
        })?;
    let root =
        canonical_root(Path::new(requested_root)).map_err(|e| error("identity", e, None, None))?;
    let project_root = canonical_root(project).map_err(|e| error("identity", e, None, None))?;
    let managed = Path::new(&project_root)
        .parent()
        .ok_or_else(|| error("identity", "invalid project root", None, None))?
        .join(".agentz-worktrees");
    let expected = managed.join(format!("task-{task_id}"));
    if root != record.workspace_root
        || requested_branch != record.branch.as_deref().unwrap_or("")
        || requested_branch != format!("workz/task-{task_id}")
        || root == project_root
        || canonical_root(&expected).ok().as_deref() != Some(root.as_str())
    {
        return Err(error(
            "identity",
            "requested root/branch is not the registered managed task",
            None,
            None,
        ));
    }
    check_record(project, &record).map_err(|e| error("identity", e, None, None))?;
    let parent = project.join(".agentz/retained-worktrees");
    fs::create_dir_all(&parent).map_err(|e| error("prepare", e.to_string(), None, None))?;
    let parent =
        fs::canonicalize(parent).map_err(|e| error("prepare", e.to_string(), None, None))?;
    if !parent.starts_with(Path::new(&project_root)) {
        return Err(error(
            "prepare",
            "retention directory escapes project",
            None,
            None,
        ));
    }
    // create validates the archive id before it can be interpolated into a path.
    let snapshot =
        archive::create(project, task_id, id, at).map_err(|e| error("snapshot", e, None, None))?;
    let destination = parent.join(id);
    if destination.exists() {
        return Err(error(
            "prepare",
            "retained destination already exists",
            Some(id),
            None,
        ));
    }
    check_record(project, &record).map_err(|e| error("pre_move", e, Some(id), None))?;
    if string(
        git(Path::new(&root), None, &["rev-parse", "HEAD"], None)
            .map_err(|e| error("pre_move", e, Some(id), None))?,
    )
    .map_err(|e| error("pre_move", e, Some(id), None))?
        != snapshot.head_sha
    {
        return Err(error(
            "pre_move",
            "HEAD changed after snapshot; source retained",
            Some(id),
            None,
        ));
    }
    let target = destination.to_string_lossy().to_string();
    // Git for Windows path arguments also need an ordinary DOS/UNC representation.
    let git_target = if let Some(unc) = target.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        target.strip_prefix(r"\\?\").unwrap_or(&target).to_string()
    };
    let git_source = if let Some(unc) = root.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        root.strip_prefix(r"\\?\").unwrap_or(&root).to_string()
    };
    let mut receipt = Receipt {
        schema_version: 1,
        archive_id: id.into(),
        task_id: task_id.into(),
        source_root: root,
        retained_root: target,
        snapshot_commit: snapshot.snapshot_commit,
        branch: requested_branch.into(),
        branch_action: "retained".into(),
        state: "planned".into(),
    };
    persist(project, &receipt).map_err(|e| error("receipt_before_move", e, Some(id), None))?;
    if let Err(e) = git(
        project,
        None,
        &["worktree", "move", &git_source, &git_target],
        None,
    ) {
        let retained = destination
            .exists()
            .then_some(receipt.retained_root.as_str());
        return Err(error("worktree_move", e, Some(id), retained));
    }
    receipt.retained_root = canonical_root(&destination)
        .map_err(|e| error("verify_move", e, Some(id), Some(&receipt.retained_root)))?;
    if Path::new(&receipt.source_root).exists() {
        return Err(error(
            "verify_move",
            "source still exists; retained checkout kept",
            Some(id),
            Some(&receipt.retained_root),
        ));
    }
    receipt.state = "archived".into();
    persist(project, &receipt).map_err(|e| {
        error(
            "receipt_after_move",
            e,
            Some(id),
            Some(&receipt.retained_root),
        )
    })?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{register, TaskEnvironment};
    #[test]
    fn rejects_wrong_objects_and_preserves_ignored_runtime_index_and_branch() {
        let fixture = std::env::temp_dir().join(format!("agentz-retain-{}", uuid::Uuid::new_v4()));
        let project = fixture.join("project");
        let source = fixture.join(".agentz-worktrees/task-a");
        fs::create_dir_all(&project).unwrap();
        git(&project, None, &["init", "-b", "main"], None).unwrap();
        fs::write(project.join("file.txt"), "base\n").unwrap();
        fs::write(project.join(".gitignore"), "ignored.txt\n").unwrap();
        git(&project, None, &["add", "."], None).unwrap();
        git(&project, None, &["commit", "-m", "base"], None).unwrap();
        let head = string(git(&project, None, &["rev-parse", "HEAD"], None).unwrap()).unwrap();
        git(
            &project,
            None,
            &[
                "worktree",
                "add",
                "-b",
                "workz/task-a",
                source.to_str().unwrap(),
            ],
            None,
        )
        .unwrap();
        let record = TaskEnvironment {
            schema_version: 1,
            task_id: "a".into(),
            environment_id: "env-a".into(),
            project_root: canonical_root(&project).unwrap(),
            workspace_root: canonical_root(&source).unwrap(),
            kind: "managed-worktree".into(),
            branch: Some("workz/task-a".into()),
            base_ref: "main".into(),
            base_sha: head.clone(),
            created_at: "test".into(),
        };
        register(&project, &record).unwrap();
        fs::write(source.join("file.txt"), "staged\n").unwrap();
        git(&source, None, &["add", "file.txt"], None).unwrap();
        fs::write(source.join("file.txt"), "dirty\n").unwrap();
        fs::write(source.join("new.bin"), [0, 7, 255]).unwrap();
        fs::write(source.join("ignored.txt"), "ignored but important").unwrap();
        fs::create_dir(source.join(".agentz")).unwrap();
        fs::write(source.join(".agentz/journal.db"), "runtime bytes").unwrap();
        let index =
            string(git(&source, None, &["rev-parse", "--git-path", "index"], None).unwrap())
                .unwrap();
        let index_bytes = fs::read(source.join(&index)).unwrap();
        assert_eq!(
            retain_task(
                &project,
                "a",
                project.to_str().unwrap(),
                "workz/task-a",
                "wrong-root",
                "test"
            )
            .unwrap_err()
            .stage,
            "identity"
        );
        assert_eq!(
            retain_task(
                &project,
                "a",
                source.to_str().unwrap(),
                "main",
                "wrong-branch",
                "test"
            )
            .unwrap_err()
            .stage,
            "identity"
        );
        assert!(retain_task(
            &project,
            "b",
            source.to_str().unwrap(),
            "workz/task-a",
            "other-task",
            "test"
        )
        .is_err());
        // Git refuses a locked checkout. A snapshot is retained; no success is fabricated.
        git(
            &project,
            None,
            &["worktree", "lock", source.to_str().unwrap()],
            None,
        )
        .unwrap();
        let failed = retain_task(
            &project,
            "a",
            source.to_str().unwrap(),
            "workz/task-a",
            "locked",
            "test",
        )
        .unwrap_err();
        assert_eq!(failed.stage, "worktree_move");
        assert_eq!(failed.archive_id.as_deref(), Some("locked"));
        assert!(source.exists());
        assert!(archive::read(&project, "locked").is_ok());
        assert_eq!(index_bytes, fs::read(source.join(&index)).unwrap());
        git(
            &project,
            None,
            &["worktree", "unlock", source.to_str().unwrap()],
            None,
        )
        .unwrap();
        let receipt = retain_task(
            &project,
            "a",
            source.to_str().unwrap(),
            "workz/task-a",
            "success",
            "test",
        )
        .unwrap();
        let retained = Path::new(&receipt.retained_root);
        assert!(!source.exists());
        assert_eq!(receipt.state, "archived");
        assert_eq!(receipt.branch_action, "retained");
        for (path, bytes) in [
            ("file.txt", b"dirty\n".as_slice()),
            ("ignored.txt", b"ignored but important".as_slice()),
            (".agentz/journal.db", b"runtime bytes".as_slice()),
            ("new.bin", [0, 7, 255].as_slice()),
        ] {
            assert_eq!(fs::read(retained.join(path)).unwrap(), bytes);
        }
        assert_eq!(index_bytes, fs::read(retained.join(index)).unwrap());
        assert_eq!(
            string(git(retained, None, &["rev-parse", "HEAD"], None).unwrap()).unwrap(),
            head
        );
        assert_eq!(
            string(git(&project, None, &["rev-parse", "workz/task-a"], None).unwrap()).unwrap(),
            head
        );
        let persisted: Receipt = serde_json::from_slice(
            &fs::read(project.join(".agentz/archives/success.cleanup.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(persisted.state, "archived");
        // Restore the source location in this private fixture, then inject failure
        // only in final receipt publication. The move must remain recoverable.
        git(
            &project,
            None,
            &[
                "worktree",
                "move",
                project
                    .join(".agentz/retained-worktrees/success")
                    .to_str()
                    .unwrap(),
                source.to_str().unwrap(),
            ],
            None,
        )
        .unwrap();
        fs::write(
            project.join(".agentz/archives/partial.cleanup.archived.tmp"),
            b"reserved",
        )
        .unwrap();
        let partial = retain_task(
            &project,
            "a",
            source.to_str().unwrap(),
            "workz/task-a",
            "partial",
            "test",
        )
        .unwrap_err();
        assert_eq!(partial.stage, "receipt_after_move");
        assert_eq!(partial.archive_id.as_deref(), Some("partial"));
        let retained = Path::new(partial.retained_root.as_deref().unwrap());
        assert_eq!(
            fs::read(retained.join("ignored.txt")).unwrap(),
            b"ignored but important"
        );
        assert_eq!(
            fs::read(retained.join(".agentz/journal.db")).unwrap(),
            b"runtime bytes"
        );
        assert!(archive::read(&project, "partial").is_ok());
        let planned: Receipt = serde_json::from_slice(
            &fs::read(project.join(".agentz/archives/partial.cleanup.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(planned.state, "planned");
        assert!(!source.exists());
        fs::remove_dir_all(fixture).unwrap();
    }
}
