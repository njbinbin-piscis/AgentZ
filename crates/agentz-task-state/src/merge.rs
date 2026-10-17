//! Non-mutating merge preflight; transaction execution is integrated separately.
use crate::{
    archive::{check_record, git, string},
    canonical_root, load,
};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct MergeError {
    pub stage: &'static str,
    pub message: String,
    pub transaction_id: Option<String>,
    pub recovery_ref: Option<String>,
    pub state: Option<String>,
}
impl MergeError {
    pub fn new(stage: &'static str, message: impl Into<String>) -> Self {
        error(stage, message)
    }
    fn recovery(mut self, receipt: &Receipt) -> Self {
        self.transaction_id = Some(receipt.transaction_id.clone());
        self.recovery_ref = Some(receipt.recovery_ref.clone());
        self.state = Some(receipt.state.clone());
        self
    }
}
fn error(stage: &'static str, message: impl Into<String>) -> MergeError {
    MergeError {
        stage,
        message: message.into(),
        transaction_id: None,
        recovery_ref: None,
        state: None,
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    pub task_id: String,
    pub task_head: String,
    pub target_branch: String,
    pub target_head: String,
    pub merged_tree: String,
    pub repository: String,
}
fn output(root: &Path, args: &[&str], stage: &'static str) -> Result<String, MergeError> {
    string(git(root, None, args, None).map_err(|e| error(stage, e))?).map_err(|e| error(stage, e))
}
fn clean(root: &Path, stage: &'static str) -> Result<(), MergeError> {
    for operation in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        let file = output(root, &["rev-parse", "--git-path", operation], stage)?;
        if root.join(file).exists() {
            return Err(error(stage, format!("existing Git operation: {operation}")));
        }
    }
    for args in [
        vec!["diff", "--name-only", "-z"],
        vec!["diff", "--cached", "--name-only", "-z"],
    ] {
        if !git(root, None, &args, None)
            .map_err(|e| error(stage, e))?
            .is_empty()
        {
            return Err(error(
                stage,
                "tracked staged/working changes must be preserved before merging",
            ));
        }
    }
    let untracked = git(
        root,
        None,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        None,
    )
    .map_err(|e| error(stage, e))?;
    for bytes in untracked.split(|b| *b == 0).filter(|b| !b.is_empty()) {
        let path = std::str::from_utf8(bytes)
            .map_err(|_| error(stage, "unsupported non-UTF8 source path"))?;
        if ![".agentz", ".codez", ".piscis"].contains(&path.split('/').next().unwrap_or("")) {
            return Err(error(
                stage,
                format!("untracked source must be preserved before merging: {path}"),
            ));
        }
    }
    Ok(())
}

