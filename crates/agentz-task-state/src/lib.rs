//! Durable task/environment identity, independent of the conversation database.
pub mod access;
pub mod approval;
pub mod approval_broker;
pub mod archive;
pub mod artifacts;
pub mod automation;
pub mod budget;
pub mod budget_reservation;
pub mod recovery;
pub mod task_details;
pub mod changes;
pub mod cleanup;
pub mod events;
pub mod external_action;
pub mod inbox;
pub mod merge;
pub mod plan;
pub mod run;
pub mod start;

/// `git` child processes never open a console window on Windows. A window created by the GUI
/// host takes focus, which re-fires focus-driven refreshes and starts more git processes.
pub(crate) mod git_process {
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Blocking `git` command with the console window suppressed on Windows.
    pub(crate) fn command() -> std::process::Command {
        #[allow(unused_mut)]
        let mut cmd = std::process::Command::new("git");
        cmd.args(UNQUOTED_PATHS);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd
    }

    /// Line-based output (`--name-status`, `--numstat`) octal-escapes
    /// non-ASCII paths unless quoting is off; `-z` output is unaffected.
    const UNQUOTED_PATHS: [&str; 2] = ["-c", "core.quotepath=false"];

    /// Async `git` command with the console window suppressed on Windows.
    pub(crate) fn tokio_command() -> tokio::process::Command {
        #[allow(unused_mut)]
        let mut cmd = tokio::process::Command::new("git");
        cmd.args(UNQUOTED_PATHS);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
}
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEnvironment {
    pub schema_version: u32,
    pub task_id: String,
    pub environment_id: String,
    pub project_root: String,
    pub workspace_root: String,
    pub kind: String,
    pub branch: Option<String>,
    pub base_ref: String,
    pub base_sha: String,
    pub created_at: String,
}

pub fn canonical_root(root: &Path) -> Result<String, String> {
    if !root.is_dir() {
        return Err(format!(
            "environment directory not found: {}",
            root.display()
        ));
    }
    std::fs::canonicalize(root)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("cannot resolve environment directory: {e}"))
}

fn database_path(project: &Path) -> PathBuf {
    project.join(".agentz").join("task-environments.db")
}

/// Facts collected from the filesystem/Git by the host, never from the renderer.
pub struct ManagedSnapshot {
    pub workspace_root: String,
    pub project_git_dir: String,
    pub workspace_git_dir: String,
    pub branch: String,
    pub registered: bool,
}

pub fn validate_managed(
    record: &TaskEnvironment,
    snapshot: &ManagedSnapshot,
) -> Result<(), String> {
    if record.kind != "managed-worktree"
        || record.workspace_root != snapshot.workspace_root
        || snapshot.project_git_dir != snapshot.workspace_git_dir
        || record.branch.as_deref() != Some(snapshot.branch.as_str())
        || !snapshot.registered
    {
        return Err(
            "task environment no longer matches its registered worktree/repository/branch".into(),
        );
    }
    Ok(())
}

pub fn validate_requested_workspace(
    record: &TaskEnvironment,
    requested_root: Option<&str>,
) -> Result<(), String> {
    if requested_root.is_some_and(|root| root != record.workspace_root) {
        return Err(
            "requested workspace does not match the task environment; explicit handoff required"
                .into(),
        );
    }
    Ok(())
}

/// `git worktree list --porcelain -z`: parse NUL fields so paths with newlines survive.
pub fn registered_worktree(output: &str, root: &str, branch: &str) -> bool {
    output.split("\0\0").any(|block| {
        let mut found_root = false;
        let mut found_branch = false;
        for field in block.split('\0') {
            if let Some(directory) = field.strip_prefix("worktree ") {
                found_root =
                    canonical_root(Path::new(directory)).is_ok_and(|actual| actual == root);
            }
            if let Some(actual) = field.strip_prefix("branch refs/heads/") {
                found_branch = actual == branch;
            }
        }
        found_root && found_branch
    })
}

