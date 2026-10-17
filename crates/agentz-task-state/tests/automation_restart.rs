use agentz_task_state::automation::{MissedWindow, Trigger, TriggerStore, UnresolvedState};
use std::{path::Path, process::Command};

const CHILD_PROJECT: &str = "AGENTZ_AUTOMATION_TEST_PROJECT";
const CHILD_MODE: &str = "AGENTZ_AUTOMATION_TEST_MODE";

#[test]
fn automation_child_claim() {
    let Ok(project) = std::env::var(CHILD_PROJECT) else {
        return;
    };
    let mode = std::env::var(CHILD_MODE).unwrap();
    let mut store = TriggerStore::open(Path::new(&project)).unwrap();
    let claim = store.claim_due(350).unwrap();
    match mode.as_str() {
        "first" => {
            let claim = claim.expect("first process should claim the latest missed slot");
            assert_eq!(claim.due_ms, 300);
            assert!(store.begin_dispatch(&claim).unwrap());
        }
        "repeat" => assert!(claim.is_none(), "a later process must not replay the slot"),
        other => panic!("unexpected child mode {other}"),
    }
}

fn child(project: &Path, mode: &str) {
    let result = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("automation_child_claim")
        .arg("--nocapture")
        .env(CHILD_PROJECT, project)
        .env(CHILD_MODE, mode)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&result.stdout)
    );
}

#[test]
fn separate_process_restart_keeps_missed_slot_single_dispatch_and_disable_effective() {
    let project = std::env::temp_dir().join(format!(
        "agentz-automation-process-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&project).unwrap();
    let trigger = Trigger {
        id: "morning".into(),
        task_id: "task-a".into(),
        interval_ms: 100,
        next_due_ms: 100,
        missed: MissedWindow::Latest,
    };
    {
        let mut store = TriggerStore::open(&project).unwrap();
        assert_eq!(store.register(&trigger).unwrap(), 1);
    }
    // The process observes a deterministic clock jump (e.g. missed scheduler
    // windows), then dies with a dispatched but uncompleted occurrence.
    child(&project, "first");
    child(&project, "repeat");
    {
        let mut store = TriggerStore::open(&project).unwrap();
        let unresolved = store.unresolved().unwrap();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].state, UnresolvedState::Dispatched);
        assert_eq!(unresolved[0].occurrence.due_ms, 300);
        assert!(!store.begin_dispatch(&unresolved[0].occurrence).unwrap());
        store.disable("morning").unwrap();
    }
    child(&project, "repeat");
    std::fs::remove_dir_all(project).unwrap();
}
