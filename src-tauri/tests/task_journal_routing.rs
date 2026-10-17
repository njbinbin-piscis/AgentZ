//! Real Git + SQLite + pinned kernel hooks, with no model or desktop window.
use agentz_desktop_lib::commands::{
    agent_task::{agent_task_change_set, resolve_task_workspace},
    journal::open_task_journal,
};
use agentz_desktop_lib::journal::open_project_journal;
use agentz_task_state::{canonical_root, register, TaskEnvironment};
use piscis_kernel::agent::{
    hooks::{AgentHooks, ToolHookEvent},
    tool::ToolResult,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[tokio::test]
async fn retained_checkout_keeps_real_journal_readable_and_old_session_invalid() {
    let fixture = Fixture(
        std::env::temp_dir().join(format!("agentz-retained-journal-{}", uuid::Uuid::new_v4())),
    );
    let project = fixture.0.join("project");
    let workspace = fixture.0.join(".agentz-worktrees/task-a");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(workspace.parent().unwrap()).unwrap();
    git(&project, &["init", "-b", "main"]);
    git(
        &project,
        &["config", "user.email", "fixture@example.invalid"],
    );
    git(&project, &["config", "user.name", "Fixture"]);
    git(&project, &["config", "core.autocrlf", "false"]);
    fs::write(project.join("shared.txt"), "base\n").unwrap();
    git(&project, &["add", "."]);
    git(&project, &["commit", "-m", "base"]);
    let base = git(&project, &["rev-parse", "HEAD"]);
    git(
        &project,
        &[
            "worktree",
            "add",
            "-b",
            "workz/task-a",
            workspace.to_str().unwrap(),
            &base,
        ],
    );
    let project_root = canonical_root(&project).unwrap();
    let workspace_root = canonical_root(&workspace).unwrap();
    register(
        &project,
        &TaskEnvironment {
            schema_version: 1,
            task_id: "a".into(),
            environment_id: uuid::Uuid::new_v4().to_string(),
            project_root: project_root.clone(),
            workspace_root: workspace_root.clone(),
            kind: "managed-worktree".into(),
            branch: Some("workz/task-a".into()),
            base_ref: "main".into(),
            base_sha: base,
            created_at: "2026-10-05".into(),
        },
    )
    .unwrap();
    let turn = edit(&workspace, "a", "shared.txt", "edited\n", "retained-edit").await;
    let receipt = agentz_task_state::cleanup::retain_task(
        &project,
        "a",
        &workspace_root,
        "workz/task-a",
        &uuid::Uuid::new_v4().to_string(),
        "2026-10-05",
    )
    .unwrap();
    assert!(open_task_journal(Some(&project_root), "a").await.is_err());
    let retained = PathBuf::from(&receipt.retained_root);
    let journal = open_project_journal(&receipt.retained_root).unwrap();
    let diffs = journal.get_turn_file_diffs("a", &turn).unwrap();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].before.as_deref(), Some("base\n"));
    assert_eq!(diffs[0].after, "edited\n");
    // Archive browsing retains history; reopening an execution session requires explicit recovery.
    assert_eq!(
        fs::read_to_string(retained.join("shared.txt")).unwrap(),
        "edited\n"
    );
    assert_eq!(
        fs::read_to_string(project.join("shared.txt")).unwrap(),
        "base\n"
    );
    assert!(!workspace.exists());
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}
async fn edit(root: &Path, session: &str, path: &str, content: &str, call: &str) -> String {
    let journal = open_project_journal(root.to_str().unwrap()).unwrap();
    let turn = journal.begin_turn(session);
    let input = serde_json::json!({"path": path});
    let event = ToolHookEvent {
        session_id: session,
        tool_use_id: call,
        tool_name: "file_write",
        input: &input,
        workspace_root: root,
    };
    journal.before_tool(&event).await;
    fs::write(root.join(path), content).unwrap();
    journal.after_tool(&event, &ToolResult::ok("written")).await;
    turn
}