fn open(project: &Path, create: bool) -> Result<Option<Connection>, String> {
    let file = database_path(project);
    if !create && !file.exists() {
        return Ok(None);
    }
    if create {
        std::fs::create_dir_all(file.parent().ok_or("invalid registry path")?)
            .map_err(|e| format!("cannot create environment registry: {e}"))?;
    }
    let conn =
        Connection::open(&file).map_err(|e| format!("cannot open environment registry: {e}"))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version > SCHEMA_VERSION {
        return Err(format!(
            "environment registry schema {version} is newer than supported {SCHEMA_VERSION}"
        ));
    }
    if version == 0 {
        conn.execute_batch(
            "BEGIN IMMEDIATE;
         CREATE TABLE IF NOT EXISTS task_environments (
           task_id TEXT PRIMARY KEY NOT NULL,
           environment_id TEXT UNIQUE NOT NULL,
           record TEXT NOT NULL
         );
         PRAGMA user_version = 1;
         COMMIT;",
        )
        .map_err(|e| format!("cannot initialize environment registry: {e}"))?;
    }
    Ok(Some(conn))
}

fn parse_record(project: &Path, raw: &str) -> Result<TaskEnvironment, String> {
    let record: TaskEnvironment =
        serde_json::from_str(raw).map_err(|e| format!("invalid task environment: {e}"))?;
    if record.schema_version != SCHEMA_VERSION || record.project_root != canonical_root(project)? {
        return Err("task environment schema/project identity mismatch".into());
    }
    Ok(record)
}

pub fn load(project: &Path, task_id: &str) -> Result<Option<TaskEnvironment>, String> {
    load_existing_readonly(project, task_id)
}

fn open_existing_readonly(project: &Path) -> Result<Option<Connection>, String> {
    let file = database_path(project);
    if !file
        .try_exists()
        .map_err(|e| format!("cannot inspect environment registry: {e}"))?
    {
        return Ok(None);
    }
    let conn = Connection::open_with_flags(&file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("cannot open read-only environment registry: {e}"))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version != SCHEMA_VERSION {
        return Err(format!("environment registry schema {version} requires explicit recovery; expected {SCHEMA_VERSION}"));
    }
    let columns = conn
        .prepare("PRAGMA table_info(task_environments)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (name, pk) in [("task_id", 1), ("environment_id", 0), ("record", 0)] {
        if !columns.iter().any(|(column, kind, not_null, primary)| {
            column == name && kind.eq_ignore_ascii_case("TEXT") && *not_null == 1 && *primary == pk
        }) {
            return Err("environment registry columns are invalid".into());
        }
    }
    Ok(Some(conn))
}

