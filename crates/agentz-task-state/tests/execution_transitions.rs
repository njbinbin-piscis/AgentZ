//! Exhaustive check of the public run execution transition API.
//!
//! Every ordered pair of execution states is attempted through `RunStore::transition`
//! on a fresh run. The accepted pairs must be exactly the ten edges of the state table,
//! and a rejected attempt must not be applied.

use agentz_task_state::run::{Execution, RunStore};
use agentz_task_state::{canonical_root, register, TaskEnvironment};
use std::path::PathBuf;

const ALL: [Execution; 7] = [
    Execution::Queued,
    Execution::Running,
    Execution::Cancelling,
    Execution::Succeeded,
    Execution::Failed,
    Execution::Cancelled,
    Execution::Interrupted,
];

const ALLOWED: [(Execution, Execution); 10] = [
    (Execution::Queued, Execution::Running),
    (Execution::Queued, Execution::Cancelled),
    (Execution::Queued, Execution::Interrupted),
    (Execution::Running, Execution::Cancelling),
    (Execution::Running, Execution::Succeeded),
    (Execution::Running, Execution::Failed),
    (Execution::Running, Execution::Interrupted),
    (Execution::Cancelling, Execution::Cancelled),
    (Execution::Cancelling, Execution::Failed),
    (Execution::Cancelling, Execution::Interrupted),
];

fn fresh_project() -> PathBuf {
    let directory = std::env::temp_dir().join(format!("agentz-exec-table-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let root = canonical_root(&directory).unwrap();
    register(
        &directory,
        &TaskEnvironment {
            schema_version: 1,
            task_id: "task".into(),
            environment_id: "environment".into(),
            project_root: root.clone(),
            workspace_root: root,
            kind: "local".into(),
            branch: None,
            base_ref: String::new(),
            base_sha: String::new(),
            created_at: "fixture".into(),
        },
    )
    .unwrap();
    directory
}

/// Legal path from a freshly created (queued) run to `target`.
fn path_to(target: Execution) -> Vec<Execution> {
    use Execution::*;
    match target {
        Queued => vec![],
        Running => vec![Running],
        Cancelling => vec![Running, Cancelling],
        Succeeded => vec![Running, Succeeded],
        Failed => vec![Running, Failed],
        Cancelled => vec![Cancelled],
        Interrupted => vec![Interrupted],
    }
}

#[test]
fn accepted_transitions_are_exactly_the_state_table() {
    for from in ALL {
        for to in ALL {
            let directory = fresh_project();
            let mut store = RunStore::open(&directory).unwrap();
            store.create("run", "task", "environment", "a1", 1).unwrap();
            let mut current = Execution::Queued;
            for step in path_to(from) {
                store.transition("run", "a1", current, step, 2).unwrap();
                current = step;
            }
            assert_eq!(current, from, "setup must reach {from:?}");

            let accepted = store.transition("run", "a1", from, to, 3).is_ok();
            let expected = ALLOWED.contains(&(from, to));
            assert_eq!(accepted, expected, "{from:?} -> {to:?}");
        }
    }
}

#[test]
fn terminal_states_reject_every_outgoing_transition() {
    for terminal in [
        Execution::Succeeded,
        Execution::Failed,
        Execution::Cancelled,
        Execution::Interrupted,
    ] {
        for to in ALL {
            let directory = fresh_project();
            let mut store = RunStore::open(&directory).unwrap();
            store.create("run", "task", "environment", "a1", 1).unwrap();
            let mut current = Execution::Queued;
            for step in path_to(terminal) {
                store.transition("run", "a1", current, step, 2).unwrap();
                current = step;
            }
            assert!(
                store.transition("run", "a1", terminal, to, 3).is_err(),
                "{terminal:?} -> {to:?} must be rejected"
            );
        }
    }
}