/// expected_task_head is the commit captured with the user's reviewed change set.
/// A commit SHA is never accepted as the destination branch. No checkout is performed.
pub fn preview(
    project: &Path,
    task_id: &str,
    target: &str,
    expected_task_head: &str,
) -> Result<Preview, MergeError> {
    let record = load(project, task_id)
        .map_err(|e| error("identity", e))?
        .ok_or_else(|| error("identity", "registered task identity is required"))?;
    check_record(project, &record).map_err(|e| error("identity", e))?;
    if target.is_empty()
        || target == "HEAD"
        || target.starts_with('-')
        || target.starts_with("workz/task-")
    {
        return Err(error(
            "target",
            "explicit local destination branch is required",
        ));
    }
    let current = output(
        project,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        "target",
    )?;
    if current != target {
        return Err(error(
            "target",
            "switch to the intended target branch explicitly before merging",
        ));
    }
    let target_head = output(
        project,
        &["rev-parse", "--verify", &format!("refs/heads/{target}")],
        "target",
    )?;
    let root = Path::new(&record.workspace_root);
    let task_head = output(root, &["rev-parse", "--verify", "HEAD^{commit}"], "source")?;
    if expected_task_head.is_empty() || task_head != expected_task_head {
        return Err(error(
            "review_changed",
            "task HEAD changed; refresh and review before merging",
        ));
    }
    clean(project, "target_dirty")?;
    clean(root, "source_dirty")?;
    // merge-tree writes only Git objects, never the user's working directory/index/ref.
    // Unsupported Git versions return an explicit error, never an empty successful diff.
    let tree = output(
        project,
        &["merge-tree", "--write-tree", &target_head, &task_head],
        "conflict_preflight",
    )?;
    let merged_tree = tree.lines().next().unwrap_or("").to_owned();
    if ![40, 64].contains(&merged_tree.len()) || !merged_tree.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(error("conflict_preflight", "invalid merge-tree result"));
    }
    if output(
        project,
        &["cat-file", "-t", &merged_tree],
        "conflict_preflight",
    )? != "tree"
    {
        return Err(error(
            "conflict_preflight",
            "preflight object is not a tree",
        ));
    }
    // Reject a drift even during preflight. The executor must repeat these checks under its lock.
    check_record(project, &record).map_err(|e| error("identity", e))?;
    if output(project, &["rev-parse", "HEAD"], "target")? != target_head
        || output(root, &["rev-parse", "HEAD"], "source")? != task_head
        || output(
            project,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
            "target",
        )? != target
    {
        return Err(error("head_changed", "repository changed during preflight"));
    }
    clean(project, "target_dirty")?;
    clean(root, "source_dirty")?;
    let common = output(
        project,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        "identity",
    )?;
    Ok(Preview {
        task_id: task_id.into(),
        task_head,
        target_branch: target.into(),
        target_head,
        merged_tree,
        repository: canonical_root(Path::new(&common)).map_err(|e| error("identity", e))?,
    })
}

#[derive(Debug, Serialize)]
pub struct Receipt {
    pub schema_version: u32,
    pub transaction_id: String,
    pub task_id: String,
    pub task_head: String,
    pub target_branch: String,
    pub before_head: String,
    pub after_head: Option<String>,
    pub recovery_ref: String,
    pub task_archive_id: String,
    pub state: String,
}
fn persist(project: &Path, receipt: &Receipt) -> Result<(), MergeError> {
    use std::{fs, io::Write};
    let directory = project.join(".agentz/merges");
    fs::create_dir_all(&directory).map_err(|e| error("receipt", e.to_string()))?;
    let temporary = directory.join(format!("{}.{}.tmp", receipt.transaction_id, receipt.state));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|e| error("receipt", e.to_string()))?;
    file.write_all(
        &serde_json::to_vec_pretty(receipt).map_err(|e| error("receipt", e.to_string()))?,
    )
    .map_err(|e| error("receipt", e.to_string()))?;
    file.sync_all()
        .map_err(|e| error("receipt", e.to_string()))?;
    drop(file);
    fs::rename(
        temporary,
        directory.join(format!("{}.json", receipt.transaction_id)),
    )
    .map_err(|e| error("receipt", e.to_string()))
}
async fn user_git(project: &Path, args: &[&str]) -> Result<String, String> {
    // Preserve the user's identity/configuration. Metadata snapshots use a separate identity.
    let mut command = tokio::process::Command::new("git");
    command
        .arg("-C")
        .arg(project)
        .args(args)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    let output = tokio::time::timeout(std::time::Duration::from_secs(60), command.output())
        .await
        .map_err(|_| "Git operation timed out; inspect the retained transaction state".to_string())?
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        )
        .trim()
        .into());
    }
    string(output.stdout)
}