/// Lookup never creates or repairs a registry, including existing version-zero files.
pub fn load_existing_readonly(
    project: &Path,
    task_id: &str,
) -> Result<Option<TaskEnvironment>, String> {
    let Some(conn) = open_existing_readonly(project)? else {
        return Ok(None);
    };
    let raw: Option<(String, String, String)> = conn
        .query_row(
            "SELECT task_id,environment_id,record FROM task_environments WHERE task_id = ?1",
            [task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    raw.map(|(task, environment, text)| {
        let record = parse_record(project, &text)?;
        if task != task_id
            || record.task_id != task
            || record.environment_id != environment
            || task.trim().is_empty()
            || environment.trim().is_empty()
        {
            return Err("task environment record does not match its task/environment keys".into());
        }
        Ok(record)
    })
    .transpose()
}

/// A binding is immutable until a future explicit handoff transaction replaces it.
/// Concurrent/idempotent registration cannot silently retarget an existing task.
pub fn register(project: &Path, record: &TaskEnvironment) -> Result<TaskEnvironment, String> {
    if record.task_id.trim().is_empty() || record.environment_id.trim().is_empty() {
        return Err("task and environment identity must not be empty".into());
    }
    if record.schema_version != SCHEMA_VERSION || record.project_root != canonical_root(project)? {
        return Err("task environment schema/project identity mismatch".into());
    }
    let mut conn = open(project, true)?.ok_or("environment registry unavailable")?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let raw: Option<String> = tx
        .query_row(
            "SELECT record FROM task_environments WHERE task_id = ?1",
            [&record.task_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(raw) = raw {
        let existing = parse_record(project, &raw)?;
        if existing.task_id != record.task_id
            || existing.environment_id != record.environment_id
            || existing.workspace_root != record.workspace_root
            || existing.kind != record.kind
            || existing.branch != record.branch
            || existing.base_sha != record.base_sha
        {
            return Err(
                "task is already bound to a different environment; explicit handoff required"
                    .into(),
            );
        }
        return Ok(existing);
    }
    let raw = serde_json::to_string(record).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO task_environments (task_id, environment_id, record) VALUES (?1, ?2, ?3)",
        params![record.task_id, record.environment_id, raw],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(record.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readonly_missing_registry_does_not_create_and_valid_registration_reopens_without_writes() {
        let (project, record) = fixture();
        assert!(load(&project, &record.task_id).unwrap().is_none());
        assert!(!project.join(".agentz").exists());
        register(&project, &record).unwrap();
        let before = std::fs::read(database_path(&project)).unwrap();
        assert_eq!(
            load(&project, &record.task_id).unwrap(),
            Some(record.clone())
        );
        assert_eq!(
            load_existing_readonly(&project, &record.task_id).unwrap(),
            Some(record)
        );
        assert!(load(&project, "missing-task").unwrap().is_none());
        assert_eq!(std::fs::read(database_path(&project)).unwrap(), before);
        assert!(project.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(project).unwrap();
    }
    #[test]
    fn readonly_version_zero_and_future_registry_are_not_initialized_or_repaired() {
        for version in [0, 2] {
            let (project, record) = fixture();
            std::fs::create_dir_all(project.join(".agentz")).unwrap();
            let conn = Connection::open(database_path(&project)).unwrap();
            conn.pragma_update(None, "user_version", version).unwrap();
            drop(conn);
            let before = std::fs::read(database_path(&project)).unwrap();
            assert!(load(&project, &record.task_id).is_err());
            assert_eq!(std::fs::read(database_path(&project)).unwrap(), before);
            assert!(project.starts_with(std::env::temp_dir()));
            std::fs::remove_dir_all(project).unwrap();
        }
    }
    #[test]
    fn readonly_corrupt_environment_column_record_and_schema_fail_closed_unchanged() {
        for corrupt in [
            "UPDATE task_environments SET environment_id='wrong-column'",
            "UPDATE task_environments SET record='invalid-json'",
            "DROP TABLE task_environments",
        ] {
            let (project, record) = fixture();
            register(&project, &record).unwrap();
            let conn = Connection::open(database_path(&project)).unwrap();
            conn.execute_batch(corrupt).unwrap();
            drop(conn);
            let before = std::fs::read(database_path(&project)).unwrap();
            assert!(load(&project, &record.task_id).is_err(), "{corrupt}");
            assert_eq!(std::fs::read(database_path(&project)).unwrap(), before);
            assert!(project.starts_with(std::env::temp_dir()));
            std::fs::remove_dir_all(project).unwrap();
        }
    }

    fn fixture() -> (PathBuf, TaskEnvironment) {
        let project =
            std::env::temp_dir().join(format!("agentz_environment_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project).unwrap();
        let root = canonical_root(&project).unwrap();
        let record = TaskEnvironment {
            schema_version: 1,
            task_id: "task-a".into(),
            environment_id: "env-a".into(),
            project_root: root.clone(),
            workspace_root: root,
            kind: "local".into(),
            branch: None,
            base_ref: String::new(),
            base_sha: String::new(),
            created_at: "fixture".into(),
        };
        (project, record)
    }

    #[test]
    fn survives_reopen_and_keeps_tasks_separate() {
        let (project, a) = fixture();
        assert!(load(&project, "task-a").unwrap().is_none());
        register(&project, &a).unwrap();
        let mut b = a.clone();
        b.task_id = "task-b".into();
        b.environment_id = "env-b".into();
        register(&project, &b).unwrap();
        assert_eq!(load(&project, "task-a").unwrap(), Some(a));
        assert_eq!(load(&project, "task-b").unwrap(), Some(b));
    }

    #[test]
    fn idempotent_registration_does_not_retarget_or_change_base() {
        let (project, a) = fixture();
        register(&project, &a).unwrap();
        assert_eq!(register(&project, &a).unwrap(), a);
        let mut altered = a.clone();
        altered.base_sha = "other".into();
        assert!(register(&project, &altered).is_err());
        assert_eq!(load(&project, "task-a").unwrap(), Some(a));
    }

    #[test]
    fn rejects_cross_project_records_and_future_schema() {
        let (project, a) = fixture();
        let (other, _) = fixture();
        assert!(register(&other, &a).is_err());
        register(&project, &a).unwrap();
        let conn = Connection::open(database_path(&project)).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        drop(conn);
        assert!(load(&project, "task-a").is_err());
    }

    #[test]
    fn rejects_environment_reuse_by_another_task_and_corrupt_record() {
        let (project, a) = fixture();
        register(&project, &a).unwrap();
        let mut b = a.clone();
        b.task_id = "task-b".into();
        assert!(register(&project, &b).is_err());
        assert!(load(&project, "task-b").unwrap().is_none());
        let conn = Connection::open(database_path(&project)).unwrap();
        conn.execute(
            "UPDATE task_environments SET record = ?1 WHERE task_id = 'task-a'",
            [serde_json::to_string(&b).unwrap()],
        )
        .unwrap();
        assert!(load(&project, "task-a").is_err());
        conn.execute(
            "UPDATE task_environments SET record = 'not-json' WHERE task_id = 'task-a'",
            [],
        )
        .unwrap();
        drop(conn);
        assert!(load(&project, "task-a").is_err());
    }

    #[test]
    fn execution_rejects_retargeted_unregistered_and_wrong_repository_snapshots() {
        let (_, mut record) = fixture();
        record.kind = "managed-worktree".into();
        record.branch = Some("workz/task-a".into());
        let mut snapshot = ManagedSnapshot {
            workspace_root: record.workspace_root.clone(),
            project_git_dir: "repo-a".into(),
            workspace_git_dir: "repo-a".into(),
            branch: "workz/task-a".into(),
            registered: true,
        };
        assert!(validate_managed(&record, &snapshot).is_ok());
        assert!(validate_requested_workspace(&record, None).is_ok());
        assert!(validate_requested_workspace(&record, Some("other-task-directory")).is_err());
        snapshot.workspace_git_dir = "repo-b".into();
        assert!(validate_managed(&record, &snapshot).is_err());
        snapshot.workspace_git_dir = "repo-a".into();
        snapshot.branch = "workz/task-b".into();
        assert!(validate_managed(&record, &snapshot).is_err());
        snapshot.branch = "workz/task-a".into();
        snapshot.registered = false;
        assert!(validate_managed(&record, &snapshot).is_err());
    }

    #[test]
    fn worktree_registration_parser_preserves_spaces_and_requires_matching_branch() {
        let (project, _) = fixture();
        let nested = project.join("with spaces");
        std::fs::create_dir_all(&nested).unwrap();
        let root = canonical_root(&nested).unwrap();
        let output = format!(
            "worktree {}\0HEAD abc\0branch refs/heads/workz/task-a\0\0",
            nested.display()
        );
        assert!(registered_worktree(&output, &root, "workz/task-a"));
        assert!(!registered_worktree(&output, &root, "workz/task-b"));
    }

    #[test]
    fn validates_real_git_worktree_then_rejects_changed_branch_and_missing_directory() {
        fn git(dir: &Path, args: &[&str]) -> String {
            let output = std::process::Command::new("git")
                .args([
                    "-c",
                    "user.name=AgentZFixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                ])
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().to_string()
        }
        fn snapshot(project: &Path, workspace: &Path) -> ManagedSnapshot {
            let workspace_root = canonical_root(workspace).unwrap();
            let project_git = git(
                project,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            );
            let workspace_git = git(
                workspace,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            );
            let branch = git(workspace, &["rev-parse", "--abbrev-ref", "HEAD"]);
            let listing = git(project, &["worktree", "list", "--porcelain", "-z"]);
            ManagedSnapshot {
                registered: registered_worktree(&listing, &workspace_root, &branch),
                workspace_root,
                project_git_dir: canonical_root(Path::new(&project_git)).unwrap(),
                workspace_git_dir: canonical_root(Path::new(&workspace_git)).unwrap(),
                branch,
            }
        }
        let (project, mut record) = fixture();
        git(&project, &["init", "-q", "-b", "main"]);
        git(&project, &["commit", "--allow-empty", "-qm", "base"]);
        let workspace = project.join("task with spaces");
        git(
            &project,
            &[
                "worktree",
                "add",
                "-b",
                "workz/task-a",
                workspace.to_str().unwrap(),
                "main",
            ],
        );
        record.kind = "managed-worktree".into();
        record.branch = Some("workz/task-a".into());
        record.workspace_root = canonical_root(&workspace).unwrap();
        record.base_sha = git(&workspace, &["rev-parse", "HEAD"]);
        register(&project, &record).unwrap();
        assert!(validate_managed(&record, &snapshot(&project, &workspace)).is_ok());
        assert!(
            validate_requested_workspace(&record, Some(&canonical_root(&project).unwrap()))
                .is_err()
        );
        git(&workspace, &["switch", "-qc", "other"]);
        assert!(validate_managed(&record, &snapshot(&project, &workspace)).is_err());
        std::fs::rename(&workspace, project.join("retained moved task")).unwrap();
        assert!(canonical_root(&workspace).is_err());
        assert_eq!(
            load(&project, "task-a").unwrap().unwrap().workspace_root,
            record.workspace_root
        );
    }
}
pub mod projectless;
