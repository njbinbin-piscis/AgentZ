//! Isolated kernel lifecycle fixture. This is not a native desktop E2E run.
use agentz_task_state::{
    archive,
    artifacts::{ArtifactStore, Capture, CaptureInput, Kind},
    canonical_root, changes,
    plan::{ItemState, PlanBinding, PlanItem, PlanStore},
    register,
    run::{Execution, RunStore},
    TaskEnvironment,
};
use std::{path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "AgentZ Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@agentz.invalid")
        .env("GIT_COMMITTER_NAME", "AgentZ Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@agentz.invalid")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

#[test]
fn two_tasks_continue_after_store_reopen_and_archive_restores_source() {
    let root = std::env::temp_dir().join(format!("agentz-lifecycle-{}", uuid::Uuid::new_v4()));
    eprintln!("fixture_root={}", root.display());
    let project = root.join("project");
    std::fs::create_dir_all(&project).unwrap();
    git(&project, &["init", "-b", "main"]);
    std::fs::write(project.join("README.md"), "base\n").unwrap();
    git(&project, &["add", "README.md"]);
    git(&project, &["commit", "-m", "fixture"]);
    let base = git(&project, &["rev-parse", "HEAD"]);
    let project_id = canonical_root(&project).unwrap();
    let bindings: Vec<_> = ["a", "b"]
        .iter()
        .map(|name| {
            let worktree = root.join(format!("task-{name}"));
            git(
                &project,
                &[
                    "worktree",
                    "add",
                    "-b",
                    &format!("task-{name}"),
                    worktree.to_str().unwrap(),
                ],
            );
            let task_id = format!("task-{name}");
            let environment_id = format!("env-{name}");
            register(
                &project,
                &TaskEnvironment {
                    schema_version: 1,
                    task_id: task_id.clone(),
                    environment_id: environment_id.clone(),
                    project_root: project_id.clone(),
                    workspace_root: canonical_root(&worktree).unwrap(),
                    kind: "managed-worktree".into(),
                    branch: Some(task_id.clone()),
                    base_ref: "main".into(),
                    base_sha: base.clone(),
                    created_at: "fixture".into(),
                },
            )
            .unwrap();
            (
                worktree,
                PlanBinding {
                    project_id: project_id.clone(),
                    task_id,
                    environment_id,
                    binding_version: name.repeat(64),
                },
            )
        })
        .collect();
    {
        let mut plans = PlanStore::open(&project).unwrap();
        let mut runs = RunStore::open(&project).unwrap();
        let mut artifacts = ArtifactStore::initialize(&project).unwrap();
        for (worktree, binding) in &bindings {
            plans
                .create(
                    binding,
                    &format!("plan-{}", binding.task_id),
                    "fixture objective",
                    vec![PlanItem {
                        id: "step".into(),
                        description: "edit and archive".into(),
                        state: ItemState::InProgress,
                    }],
                    1,
                )
                .unwrap();
            let run_id = format!("run-{}", binding.task_id);
            runs.create(
                &run_id,
                &binding.task_id,
                &binding.environment_id,
                &format!("attempt-{}", binding.task_id),
                1,
            )
            .unwrap();
            runs.transition(
                &run_id,
                &format!("attempt-{}", binding.task_id),
                Execution::Queued,
                Execution::Running,
                2,
            )
            .unwrap();
            runs.transition(
                &run_id,
                &format!("attempt-{}", binding.task_id),
                Execution::Running,
                Execution::Succeeded,
                3,
            )
            .unwrap();
            let payload = format!("{} source\n", binding.task_id);
            std::fs::write(worktree.join("result.md"), &payload).unwrap();
            let captured = artifacts
                .capture_next(
                    binding,
                    CaptureInput {
                        artifact_id: "result".into(),
                        version: 0,
                        kind: Kind::Document,
                        title: "Result".into(),
                        source_relative_path: Some("result.md".into()),
                        media_type: "text/markdown".into(),
                        created_ms: 3,
                    },
                    payload.as_bytes(),
                )
                .unwrap();
            assert!(matches!(captured, Capture::Created(m) if m.version == 1));
        }
    }
    // A process restart is represented by closing and reopening every durable store.
    let plans = PlanStore::open_existing_readonly(&project).unwrap();
    let runs = RunStore::open_existing_readonly(&project).unwrap();
    let mut artifacts = ArtifactStore::open_existing(&project, true)
        .unwrap()
        .unwrap();
    for (_, binding) in &bindings {
        assert_eq!(
            plans.continue_plan(binding).unwrap().unwrap().items[0].state,
            ItemState::InProgress
        );
        assert_eq!(
            runs.load(&format!("run-{}", binding.task_id))
                .unwrap()
                .unwrap()
                .execution,
            Execution::Succeeded
        );
        let saved = artifacts
            .read_version(binding, "result", 1)
            .unwrap()
            .unwrap();
        assert_eq!(
            saved.payload,
            format!("{} source\n", binding.task_id).as_bytes()
        );
    }
    for (worktree, binding) in &bindings {
        let review = changes::collect(worktree, &base).unwrap();
        assert_eq!(review.changes.len(), 1);
        assert_eq!(review.changes[0].path, "result.md");
        assert_eq!(review.changes[0].status, "?");
        let file = changes::file_view(worktree, &base, "result.md").unwrap();
        assert_eq!(file.original, None);
        assert_eq!(
            file.modified.as_deref(),
            Some(format!("{} source\n", binding.task_id).as_str())
        );
    }
    assert!(artifacts
        .read_version(&bindings[0].1, "missing", 1)
        .unwrap()
        .is_none());
    assert!(artifacts
        .read_version(&bindings[1].1, "result", 2)
        .unwrap()
        .is_none());
    let snapshot =
        archive::create(&project, &bindings[0].1.task_id, "fixture-a", "fixture").unwrap();
    assert!(snapshot.omitted_paths.is_empty());
    let restored = root.join("restored-a");
    archive::restore(&project, "fixture-a", &restored).unwrap();
    assert_eq!(
        std::fs::read_to_string(restored.join("result.md")).unwrap(),
        "task-a source\n"
    );
    assert_eq!(
        std::fs::read_to_string(bindings[1].0.join("result.md")).unwrap(),
        "task-b source\n"
    );
    drop(artifacts);
    drop(runs);
    drop(plans);
    for (worktree, _) in &bindings {
        git(
            &project,
            &["worktree", "remove", "--force", worktree.to_str().unwrap()],
        );
    }
    git(
        &project,
        &["worktree", "remove", "--force", restored.to_str().unwrap()],
    );
    std::fs::remove_dir_all(root).unwrap();
}