fn prepare(
    project: &Path,
    requested: &Preview,
    id: &str,
    at: &str,
) -> Result<(Preview, Receipt), MergeError> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(error("identity", "invalid transaction identity"));
    }
    let current = preview(
        project,
        &requested.task_id,
        &requested.target_branch,
        &requested.task_head,
    )?;
    if current.target_head != requested.target_head || current.repository != requested.repository {
        return Err(error(
            "head_changed",
            "target HEAD changed since confirmation; refresh before merging",
        ));
    }
    let recovery_ref = format!("refs/agentz/merges/{id}");
    output(
        project,
        &[
            "update-ref",
            &recovery_ref,
            &current.target_head,
            &"0".repeat(current.target_head.len()),
        ],
        "backup",
    )?;
    let mut receipt = Receipt {
        schema_version: 1,
        transaction_id: id.into(),
        task_id: current.task_id.clone(),
        task_head: current.task_head.clone(),
        target_branch: current.target_branch.clone(),
        before_head: current.target_head.clone(),
        after_head: None,
        recovery_ref,
        task_archive_id: format!("merge-{id}"),
        state: "planned".into(),
    };
    crate::archive::create(project, &current.task_id, &receipt.task_archive_id, at)
        .map_err(|e| error("backup", e).recovery(&receipt))?;
    persist(project, &receipt).map_err(|e| e.recovery(&receipt))?;
    // Capturing recovery data may take time: repeat all identity/dirty/version checks.
    let final_preview = preview(
        project,
        &current.task_id,
        &current.target_branch,
        &current.task_head,
    )
    .map_err(|e| e.recovery(&receipt))?;
    if final_preview.target_head != current.target_head
        || final_preview.merged_tree != current.merged_tree
    {
        receipt.state = "not_started".into();
        persist(project, &receipt).map_err(|e| e.recovery(&receipt))?;
        return Err(error(
            "head_changed",
            "target changed before merge; recovery data retained",
        )
        .recovery(&receipt));
    }
    Ok((current, receipt))
}
fn safe_to_abort(project: &Path, preview: &Preview) -> bool {
    output(project, &["rev-parse", "HEAD"], "recovery")
        .ok()
        .as_deref()
        == Some(&preview.target_head)
        && output(
            project,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
            "recovery",
        )
        .ok()
        .as_deref()
            == Some(&preview.target_branch)
        && output(
            project,
            &["rev-parse", "--verify", "MERGE_HEAD"],
            "recovery",
        )
        .ok()
        .as_deref()
            == Some(&preview.task_head)
        && git(
            project,
            None,
            &[
                "diff",
                "--cached",
                "--name-only",
                "-z",
                &preview.merged_tree,
            ],
            None,
        )
        .is_ok_and(|bytes| bytes.is_empty())
        && git(project, None, &["diff", "--name-only", "-z"], None)
            .is_ok_and(|bytes| bytes.is_empty())
}