#[tokio::test]
async fn reopened_task_journal_restores_only_its_environment() {
    let fixture =
        Fixture(std::env::temp_dir().join(format!("agentz-journal-{}", uuid::Uuid::new_v4())));
    let project = fixture.0.join("project");
    fs::create_dir_all(&project).unwrap();
    git(&project, &["init", "-b", "main"]);
    git(
        &project,
        &["config", "user.email", "fixture@example.invalid"],
    );
    git(&project, &["config", "user.name", "Fixture"]);
    git(&project, &["config", "core.autocrlf", "false"]);
    fs::write(project.join("shared.txt"), "base\n").unwrap();
    git(&project, &["add", "shared.txt"]);
    git(&project, &["commit", "-m", "base"]);
    let base = git(&project, &["rev-parse", "HEAD"]);
    let project_root = canonical_root(&project).unwrap();
    let mut roots = Vec::new();
    for session in ["task-a", "task-b"] {
        let workspace = fixture.0.join(session);
        git(
            &project,
            &[
                "worktree",
                "add",
                "-b",
                session,
                workspace.to_str().unwrap(),
                &base,
            ],
        );
        let root = canonical_root(&workspace).unwrap();
        register(
            &project,
            &TaskEnvironment {
                schema_version: 1,
                task_id: session.into(),
                environment_id: uuid::Uuid::new_v4().to_string(),
                project_root: project_root.clone(),
                workspace_root: root.clone(),
                kind: "managed-worktree".into(),
                branch: Some(session.into()),
                base_ref: "main".into(),
                base_sha: base.clone(),
                created_at: "2026-10-05".into(),
            },
        )
        .unwrap();
        roots.push(PathBuf::from(root));
    }
    let main_turn = edit(
        &project,
        "local",
        "shared.txt",
        "main edited\n",
        "main-write",
    )
    .await;
    let a_turn = edit(&roots[0], "task-a", "shared.txt", "a edited\n", "a-write").await;
    let b_turn = edit(&roots[1], "task-b", "shared.txt", "b edited\n", "b-write").await;
    let created_turn = edit(&roots[0], "task-a", "created.txt", "new\n", "a-create").await;
    let set = agent_task_change_set(Some(project_root.clone()), "task-a".into())
        .await
        .unwrap();
    assert_eq!(set.base_sha, base);
    assert_eq!(
        set.changes
            .iter()
            .map(|c| c.path.as_str())
            .collect::<Vec<_>>(),
        ["created.txt", "shared.txt"]
    );
    // Reopen through the same resolver used by IPC; there is no cached journal/root.
    let a = open_task_journal(Some(&project_root), "task-a")
        .await
        .unwrap();
    let diffs = a.get_turn_file_diffs("task-a", &a_turn).unwrap();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].before.as_deref(), Some("base\n"));
    assert_eq!(diffs[0].after, "a edited\n");
    assert!(a.list_changes("task-b", &b_turn).unwrap().is_empty());
    a.undo_turn("task-a", &a_turn).unwrap();
    a.undo_turn("task-a", &created_turn).unwrap();
    assert_eq!(
        fs::read_to_string(roots[0].join("shared.txt")).unwrap(),
        "base\n"
    );
    assert!(!roots[0].join("created.txt").exists());
    assert_eq!(
        fs::read_to_string(project.join("shared.txt")).unwrap(),
        "main edited\n"
    );
    assert_eq!(
        fs::read_to_string(roots[1].join("shared.txt")).unwrap(),
        "b edited\n"
    );
    assert!(a.list_changes("task-a", &a_turn).unwrap().is_empty());
    assert!(a.undo_turn("task-a", &a_turn).unwrap().is_empty());
    let local = open_task_journal(Some(&project_root), "local")
        .await
        .unwrap();
    assert_eq!(local.list_changes("local", &main_turn).unwrap().len(), 1);
    local.undo_turn("local", &main_turn).unwrap();
    assert_eq!(
        fs::read_to_string(project.join("shared.txt")).unwrap(),
        "base\n"
    );
    assert_eq!(
        fs::read_to_string(roots[1].join("shared.txt")).unwrap(),
        "b edited\n"
    );
    assert!(
        resolve_task_workspace(&project, Some("task-a"), Some(&project_root), true)
            .await
            .is_err()
    );
    // A stale branch is rejected before any journal can be opened in a fallback root.
    git(&roots[1], &["switch", "-c", "wrong-branch"]);
    assert!(open_task_journal(Some(&project_root), "task-b")
        .await
        .is_err());
    let error = agent_task_change_set(Some(project_root.clone()), "task-b".into())
        .await
        .unwrap_err();
    assert_eq!(error.code, "environment_invalid");
    assert_eq!(
        fs::read_to_string(roots[1].join("shared.txt")).unwrap(),
        "b edited\n"
    );
}