/// Caller owns repository queue + exclusive execution leases for this entire worker.
/// Dropping the IPC waiter must not cancel this worker or release its leases early.
pub async fn execute(
    project: &Path,
    requested: &Preview,
    id: &str,
    at: &str,
) -> Result<Receipt, MergeError> {
    let source = project.to_path_buf();
    let requested = requested.clone();
    let id = id.to_owned();
    let at = at.to_owned();
    let (current, mut receipt) =
        tokio::task::spawn_blocking(move || prepare(&source, &requested, &id, &at))
            .await
            .map_err(|e| error("worker", e.to_string()))??;
    let message = format!("Merge AgentZ task {}", current.task_id);
    let merged = user_git(
        project,
        &[
            "merge",
            "--no-ff",
            "--no-edit",
            &current.task_head,
            "-m",
            &message,
        ],
    )
    .await;
    if let Err(message) = merged {
        receipt.state = "needs_recovery".into();
        let source = project.to_path_buf();
        let check = current.clone();
        let timed_out = message.starts_with("Git operation timed out");
        let safe =
            tokio::task::spawn_blocking(move || !timed_out && safe_to_abort(&source, &check))
                .await
                .map_err(|e| error("recovery", e.to_string()).recovery(&receipt))?;
        let recovery = if safe {
            match user_git(project, &["merge", "--abort"]).await {
                Ok(_) => {
                    receipt.state = "aborted".into();
                    "merge aborted".into()
                }
                Err(e) => format!("abort also failed: {e}"),
            }
        } else {
            "state not proven to belong solely to this transaction; preserved for recovery".into()
        };
        receipt.after_head = output(project, &["rev-parse", "HEAD"], "recovery").ok();
        if receipt.state == "aborted"
            && (receipt.after_head.as_deref() != Some(&receipt.before_head)
                || clean(project, "recovery").is_err())
        {
            receipt.state = "needs_recovery".into();
        }
        persist(project, &receipt).map_err(|e| e.recovery(&receipt))?;
        return Err(error("merge", format!("{message}; {recovery}")).recovery(&receipt));
    }
    receipt.state = "needs_recovery".into();
    receipt.after_head = match output(project, &["rev-parse", "HEAD"], "verify") {
        Ok(head) => Some(head),
        Err(e) => {
            persist(project, &receipt).map_err(|e| e.recovery(&receipt))?;
            return Err(e.recovery(&receipt));
        }
    };
    let head = receipt.after_head.as_deref().unwrap_or("");
    let verified = (|| -> Result<bool, MergeError> {
        Ok(
            output(project, &["rev-parse", "HEAD^{tree}"], "verify")? == current.merged_tree
                && output(
                    project,
                    &["symbolic-ref", "--quiet", "--short", "HEAD"],
                    "verify",
                )? == current.target_branch
                && (head == receipt.before_head
                    || output(project, &["show", "-s", "--format=%P", "HEAD"], "verify")?
                        == format!("{} {}", receipt.before_head, current.task_head))
                && clean(project, "verify").is_ok(),
        )
    })();
    let verified = match verified {
        Ok(value) => value,
        Err(e) => {
            persist(project, &receipt).map_err(|e| e.recovery(&receipt))?;
            return Err(e.recovery(&receipt));
        }
    };
    if !verified {
        persist(project, &receipt).map_err(|e| e.recovery(&receipt))?;
        return Err(error(
            "verify",
            "merge result or working directory changed; retained for recovery",
        )
        .recovery(&receipt));
    }
    receipt.state = if head == receipt.before_head {
        "unchanged"
    } else {
        "merged"
    }
    .into();
    persist(project, &receipt).map_err(|e| e.recovery(&receipt))?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{register, TaskEnvironment};
    use std::fs;
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn run(root: &Path, args: &[&str]) -> String {
        string(git(root, None, args, None).unwrap()).unwrap()
    }
    fn setup(count: usize) -> (Fixture, std::path::PathBuf, Vec<Preview>) {
        let fixture = Fixture(
            std::env::temp_dir().join(format!("agentz-merge-execute-{}", uuid::Uuid::new_v4())),
        );
        let project = fixture.0.join("project");
        fs::create_dir_all(&project).unwrap();
        run(&project, &["init", "-b", "main"]);
        run(&project, &["config", "user.name", "Merge Fixture"]);
        run(&project, &["config", "user.email", "merge@example.invalid"]);
        run(&project, &["config", "core.autocrlf", "false"]);
        fs::write(project.join("file.txt"), b"base\n").unwrap();
        fs::write(project.join(".gitignore"), b".agentz/\n").unwrap();
        run(&project, &["add", "."]);
        run(&project, &["commit", "-m", "base"]);
        let base = run(&project, &["rev-parse", "HEAD"]);
        let mut previews = Vec::new();
        for id in ["a", "b"].into_iter().take(count) {
            let task = fixture.0.join(format!("task-{id}"));
            let branch = format!("workz/task-{id}");
            run(
                &project,
                &[
                    "worktree",
                    "add",
                    "-b",
                    &branch,
                    task.to_str().unwrap(),
                    &base,
                ],
            );
            register(
                &project,
                &TaskEnvironment {
                    schema_version: 1,
                    task_id: id.into(),
                    environment_id: uuid::Uuid::new_v4().to_string(),
                    project_root: canonical_root(&project).unwrap(),
                    workspace_root: canonical_root(&task).unwrap(),
                    kind: "managed-worktree".into(),
                    branch: Some(branch),
                    base_ref: "main".into(),
                    base_sha: base.clone(),
                    created_at: "test".into(),
                },
            )
            .unwrap();
            fs::write(task.join(format!("{id}.txt")), format!("task {id}\n")).unwrap();
            run(&task, &["add", "."]);
            run(&task, &["commit", "-m", "task"]);
            previews
                .push(preview(&project, id, "main", &run(&task, &["rev-parse", "HEAD"])).unwrap());
        }
        (fixture, project, previews)
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn actual_repository_queue_merges_once_then_rejects_stale_target_and_accepts_refreshed_retry(
    ) {
        use crate::access::{EnvironmentAccess, RepositoryAccess};
        use std::sync::Arc;
        let (_fixture, project, previews) = setup(2);
        let queues = Arc::new(RepositoryAccess::default());
        let gate = Arc::new(EnvironmentAccess::default());
        let first_queue = queues.transaction(&previews[0].repository).await.unwrap();
        let first_execution = gate.cleanup().unwrap();
        let source = project.clone();
        let first = previews[0].clone();
        let a = tokio::spawn(async move {
            let _queue = first_queue;
            let _execution = first_execution;
            execute(&source, &first, "serial-a", "test").await
        });
        let source = project.clone();
        let second = previews[1].clone();
        let second_queue = queues.clone();
        let second_gate = gate.clone();
        let b = tokio::spawn(async move {
            let _queue = second_queue.transaction(&second.repository).await.unwrap();
            let _execution = second_gate.cleanup().unwrap();
            execute(&source, &second, "serial-b-stale", "test").await
        });
        let (a, b) = tokio::join!(a, b);
        let receipt = a.unwrap().unwrap();
        assert_eq!(receipt.state, "merged");
        assert_eq!(b.unwrap().unwrap_err().stage, "head_changed");
        assert_eq!(
            run(&project, &["rev-parse", &receipt.recovery_ref]),
            previews[0].target_head
        );
        assert_eq!(
            run(&project, &["show", "-s", "--format=%an", "HEAD"]),
            "Merge Fixture"
        );
        assert!(project.join("a.txt").exists());
        assert!(!project.join("b.txt").exists());
        let refreshed = preview(&project, "b", "main", &previews[1].task_head).unwrap();
        let _queue = queues.transaction(&refreshed.repository).await.unwrap();
        let _execution = gate.cleanup().unwrap();
        let second = execute(&project, &refreshed, "serial-b-refresh", "test")
            .await
            .unwrap();
        assert_eq!(second.state, "merged");
        assert_eq!(fs::read(project.join("a.txt")).unwrap(), b"task a\n");
        assert_eq!(fs::read(project.join("b.txt")).unwrap(), b"task b\n");
        assert_eq!(
            run(&project, &["show", "-s", "--format=%P", "HEAD"]),
            format!("{} {}", refreshed.target_head, refreshed.task_head)
        );
    }
    fn hook(project: &Path, body: &str) {
        let path = project.join(".git/hooks/pre-merge-commit");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn canceled_waiter_does_not_cancel_merge_worker_or_release_its_guards() {
        use crate::access::{EnvironmentAccess, RepositoryAccess};
        use std::sync::Arc;
        let (_fixture, project, previews) = setup(1);
        let queues = RepositoryAccess::default();
        let gate = Arc::new(EnvironmentAccess::default());
        let repository_guard = queues.transaction(&previews[0].repository).await.unwrap();
        let execution_guard = gate.cleanup().unwrap();
        let preview = previews[0].clone();
        let source = project.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(async move {
            let _ = started_tx.send(());
            let result = execute(&source, &preview, "canceled-waiter", "test").await;
            drop(execution_guard);
            drop(repository_guard);
            let _ = done_tx.send(result);
        });
        let waiter = tokio::spawn(worker);
        started_rx.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert!(
            gate.cleanup().is_err(),
            "the independent worker must retain its exclusive lease"
        );
        let receipt = tokio::time::timeout(std::time::Duration::from_secs(120), done_rx)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(receipt.state, "merged");
        assert!(gate.cleanup().is_ok());
        assert_eq!(fs::read(project.join("a.txt")).unwrap(), b"task a\n");
        let saved: serde_json::Value = serde_json::from_slice(
            &fs::read(project.join(".agentz/merges/canceled-waiter.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["state"], "merged");
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hook_failure_aborts_only_unchanged_owned_state_and_reports_abort_failure() {
        for (id, body, expected) in [
            ("abort", "exit 1", "aborted"),
            (
                "preserve",
                "printf 'hook edit\\n' > file.txt\nexit 1",
                "needs_recovery",
            ),
            ("locked", ": > .git/index.lock\nexit 1", "needs_recovery"),
        ] {
            let (fixture, project, previews) = setup(1);
            hook(&project, body);
            let failed = execute(&project, &previews[0], id, "test")
                .await
                .unwrap_err();
            assert_eq!(
                failed.state.as_deref(),
                Some(expected),
                "{}",
                failed.message
            );
            assert_eq!(
                run(&project, &["rev-parse", "HEAD"]),
                previews[0].target_head
            );
            assert_eq!(
                run(
                    &project,
                    &["rev-parse", failed.recovery_ref.as_deref().unwrap()]
                ),
                previews[0].target_head
            );
            assert_eq!(
                fs::read(fixture.0.join("task-a/a.txt")).unwrap(),
                b"task a\n"
            );
            if id == "abort" {
                assert!(!project.join("a.txt").exists());
                assert!(!project.join(".git/MERGE_HEAD").exists());
            }
            if id == "preserve" {
                assert_eq!(fs::read(project.join("file.txt")).unwrap(), b"hook edit\n");
                assert!(project.join(".git/MERGE_HEAD").exists());
            }
            if id == "locked" {
                assert!(
                    failed.message.contains("abort also failed"),
                    "{}",
                    failed.message
                );
                assert!(project.join(".git/index.lock").exists());
            }
            let saved: serde_json::Value = serde_json::from_slice(
                &fs::read(project.join(format!(".agentz/merges/{id}.json"))).unwrap(),
            )
            .unwrap();
            assert_eq!(saved["state"], expected);
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn publication_failure_after_merge_reports_actual_completed_commit_and_keeps_recovery_ref(
    ) {
        let (_fixture, project, previews) = setup(1);
        fs::create_dir_all(project.join(".agentz/merges/publish.merged.tmp")).unwrap();
        let failed = execute(&project, &previews[0], "publish", "test")
            .await
            .unwrap_err();
        assert_eq!(failed.stage, "receipt");
        assert_eq!(failed.state.as_deref(), Some("merged"));
        assert_ne!(
            run(&project, &["rev-parse", "HEAD"]),
            previews[0].target_head
        );
        assert_eq!(
            run(
                &project,
                &["rev-parse", failed.recovery_ref.as_deref().unwrap()]
            ),
            previews[0].target_head
        );
        assert_eq!(fs::read(project.join("a.txt")).unwrap(), b"task a\n");
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(project.join(".agentz/merges/publish.json")).unwrap())
                .unwrap();
        assert_eq!(saved["state"], "planned");
    }
    #[test]
    fn preflight_rejects_dirty_wrong_target_stale_review_and_conflict_without_mutating_source() {
        let fixture = Fixture(
            std::env::temp_dir().join(format!("agentz-merge-test-{}", uuid::Uuid::new_v4())),
        );
        let project = fixture.0.join("project");
        let task = fixture.0.join("task");
        fs::create_dir_all(&project).unwrap();
        run(&project, &["init", "-b", "main"]);
        run(&project, &["config", "core.autocrlf", "false"]);
        fs::write(project.join("file.txt"), b"base\n").unwrap();
        run(&project, &["add", "."]);
        run(&project, &["commit", "-m", "base"]);
        let base = run(&project, &["rev-parse", "HEAD"]);
        run(
            &project,
            &[
                "worktree",
                "add",
                "-b",
                "workz/task-a",
                task.to_str().unwrap(),
                &base,
            ],
        );
        register(
            &project,
            &TaskEnvironment {
                schema_version: 1,
                task_id: "a".into(),
                environment_id: uuid::Uuid::new_v4().to_string(),
                project_root: canonical_root(&project).unwrap(),
                workspace_root: canonical_root(&task).unwrap(),
                kind: "managed-worktree".into(),
                branch: Some("workz/task-a".into()),
                base_ref: "main".into(),
                base_sha: base.clone(),
                created_at: "test".into(),
            },
        )
        .unwrap();
        fs::write(task.join("file.txt"), b"task\n").unwrap();
        run(&task, &["add", "."]);
        run(&task, &["commit", "-m", "task"]);
        let head = run(&task, &["rev-parse", "HEAD"]);
        let index = fs::read(project.join(".git/index")).unwrap();
        let valid = preview(&project, "a", "main", &head).unwrap();
        assert_eq!(valid.target_head, base);
        assert_eq!(fs::read(project.join(".git/index")).unwrap(), index);
        assert_eq!(fs::read(project.join("file.txt")).unwrap(), b"base\n");
        fs::write(project.join("loose.txt"), b"keep this").unwrap();
        assert_eq!(
            preview(&project, "a", "main", &head).unwrap_err().stage,
            "target_dirty"
        );
        assert_eq!(fs::read(project.join("loose.txt")).unwrap(), b"keep this");
        fs::remove_file(project.join("loose.txt")).unwrap();
        fs::write(project.join(".git/MERGE_HEAD"), &head).unwrap();
        assert_eq!(
            preview(&project, "a", "main", &head).unwrap_err().stage,
            "target_dirty"
        );
        fs::remove_file(project.join(".git/MERGE_HEAD")).unwrap();
        assert_eq!(
            preview(&project, "a", "main", &base).unwrap_err().stage,
            "review_changed"
        );
        assert_eq!(
            preview(&project, "a", &base, &head).unwrap_err().stage,
            "target"
        );
        assert_eq!(
            preview(&project, "b", "main", &head).unwrap_err().stage,
            "identity"
        );
        fs::write(project.join("file.txt"), b"dirty\n").unwrap();
        assert_eq!(
            preview(&project, "a", "main", &head).unwrap_err().stage,
            "target_dirty"
        );
        run(&project, &["add", "file.txt"]);
        assert_eq!(
            preview(&project, "a", "main", &head).unwrap_err().stage,
            "target_dirty"
        );
        run(&project, &["commit", "-m", "target edit"]);
        let target_head = run(&project, &["rev-parse", "HEAD"]);
        let index = fs::read(project.join(".git/index")).unwrap();
        assert_eq!(
            preview(&project, "a", "main", &head).unwrap_err().stage,
            "conflict_preflight"
        );
        assert_eq!(run(&project, &["rev-parse", "HEAD"]), target_head);
        assert_eq!(fs::read(project.join(".git/index")).unwrap(), index);
        assert_eq!(fs::read(project.join("file.txt")).unwrap(), b"dirty\n");
        assert!(!project.join(".git/MERGE_HEAD").exists());
        fs::write(task.join("new.txt"), b"uncommitted").unwrap();
        assert_eq!(
            preview(&project, "a", "main", &head).unwrap_err().stage,
            "source_dirty"
        );
        assert_eq!(fs::read(task.join("new.txt")).unwrap(), b"uncommitted");
    }
}
