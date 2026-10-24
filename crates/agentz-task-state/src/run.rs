//! Candidate durable execution state. Host lifecycle integration is tracked separately.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Execution {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn agent_binding(project: &Path) -> crate::approval::Binding {
        crate::approval::Binding {
            project_id: crate::canonical_root(project).unwrap(),
            task_id: "task".into(),
            environment_id: "environment".into(),
            binding_version: "a".repeat(64),
            run_id: "run".into(),
            attempt_id: "a1".into(),
            capability_version: "fixture".into(),
        }
    }
    #[test]
    fn agent_events_commit_reopen_deduplicate_exactly_and_preserve_run_timestamp() {
        let project = fixture();
        let binding = agent_binding(&project.0);
        let mut store = RunStore::open(&project.0).unwrap();
        store.create("run", "task", "environment", "a1", 1).unwrap();
        store
            .transition("run", "a1", Execution::Queued, Execution::Running, 2)
            .unwrap();
        let payload = serde_json::json!({"type":"text_delta","delta":"hello"});
        let event = store
            .append_agent_event(&binding, "stable-key", &payload, 3)
            .unwrap();
        assert_eq!(event.event_id, "agent.stable-key");
        assert_eq!(event.kind, "agent.event");
        assert_eq!(
            store
                .append_agent_event(&binding, "stable-key", &payload, 3)
                .unwrap(),
            event
        );
        assert!(store
            .append_agent_event(
                &binding,
                "stable-key",
                &serde_json::json!({"delta":"changed"}),
                3
            )
            .is_err());
        assert!(store
            .append_agent_event(&binding, "stable-key", &payload, 4)
            .is_err());
        assert!(store
            .append_agent_event(&binding, "agent.already-prefixed", &payload, 3)
            .is_err());
        assert_eq!(store.load("run").unwrap().unwrap().updated_ms, 2);
        drop(store);
        let mut reopened = RunStore::open_existing_readonly(&project.0).unwrap();
        let events = reopened.replay_events("run", "a1", None, 10).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[2], event);
    }
    #[test]
    fn agent_events_reject_queued_wrong_identity_oversize_terminal_and_old_retry() {
        let project = fixture();
        let binding = agent_binding(&project.0);
        let mut store = RunStore::open(&project.0).unwrap();
        store.create("run", "task", "environment", "a1", 1).unwrap();
        let payload = serde_json::json!({"type":"text_delta"});
        assert!(store
            .append_agent_event(&binding, "queued", &payload, 2)
            .is_err());
        store
            .transition("run", "a1", Execution::Queued, Execution::Running, 2)
            .unwrap();
        for field in 0..3 {
            let mut wrong = binding.clone();
            match field {
                0 => wrong.task_id = "wrong".into(),
                1 => wrong.environment_id = "wrong".into(),
                _ => wrong.project_id = "wrong".into(),
            };
            assert!(store
                .append_agent_event(&wrong, "wrong", &payload, 3)
                .is_err());
        }
        let before = store.replay_events("run", "a1", None, 10).unwrap();
        assert!(store
            .append_agent_event(
                &binding,
                "huge",
                &serde_json::json!("x".repeat(1_048_576)),
                3
            )
            .is_err());
        assert_eq!(store.replay_events("run", "a1", None, 10).unwrap(), before);
        store
            .transition("run", "a1", Execution::Running, Execution::Failed, 3)
            .unwrap();
        assert!(store
            .append_agent_event(&binding, "terminal", &payload, 4)
            .is_err());
        store.retry("run", "a1", "a2", 4).unwrap();
        store
            .transition("run", "a2", Execution::Queued, Execution::Running, 5)
            .unwrap();
        assert!(store
            .append_agent_event(&binding, "old-retry", &payload, 6)
            .is_err());
        let mut current = binding;
        current.attempt_id = "a2".into();
        assert!(store
            .append_agent_event(&current, "current", &payload, 6)
            .is_ok());
    }
    #[test]
    fn existing_readwrite_never_creates_or_migrates_missing_or_legacy_event_store() {
        let project = fixture();
        let registry = project.0.join(".agentz/task-environments.db");
        let registry_before = std::fs::read(&registry).unwrap();
        assert!(RunStore::open_existing_readwrite(&project.0).is_err());
        assert!(!project.0.join(".agentz/runs.db").exists());
        assert_eq!(std::fs::read(registry).unwrap(), registry_before);
        let binding = agent_binding(&project.0);
        let mut store = RunStore::open(&project.0).unwrap();
        store.create("run", "task", "environment", "a1", 1).unwrap();
        store
            .transition("run", "a1", Execution::Queued, Execution::Running, 2)
            .unwrap();
        drop(store);
        let file = project.0.join(".agentz/runs.db");
        let conn = Connection::open(&file).unwrap();
        conn.execute_batch("DROP TABLE durable_events; DROP TABLE durable_event_schema;")
            .unwrap();
        drop(conn);
        let before = std::fs::read(&file).unwrap();
        let mut existing = RunStore::open_existing_readwrite(&project.0).unwrap();
        assert!(existing
            .append_agent_event(&binding, "no-schema", &serde_json::Value::Null, 3)
            .is_err());
        drop(existing);
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }
    #[test]
    fn readonly_timestamp_corruption_fails_discovery_replay_and_activity_without_repair() {
        for sql in [
            "UPDATE runs SET updated_ms=-1 WHERE run_id='run'",
            "UPDATE attempts SET created_ms=-1 WHERE attempt_id='old'",
            "UPDATE attempts SET updated_ms=-1 WHERE attempt_id='old'",
            "UPDATE attempts SET created_ms=99,updated_ms=3 WHERE attempt_id='old'",
        ] {
            let project = fixture();
            let mut store = RunStore::open(&project.0).unwrap();
            store
                .create("run", "task", "environment", "old", 1)
                .unwrap();
            store
                .transition("run", "old", Execution::Queued, Execution::Running, 2)
                .unwrap();
            store
                .transition("run", "old", Execution::Running, Execution::Failed, 3)
                .unwrap();
            store.retry("run", "old", "current", 4).unwrap();
            store.conn.execute_batch(sql).unwrap();
            drop(store);
            let file = project.0.join(".agentz/runs.db");
            let before = std::fs::read(&file).unwrap();
            let mut readonly = RunStore::open_existing_readonly(&project.0).unwrap();
            assert!(readonly.has_active_task("task").is_err(), "{sql}");
            assert!(
                readonly.current_task_run("task", "environment").is_err(),
                "{sql}"
            );
            assert!(
                readonly.replay_events("run", "current", None, 10).is_err(),
                "{sql}"
            );
            if sql.starts_with("UPDATE runs") {
                assert!(readonly.load("run").is_err());
            } else {
                assert!(readonly.attempts("run").is_err());
            }
            drop(readonly);
            assert_eq!(std::fs::read(file).unwrap(), before);
        }
    }
    #[test]
    fn valid_delivery_timestamp_can_differ_from_current_attempt_timestamp() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        store
            .create("run", "task", "environment", "attempt", 1)
            .unwrap();
        store
            .transition("run", "attempt", Execution::Queued, Execution::Running, 2)
            .unwrap();
        store
            .transition(
                "run",
                "attempt",
                Execution::Running,
                Execution::Succeeded,
                3,
            )
            .unwrap();
        store
            .decide_delivery("run", "attempt", Delivery::Accepted, 10)
            .unwrap();
        assert!(!store.has_active_task("task").unwrap());
        assert_eq!(store.load("run").unwrap().unwrap().updated_ms, 10);
        assert_eq!(store.attempts("run").unwrap()[0].updated_ms, 3);
        assert!(!store
            .replay_events("run", "attempt", None, 10)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn current_task_run_uses_creation_order_current_attempt_environment_and_readonly_history() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        store
            .create("older", "task", "environment", "old-attempt", 1)
            .unwrap();
        store
            .transition(
                "older",
                "old-attempt",
                Execution::Queued,
                Execution::Running,
                2,
            )
            .unwrap();
        store
            .transition(
                "older",
                "old-attempt",
                Execution::Running,
                Execution::Failed,
                3,
            )
            .unwrap();
        store
            .create("newer", "task", "environment", "new-attempt", 4)
            .unwrap();
        store
            .transition(
                "newer",
                "new-attempt",
                Execution::Queued,
                Execution::Running,
                5,
            )
            .unwrap();
        store
            .transition(
                "newer",
                "new-attempt",
                Execution::Running,
                Execution::Failed,
                6,
            )
            .unwrap();
        store
            .retry("newer", "new-attempt", "retry-attempt", 7)
            .unwrap();
        // An old Run's later mutable activity does not make it the newest created Run.
        store
            .conn
            .execute("UPDATE runs SET updated_ms=999 WHERE run_id='older'", [])
            .unwrap();
        store.conn.execute_batch("INSERT INTO runs(run_id,task_id,environment_id,attempt_id,execution,delivery,updated_ms) VALUES('other-env','task','another-env','other-env-attempt','failed','not_ready',8); INSERT INTO attempts(attempt_id,run_id,execution,created_ms,updated_ms) VALUES('other-env-attempt','other-env','failed',8,8);").unwrap();
        drop(store);
        let file = project.0.join(".agentz/runs.db");
        let before = std::fs::read(&file).unwrap();
        let mut readonly = RunStore::open_existing_readonly(&project.0).unwrap();
        let current = readonly
            .current_task_run("task", "environment")
            .unwrap()
            .unwrap();
        assert_eq!(current.run_id, "newer");
        assert_eq!(current.attempt_id, "retry-attempt");
        assert_eq!(
            readonly
                .current_task_run("task", "another-env")
                .unwrap()
                .unwrap()
                .run_id,
            "other-env"
        );
        assert!(readonly
            .current_task_run("missing-task", "environment")
            .unwrap()
            .is_none());
        assert!(readonly
            .current_task_run("task", "missing-env")
            .unwrap()
            .is_none());
        drop(readonly);
        assert_eq!(std::fs::read(file).unwrap(), before);
    }
    #[test]
    fn current_task_run_rejects_corrupt_metadata_even_in_another_environment_without_repair() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        store
            .create("run", "task", "environment", "attempt", 1)
            .unwrap();
        store
            .conn
            .execute("UPDATE runs SET delivery='accepted' WHERE run_id='run'", [])
            .unwrap();
        drop(store);
        let file = project.0.join(".agentz/runs.db");
        let before = std::fs::read(&file).unwrap();
        let mut readonly = RunStore::open_existing_readonly(&project.0).unwrap();
        assert!(readonly.current_task_run("task", "environment").is_err());
        assert!(readonly.current_task_run("task", "missing-env").is_err());
        drop(readonly);
        assert_eq!(std::fs::read(file).unwrap(), before);
    }
    #[test]
    fn task_activity_validates_all_terminal_and_live_states() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        assert!(!store.has_active_task("task").unwrap());
        assert!(store.has_active_task(" task").is_err());
        for (run, terminal) in [
            ("success", Execution::Succeeded),
            ("failure", Execution::Failed),
            ("cancelled", Execution::Cancelled),
            ("interrupted", Execution::Interrupted),
        ] {
            store.create(run, "task", "environment", run, 1).unwrap();
            assert!(store.has_active_task("task").unwrap());
            assert!(!store.has_active_task("other-task").unwrap());
            store
                .transition(run, run, Execution::Queued, Execution::Running, 2)
                .unwrap();
            assert!(store.has_active_task("task").unwrap());
            if terminal == Execution::Cancelled {
                store
                    .transition(run, run, Execution::Running, Execution::Cancelling, 3)
                    .unwrap();
                assert!(store.has_active_task("task").unwrap());
                store
                    .transition(run, run, Execution::Cancelling, terminal, 4)
                    .unwrap();
            } else {
                store
                    .transition(run, run, Execution::Running, terminal, 4)
                    .unwrap();
            }
            assert!(!store.has_active_task("task").unwrap());
        }
        for decision in [Delivery::Accepted, Delivery::Rejected] {
            let run = decision.encode();
            store.create(run, "task", "environment", run, 1).unwrap();
            store
                .transition(run, run, Execution::Queued, Execution::Running, 2)
                .unwrap();
            store
                .transition(run, run, Execution::Running, Execution::Succeeded, 3)
                .unwrap();
            store.decide_delivery(run, run, decision, 4).unwrap();
            assert!(!store.has_active_task("task").unwrap());
        }
    }

    #[test]
    fn task_activity_reopen_keeps_leftover_run_and_retry_history() {
        let project = fixture();
        {
            let mut store = RunStore::open(&project.0).unwrap();
            store.create("run", "task", "environment", "a1", 1).unwrap();
            store
                .transition("run", "a1", Execution::Queued, Execution::Interrupted, 2)
                .unwrap();
            store.retry("run", "a1", "a2", 3).unwrap();
        }
        let mut store = RunStore::open(&project.0).unwrap();
        assert!(store.has_active_task("task").unwrap());
        assert_eq!(
            store.load("run").unwrap().unwrap().execution,
            Execution::Queued
        );
        assert_eq!(
            store.attempts("run").unwrap()[0].execution,
            Execution::Interrupted
        );
        let events_before = store.replay_events("run", "a2", None, 10).unwrap();
        assert!(store.has_active_task("task").unwrap());
        assert_eq!(
            store.replay_events("run", "a2", None, 10).unwrap(),
            events_before
        );
    }

    #[test]
    fn task_activity_fails_closed_on_corrupt_states_and_attempts() {
        for corruption in [
            "UPDATE runs SET execution='unknown' WHERE run_id='old'",
            "UPDATE runs SET delivery='unknown' WHERE run_id='old'",
            "UPDATE runs SET delivery='accepted' WHERE run_id='old'",
            "UPDATE attempts SET execution='unknown' WHERE attempt_id='old'",
            "UPDATE attempts SET execution='running' WHERE attempt_id='old'",
            "UPDATE runs SET attempt_id='live' WHERE run_id='old'",
            "DELETE FROM attempts WHERE attempt_id='old'",
        ] {
            let project = fixture();
            let mut store = RunStore::open(&project.0).unwrap();
            store
                .create("old", "task", "environment", "old", 1)
                .unwrap();
            store
                .transition("old", "old", Execution::Queued, Execution::Interrupted, 2)
                .unwrap();
            store
                .create("live", "task", "environment", "live", 3)
                .unwrap();
            store.conn.execute_batch(corruption).unwrap();
            assert!(store.has_active_task("task").is_err(), "{corruption}");
            assert_eq!(
                store.load("live").unwrap().unwrap().execution,
                Execution::Queued
            );
        }
        // Validate historical attempts too, even when the current retry is valid.
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        store.create("run", "task", "environment", "a1", 1).unwrap();
        store
            .transition("run", "a1", Execution::Queued, Execution::Interrupted, 2)
            .unwrap();
        store.retry("run", "a1", "a2", 3).unwrap();
        store
            .conn
            .execute(
                "UPDATE attempts SET execution='running' WHERE attempt_id='a1'",
                [],
            )
            .unwrap();
        assert!(store.has_active_task("task").is_err());
    }

    fn fail_events(store: &RunStore, fail: bool) {
        if fail {
            store.conn.execute_batch("CREATE TRIGGER reject_events BEFORE INSERT ON durable_events BEGIN SELECT RAISE(ABORT,'fixture event failure'); END;").unwrap();
        } else {
            store
                .conn
                .execute_batch("DROP TRIGGER reject_events;")
                .unwrap();
        }
    }

    #[test]
    fn event_failure_rolls_back_actual_create_transition_retry_and_delivery() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        fail_events(&store, true);
        assert!(store.create("run", "task", "environment", "a1", 1).is_err());
        assert!(store.load("run").unwrap().is_none());
        assert!(store.attempts("run").unwrap().is_empty());
        fail_events(&store, false);
        store.create("run", "task", "environment", "a1", 1).unwrap();
        fail_events(&store, true);
        assert!(store
            .transition("run", "a1", Execution::Queued, Execution::Running, 2)
            .is_err());
        assert_eq!(
            store.load("run").unwrap().unwrap().execution,
            Execution::Queued
        );
        assert_eq!(
            store.attempts("run").unwrap()[0].execution,
            Execution::Queued
        );
        fail_events(&store, false);
        store
            .transition("run", "a1", Execution::Queued, Execution::Running, 2)
            .unwrap();
        store
            .transition("run", "a1", Execution::Running, Execution::Failed, 3)
            .unwrap();
        fail_events(&store, true);
        assert!(store.retry("run", "a1", "a2", 4).is_err());
        assert_eq!(store.load("run").unwrap().unwrap().attempt_id, "a1");
        assert_eq!(store.attempts("run").unwrap().len(), 1);
        fail_events(&store, false);
        store.retry("run", "a1", "a2", 4).unwrap();
        store
            .transition("run", "a2", Execution::Queued, Execution::Running, 5)
            .unwrap();
        store
            .transition("run", "a2", Execution::Running, Execution::Succeeded, 6)
            .unwrap();
        fail_events(&store, true);
        assert!(store
            .decide_delivery("run", "a2", Delivery::Accepted, 7)
            .is_err());
        assert_eq!(
            store.load("run").unwrap().unwrap().delivery,
            Delivery::AwaitingReview
        );
        assert_eq!(store.replay_events("run", "a2", None, 10).unwrap().len(), 3);
    }

    #[test]
    fn retry_event_history_survives_reopen_and_keeps_attempt_cursors_isolated() {
        let project = fixture();
        let old_cursor;
        {
            let mut store = RunStore::open(&project.0).unwrap();
            store.create("run", "task", "environment", "a1", 1).unwrap();
            store
                .transition("run", "a1", Execution::Queued, Execution::Running, 2)
                .unwrap();
            store
                .transition("run", "a1", Execution::Running, Execution::Failed, 3)
                .unwrap();
            old_cursor = store.replay_events("run", "a1", None, 10).unwrap()[2]
                .cursor
                .clone();
            store.retry("run", "a1", "a2", 4).unwrap();
            assert!(store
                .transition("run", "a1", Execution::Running, Execution::Succeeded, 5)
                .is_err());
        }
        let mut store = RunStore::open(&project.0).unwrap();
        assert_eq!(store.replay_events("run", "a1", None, 10).unwrap().len(), 3);
        let events = store.replay_events("run", "a2", None, 10).unwrap();
        assert_eq!(events[0].kind, "run.retry");
        assert_eq!(events[0].payload["details"]["previous_attempt_id"], "a1");
        assert!(store
            .replay_events("run", "a2", Some(&old_cursor), 10)
            .is_err());
        assert!(store.replay_events("missing", "a1", None, 10).is_err());
    }

    #[test]
    fn legacy_v1_initializes_events_without_inventing_history() {
        let project = fixture();
        {
            let mut store = RunStore::open(&project.0).unwrap();
            store
                .create("legacy", "task", "environment", "a1", 1)
                .unwrap();
            store
                .conn
                .execute_batch("DROP TABLE durable_events; DROP TABLE durable_event_schema;")
                .unwrap();
        }
        let mut store = RunStore::open(&project.0).unwrap();
        assert_eq!(
            store
                .conn
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(store
            .replay_events("legacy", "a1", None, 10)
            .unwrap()
            .is_empty());
        store
            .transition("legacy", "a1", Execution::Queued, Execution::Running, 2)
            .unwrap();
        assert_eq!(
            store.replay_events("legacy", "a1", None, 10).unwrap().len(),
            1
        );
        store
            .conn
            .execute("UPDATE durable_event_schema SET version=2", [])
            .unwrap();
        drop(store);
        assert!(RunStore::open(&project.0).is_err());
    }

    #[test]
    fn partial_event_namespace_is_rejected_without_repairing_run_state() {
        let project = fixture();
        {
            let mut store = RunStore::open(&project.0).unwrap();
            store.create("run", "task", "environment", "a1", 1).unwrap();
            store
                .conn
                .execute_batch("DROP TABLE durable_event_schema;")
                .unwrap();
        }
        assert!(RunStore::open(&project.0).is_err());
        let connection = Connection::open(project.0.join(".agentz/runs.db")).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT execution FROM runs WHERE run_id='run'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "queued"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name='durable_event_schema'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
    #[test]
    fn empty_event_metadata_is_not_automatically_reinitialized() {
        let project = fixture();
        {
            let mut store = RunStore::open(&project.0).unwrap();
            store.create("run", "task", "environment", "a1", 1).unwrap();
            store
                .conn
                .execute("DELETE FROM durable_event_schema", [])
                .unwrap();
        }
        assert!(RunStore::open(&project.0).is_err());
        let connection = Connection::open(project.0.join(".agentz/runs.db")).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM durable_event_schema", [], |row| row
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row("SELECT execution FROM runs", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "queued"
        );
    }

    #[test]
    fn event_namespace_requires_dedup_unique_and_nonnull_constraints() {
        for missing_unique in [true, false] {
            let project = fixture();
            {
                let store = RunStore::open(&project.0).unwrap();
                let event_null = if missing_unique { "NOT NULL" } else { "" };
                let unique = if missing_unique {
                    ""
                } else {
                    ",UNIQUE(task_id,run_id,attempt_id,environment_id,event_id)"
                };
                store.conn.execute_batch(&format!("DROP TABLE durable_events;
                    CREATE TABLE durable_events (
                    task_id TEXT NOT NULL,run_id TEXT NOT NULL,attempt_id TEXT NOT NULL,environment_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL,event_id TEXT {event_null},kind TEXT NOT NULL,payload TEXT NOT NULL,created_ms INTEGER NOT NULL,
                    PRIMARY KEY(task_id,run_id,attempt_id,environment_id,sequence){unique});")).unwrap();
            }
            let error = RunStore::open(&project.0)
                .err()
                .expect("invalid namespace accepted");
            assert!(
                error.contains(if missing_unique {
                    "uniqueness"
                } else {
                    "non-null"
                }),
                "{error}"
            );
        }
    }
    struct Project(PathBuf);
    impl Drop for Project {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> Project {
        let directory = std::env::temp_dir().join(format!("agentz-run-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let root = crate::canonical_root(&directory).unwrap();
        crate::register(
            &directory,
            &crate::TaskEnvironment {
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
        Project(directory)
    }
    #[test]
    fn run_success_survives_reopen_without_acceptance_or_publication() {
        let project = fixture();
        {
            let mut store = RunStore::open(&project.0).unwrap();
            store
                .create("run", "task", "environment", "a1", 10)
                .unwrap();
            assert!(store
                .transition("run", "a1", Execution::Queued, Execution::Succeeded, 11)
                .is_err());
            store
                .transition("run", "a1", Execution::Queued, Execution::Running, 11)
                .unwrap();
            store
                .transition("run", "a1", Execution::Running, Execution::Succeeded, 12)
                .unwrap();
        }
        let mut reopened = RunStore::open(&project.0).unwrap();
        let run = reopened.load("run").unwrap().unwrap();
        assert_eq!(run.execution, Execution::Succeeded);
        assert_eq!(run.delivery, Delivery::AwaitingReview);
        reopened
            .decide_delivery("run", "a1", Delivery::Accepted, 13)
            .unwrap();
        assert!(reopened
            .decide_delivery("run", "a1", Delivery::Rejected, 14)
            .is_err());
        assert!(reopened.retry("run", "a1", "a2", 14).is_err());
    }
    #[test]
    fn run_retry_preserves_failure_and_rejects_stale_callback() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        store
            .create("run", "task", "environment", "a1", 10)
            .unwrap();
        store
            .transition("run", "a1", Execution::Queued, Execution::Running, 11)
            .unwrap();
        store
            .transition("run", "a1", Execution::Running, Execution::Failed, 12)
            .unwrap();
        store.retry("run", "a1", "a2", 13).unwrap();
        store
            .transition("run", "a2", Execution::Queued, Execution::Running, 14)
            .unwrap();
        assert!(store
            .transition("run", "a1", Execution::Running, Execution::Succeeded, 15)
            .is_err());
        let history = store.attempts("run").unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].execution, Execution::Failed);
        assert_eq!(history[1].execution, Execution::Running);
        assert_eq!(
            store.load("run").unwrap().unwrap().delivery,
            Delivery::NotReady
        );
    }
    #[test]
    fn run_duplicate_attempt_retry_rolls_back_run_update() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        store
            .create("run", "task", "environment", "a1", 10)
            .unwrap();
        store
            .transition("run", "a1", Execution::Queued, Execution::Running, 11)
            .unwrap();
        store
            .transition("run", "a1", Execution::Running, Execution::Failed, 12)
            .unwrap();
        let before = store.load("run").unwrap();
        assert!(store.retry("run", "a1", "a1", 13).is_err());
        assert_eq!(store.load("run").unwrap(), before);
        assert_eq!(store.attempts("run").unwrap().len(), 1);
    }
    #[test]
    fn direct_run_snapshot_does_not_register_or_freeze_task_environment() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        let mut binding = crate::approval::Binding {
            project_id: crate::canonical_root(&project.0).unwrap(),
            task_id: "direct-task".into(),
            environment_id: format!("direct:{}", "a".repeat(64)),
            binding_version: "a".repeat(64),
            run_id: "direct-run".into(),
            attempt_id: "direct-attempt".into(),
            capability_version: "fixture".into(),
        };
        store.create_direct(&binding, 10).unwrap();
        assert!(crate::load(&project.0, "direct-task").unwrap().is_none());
        assert_eq!(
            store.load("direct-run").unwrap().unwrap().environment_id,
            binding.environment_id
        );
        binding.task_id = "task".into();
        binding.run_id = "forbidden-managed".into();
        assert!(store.create_direct(&binding, 11).is_err());
        binding.task_id = "other-direct".into();
        binding.environment_id = "direct:wrong".into();
        assert!(store.create_direct(&binding, 11).is_err());
        assert!(store.load("forbidden-managed").unwrap().is_none());
    }
    #[test]
    fn inconsistent_attempt_rolls_back_transition_and_preserves_evidence() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        store
            .create("run", "task", "environment", "a1", 10)
            .unwrap();
        // Simulate a partial legacy write. The host must not silently repair it
        // or commit only the Run half of the next transition.
        store
            .conn
            .execute(
                "UPDATE attempts SET execution='interrupted' WHERE attempt_id='a1'",
                [],
            )
            .unwrap();
        let before = store.load("run").unwrap();
        assert!(store
            .transition("run", "a1", Execution::Queued, Execution::Running, 11,)
            .is_err());
        drop(store);
        let reopened = RunStore::open(&project.0).unwrap();
        assert_eq!(reopened.load("run").unwrap(), before);
        assert_eq!(
            reopened.attempts("run").unwrap()[0].execution,
            Execution::Interrupted
        );
    }
    #[test]
    fn run_binding_and_competing_writer_checks_are_durable() {
        let project = fixture();
        let mut first = RunStore::open(&project.0).unwrap();
        let mut second = RunStore::open(&project.0).unwrap();
        assert!(first
            .create("run", "task", "wrong-environment", "a1", 10)
            .is_err());
        assert!(first.load("run").unwrap().is_none());
        first
            .create("run", "task", "environment", "a1", 10)
            .unwrap();
        assert!(second
            .create("other-run", "task", "environment", "a2", 10)
            .is_err());
        first
            .transition("run", "a1", Execution::Queued, Execution::Running, 11)
            .unwrap();
        assert!(second
            .transition("run", "a1", Execution::Queued, Execution::Running, 12)
            .is_err());
        assert!(second
            .transition("run", "a1", Execution::Running, Execution::Failed, 9)
            .is_err());
        assert_eq!(
            second.load("run").unwrap().unwrap().execution,
            Execution::Running
        );
    }
    #[test]
    fn run_cancel_and_delivery_are_separate() {
        let project = fixture();
        let mut store = RunStore::open(&project.0).unwrap();
        store
            .create("run", "task", "environment", "a1", 10)
            .unwrap();
        assert!(store
            .decide_delivery("run", "a1", Delivery::Accepted, 11)
            .is_err());
        store
            .transition("run", "a1", Execution::Queued, Execution::Running, 11)
            .unwrap();
        store
            .transition("run", "a1", Execution::Running, Execution::Cancelling, 12)
            .unwrap();
        assert!(store
            .transition("run", "a1", Execution::Cancelling, Execution::Succeeded, 13)
            .is_err());
        store
            .transition("run", "a1", Execution::Cancelling, Execution::Cancelled, 13)
            .unwrap();
        assert_eq!(
            store.load("run").unwrap().unwrap().delivery,
            Delivery::NotReady
        );
    }

    #[test]
    fn run_concurrent_initialization_and_future_schema_fail_closed() {
        let project = fixture();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let path = project.0.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    RunStore::open(&path).map(|_| ())
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap().unwrap();
        }
        let path = project.0.join(".agentz/runs.db");
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        drop(conn);
        assert!(RunStore::open(&project.0).is_err());
        let conn = Connection::open(path).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 99);
    }

    #[test]
    fn open_reader_does_not_block_event_append() {
        let project = fixture();
        let binding = agent_binding(&project.0);
        let mut store = RunStore::open(&project.0).unwrap();
        store.create("run", "task", "environment", "a1", 1).unwrap();
        store
            .transition("run", "a1", Execution::Queued, Execution::Running, 2)
            .unwrap();
        drop(store);
        let mut reader = Connection::open_with_flags(
            project.0.join(".agentz/runs.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        prepare_runs_connection(&reader, false).unwrap();
        let tx = reader.transaction().unwrap();
        let _: i64 = tx
            .query_row("SELECT COUNT(*) FROM runs", [], |row| row.get(0))
            .unwrap();
        let mut writer = RunStore::open_existing_readwrite(&project.0).unwrap();
        let started = std::time::Instant::now();
        writer
            .append_agent_event(
                &binding,
                "while-reading",
                &serde_json::json!({"type":"text_delta","delta":"a"}),
                3,
            )
            .unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "event append waited on an open reader"
        );
        drop(tx);
    }
}
impl Execution {
    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
    fn permits(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Queued,
                Self::Running | Self::Cancelled | Self::Interrupted
            ) | (
                Self::Running,
                Self::Cancelling | Self::Succeeded | Self::Failed | Self::Interrupted
            ) | (
                Self::Cancelling,
                Self::Cancelled | Self::Failed | Self::Interrupted
            )
        )
    }
    fn encode(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Cancelling => "cancelling",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "cancelling" => Ok(Self::Cancelling),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err("unsupported persisted execution state".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    NotReady,
    AwaitingReview,
    Accepted,
    Rejected,
}
impl Delivery {
    fn encode(self) -> &'static str {
        match self {
            Self::NotReady => "not_ready",
            Self::AwaitingReview => "awaiting_review",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "not_ready" => Ok(Self::NotReady),
            "awaiting_review" => Ok(Self::AwaitingReview),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            _ => Err("unsupported persisted delivery state".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub run_id: String,
    pub task_id: String,
    pub environment_id: String,
    pub attempt_id: String,
    pub execution: Execution,
    pub delivery: Delivery,
    pub updated_ms: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    pub attempt_id: String,
    pub execution: Execution,
    pub created_ms: i64,
    pub updated_ms: i64,
}

fn err(error: impl std::fmt::Display) -> String {
    format!("run store: {error}")
}

/// Events, budget and reservations share `runs.db` for a whole turn. Rollback
/// journal mode lets one of those connections hold the file until
/// `busy_timeout` expires, which surfaces as "database is locked" and aborts
/// the turn. WAL lets a writer proceed while another connection is reading.
pub(crate) fn prepare_runs_connection(conn: &Connection, writable: bool) -> Result<(), String> {
    conn.busy_timeout(Duration::from_secs(15)).map_err(err)?;
    if writable {
        let mode: String = conn
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
            .unwrap_or_default();
        if mode.eq_ignore_ascii_case("wal") {
            let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        }
    }
    Ok(())
}
fn id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err("invalid run identity".into())
    } else {
        Ok(())
    }
}

pub struct RunStore {
    conn: Connection,
    project: PathBuf,
}
impl RunStore {
    /// History reads never create a database, initialize events or repair old Runs.
    pub fn open_existing_readonly(project: &Path) -> Result<Self, String> {
        Self::open_existing(project, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
    }
    /// Append to an existing initialized store; never recreate or migrate it.
    pub fn open_existing_readwrite(project: &Path) -> Result<Self, String> {
        Self::open_existing(project, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
    }
    fn open_existing(project: &Path, flags: rusqlite::OpenFlags) -> Result<Self, String> {
        let project = PathBuf::from(super::canonical_root(project)?);
        let mut conn =
            Connection::open_with_flags(project.join(".agentz/runs.db"), flags).map_err(err)?;
        prepare_runs_connection(&conn, !flags.contains(rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY))?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(err)?;
        if version != 1 {
            return Err("existing run registry schema is unsupported".into());
        }
        {
            let tx = conn.transaction().map_err(err)?;
            validate_event_layout(&tx)?;
            // Require the existing Run and Attempt query shape without changing it.
            tx.prepare("SELECT run_id,task_id,environment_id,attempt_id,execution,delivery,updated_ms FROM runs LIMIT 0").map_err(err)?;
            tx.prepare(
                "SELECT run_id,attempt_id,execution,created_ms,updated_ms FROM attempts LIMIT 0",
            )
            .map_err(err)?;
        }
        Ok(Self { conn, project })
    }
    pub fn open(project: &Path) -> Result<Self, String> {
        let project = PathBuf::from(super::canonical_root(project)?);
        let directory = project.join(".agentz");
        std::fs::create_dir_all(&directory).map_err(err)?;
        let mut conn = Connection::open(directory.join("runs.db")).map_err(err)?;
        prepare_runs_connection(&conn, true)?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(err)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(err)?;
        if version > 1 {
            return Err("run registry schema is newer than supported".into());
        }
        {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(err)?;
            // Another opener may have initialized while this connection waited.
            let locked_version: i64 = tx
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .map_err(err)?;
            if locked_version > 1 {
                return Err("run registry schema is newer than supported".into());
            }
            if locked_version == 0 {
                tx.execute_batch("CREATE TABLE runs (
                run_id TEXT PRIMARY KEY NOT NULL, task_id TEXT NOT NULL, environment_id TEXT NOT NULL,
                attempt_id TEXT NOT NULL, execution TEXT NOT NULL, delivery TEXT NOT NULL, updated_ms INTEGER NOT NULL);
                CREATE TABLE attempts (
                attempt_id TEXT PRIMARY KEY NOT NULL, run_id TEXT NOT NULL REFERENCES runs(run_id),
                execution TEXT NOT NULL, created_ms INTEGER NOT NULL, updated_ms INTEGER NOT NULL);
                CREATE UNIQUE INDEX active_task_run ON runs(task_id) WHERE execution IN ('queued','running','cancelling');
                PRAGMA user_version=1;").map_err(err)?;
            }
            validate_event_layout(&tx)?;
            crate::events::initialize(&tx)?;
            tx.commit().map_err(err)?;
        }
        Ok(Self { conn, project })
    }

    pub fn create(
        &mut self,
        run_id: &str,
        task_id: &str,
        environment_id: &str,
        attempt_id: &str,
        now_ms: i64,
    ) -> Result<(), String> {
        for value in [run_id, task_id, environment_id, attempt_id] {
            id(value)?;
        }
        if now_ms < 0 {
            return Err("invalid execution timestamp".into());
        }
        let environment =
            super::load(&self.project, task_id)?.ok_or("task environment not registered")?;
        if environment.environment_id != environment_id {
            return Err("run environment identity mismatch".into());
        }
        self.insert(run_id, task_id, environment_id, attempt_id, now_ms)
    }

    /// Only a trusted host may supply this binding after validating workspace/Git identity.
    /// This stores a per-run snapshot identity; it never registers a frozen task environment.
    pub fn create_direct(
        &mut self,
        binding: &crate::approval::Binding,
        now_ms: i64,
    ) -> Result<(), String> {
        binding.validate()?;
        if now_ms < 0 || binding.project_id != super::canonical_root(&self.project)? {
            return Err("direct run project or timestamp mismatch".into());
        }
        if super::load(&self.project, &binding.task_id)?.is_some() {
            return Err("registered task requires managed run binding".into());
        }
        if binding.binding_version.len() != 64
            || !binding
                .binding_version
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
            || binding.environment_id != format!("direct:{}", binding.binding_version)
        {
            return Err("invalid direct run fingerprint".into());
        }
        self.insert(
            &binding.run_id,
            &binding.task_id,
            &binding.environment_id,
            &binding.attempt_id,
            now_ms,
        )
    }

    fn insert(
        &mut self,
        run_id: &str,
        task_id: &str,
        environment_id: &str,
        attempt_id: &str,
        now_ms: i64,
    ) -> Result<(), String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        tx.execute(
            "INSERT INTO runs VALUES (?1,?2,?3,?4,'queued','not_ready',?5)",
            params![run_id, task_id, environment_id, attempt_id, now_ms],
        )
        .map_err(err)?;
        tx.execute(
            "INSERT INTO attempts VALUES (?1,?2,'queued',?3,?3)",
            params![attempt_id, run_id, now_ms],
        )
        .map_err(err)?;
        append_lifecycle(
            &tx,
            run_id,
            attempt_id,
            "run.created",
            "run.created",
            now_ms,
            serde_json::Value::Null,
        )?;
        tx.commit().map_err(err)
    }

    pub fn load(&self, run_id: &str) -> Result<Option<Run>, String> {
        let raw = self.conn.query_row("SELECT task_id,environment_id,attempt_id,execution,delivery,updated_ms FROM runs WHERE run_id=?1", [run_id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?))).optional().map_err(err)?;
        raw.map(
            |(task_id, environment_id, attempt_id, execution, delivery, updated_ms)| {
                if updated_ms < 0 {
                    return Err("invalid Run timestamp".into());
                }
                Ok(Run {
                    run_id: run_id.into(),
                    task_id,
                    environment_id,
                    attempt_id,
                    execution: Execution::parse(&execution)?,
                    delivery: Delivery::parse(&delivery)?,
                    updated_ms,
                })
            },
        )
        .transpose()
    }

    /// Every Run still in an active execution state, for startup reconciliation.
    /// Any corrupt row fails the whole listing instead of being skipped.
    pub fn active_runs(&self) -> Result<Vec<Run>, String> {
        let mut query = self
            .conn
            .prepare("SELECT run_id FROM runs WHERE execution IN ('queued','running','cancelling') ORDER BY rowid")
            .map_err(err)?;
        let ids = query
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        ids.iter()
            .map(|run_id| {
                self.load(run_id)?
                    .ok_or_else(|| "active Run disappeared".into())
            })
            .collect()
    }

    /// Discover the latest created Run for the current environment, without
    /// treating mutable activity timestamps as creation order or writing state.
    pub fn current_task_run(
        &mut self,
        task_id: &str,
        environment_id: &str,
    ) -> Result<Option<Run>, String> {
        id(task_id)?;
        id(environment_id)?;
        let _active = self.has_active_task(task_id)?;
        let run_id: Option<String> = self.conn.query_row(
            "SELECT run_id FROM runs WHERE task_id=?1 AND environment_id=?2 ORDER BY rowid DESC LIMIT 1",
            params![task_id,environment_id],|row|row.get(0),
        ).optional().map_err(err)?;
        run_id
            .map(|run_id| {
                self.load(&run_id)?
                    .ok_or_else(|| "current task Run disappeared".into())
            })
            .transpose()
    }

    /// Authoritative activity at one read snapshot; corrupt history is not inactivity.
    /// Caller must serialize this check with any operation it protects.
    pub fn has_active_task(&mut self, task_id: &str) -> Result<bool, String> {
        id(task_id)?;
        let tx = self.conn.transaction().map_err(err)?;
        let mut query = tx.prepare("SELECT run_id,environment_id,attempt_id,execution,delivery,updated_ms FROM runs WHERE task_id=?1").map_err(err)?;
        let runs = query
            .query_map([task_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        let mut active = false;
        for (run, environment, current, raw_execution, raw_delivery, updated_ms) in runs {
            if updated_ms < 0 {
                return Err("invalid Run timestamp".into());
            }
            id(&run)?;
            id(&environment)?;
            id(&current)?;
            let execution = Execution::parse(&raw_execution)?;
            let delivery = Delivery::parse(&raw_delivery)?;
            let delivery_consistent = if execution == Execution::Succeeded {
                delivery != Delivery::NotReady
            } else {
                delivery == Delivery::NotReady
            };
            if !delivery_consistent {
                return Err("run execution and delivery are inconsistent".into());
            }
            let mut query = tx
                .prepare("SELECT attempt_id,execution,created_ms,updated_ms FROM attempts WHERE run_id=?1")
                .map_err(err)?;
            let attempts = query
                .query_map([&run], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .map_err(err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?;
            let mut found_current = false;
            for (attempt, raw_state, created_ms, updated_ms) in attempts {
                validate_attempt_time(created_ms, updated_ms)?;
                id(&attempt)?;
                let state = Execution::parse(&raw_state)?;
                if attempt == current {
                    if found_current || state != execution {
                        return Err("current attempt state does not match run".into());
                    }
                    found_current = true;
                } else if !state.terminal() {
                    return Err("historical attempt is unexpectedly active".into());
                }
            }
            if !found_current {
                return Err("current attempt is missing from run history".into());
            }
            active |= !execution.terminal();
        }
        Ok(active)
    }

    /// Compare-and-swap on current attempt and state; stale callbacks cannot finish a retry.
    pub fn transition(
        &mut self,
        run_id: &str,
        attempt_id: &str,
        expected: Execution,
        next: Execution,
        now_ms: i64,
    ) -> Result<(), String> {
        if !expected.permits(next) {
            return Err("illegal execution transition".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        let delivery = if next == Execution::Succeeded {
            Delivery::AwaitingReview
        } else {
            Delivery::NotReady
        };
        let changed = tx.execute("UPDATE runs SET execution=?1,delivery=?2,updated_ms=?3 WHERE run_id=?4 AND attempt_id=?5 AND execution=?6 AND updated_ms<=?3", params![next.encode(),delivery.encode(),now_ms,run_id,attempt_id,expected.encode()]).map_err(err)?;
        if changed != 1 {
            return Err("stale run state, attempt or timestamp".into());
        }
        let changed = tx.execute("UPDATE attempts SET execution=?1,updated_ms=?2 WHERE attempt_id=?3 AND run_id=?4 AND execution=?5", params![next.encode(),now_ms,attempt_id,run_id,expected.encode()]).map_err(err)?;
        if changed != 1 {
            return Err("attempt state does not match run".into());
        }
        append_lifecycle(
            &tx,
            run_id,
            attempt_id,
            &format!("execution.{}", next.encode()),
            "run.execution",
            now_ms,
            serde_json::json!({"previous_execution":expected.encode()}),
        )?;
        tx.commit().map_err(err)
    }

    pub fn retry(
        &mut self,
        run_id: &str,
        previous_attempt: &str,
        new_attempt: &str,
        now_ms: i64,
    ) -> Result<(), String> {
        id(new_attempt)?;
        let current = self.load(run_id)?.ok_or("run not found")?;
        if !current.execution.terminal() || current.execution == Execution::Succeeded {
            return Err("run cannot be retried in this state".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        let changed = tx.execute("UPDATE runs SET attempt_id=?1,execution='queued',delivery='not_ready',updated_ms=?2 WHERE run_id=?3 AND attempt_id=?4 AND execution=?5 AND updated_ms<=?2", params![new_attempt,now_ms,run_id,previous_attempt,current.execution.encode()]).map_err(err)?;
        if changed != 1 {
            return Err("stale retry request".into());
        }
        tx.execute(
            "INSERT INTO attempts VALUES (?1,?2,'queued',?3,?3)",
            params![new_attempt, run_id, now_ms],
        )
        .map_err(err)?;
        append_lifecycle(
            &tx,
            run_id,
            new_attempt,
            "attempt.queued",
            "run.retry",
            now_ms,
            serde_json::json!({"previous_attempt_id":previous_attempt,"attempt_id":new_attempt}),
        )?;
        tx.commit().map_err(err)
    }

    /// Human delivery decision only. No API here publishes or sends artifacts.
    pub fn decide_delivery(
        &mut self,
        run_id: &str,
        attempt_id: &str,
        decision: Delivery,
        now_ms: i64,
    ) -> Result<(), String> {
        if !matches!(decision, Delivery::Accepted | Delivery::Rejected) {
            return Err("invalid delivery decision".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        let changed = tx.execute("UPDATE runs SET delivery=?1,updated_ms=?2 WHERE run_id=?3 AND attempt_id=?4 AND execution='succeeded' AND delivery='awaiting_review' AND updated_ms<=?2", params![decision.encode(),now_ms,run_id,attempt_id]).map_err(err)?;
        if changed != 1 {
            return Err("delivery is not awaiting review for this attempt".into());
        }
        append_lifecycle(
            &tx,
            run_id,
            attempt_id,
            "delivery.decided",
            "run.delivery",
            now_ms,
            serde_json::Value::Null,
        )?;
        tx.commit().map_err(err)
    }

    /// Persist one observed agent event. Caller owns the fresh environment lease;
    /// this does not authorize or resume any tool operation.
    pub fn append_agent_event(
        &mut self,
        binding: &crate::approval::Binding,
        event_id: &str,
        payload: &serde_json::Value,
        now_ms: i64,
    ) -> Result<crate::events::Event, String> {
        binding.validate()?;
        if binding.project_id != super::canonical_root(&self.project)? {
            return Err("agent event project binding mismatch".into());
        }
        id(event_id)?;
        if event_id.starts_with("agent.") {
            return Err("agent event identity must be an unprefixed stable key".into());
        }
        let event_id = format!("agent.{event_id}");
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        let (task,environment,current,execution,delivery,updated):(String,String,String,String,String,i64)=tx.query_row(
            "SELECT task_id,environment_id,attempt_id,execution,delivery,updated_ms FROM runs WHERE run_id=?1",[&binding.run_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)),
        ).map_err(err)?;
        if task != binding.task_id
            || environment != binding.environment_id
            || current != binding.attempt_id
            || Execution::parse(&execution)? != Execution::Running
            || Delivery::parse(&delivery)? != Delivery::NotReady
            || updated < 0
        {
            return Err("agent event does not match an authoritative running Run/Attempt".into());
        }
        let (attempt_state,created,attempt_updated):(String,i64,i64)=tx.query_row("SELECT execution,created_ms,updated_ms FROM attempts WHERE run_id=?1 AND attempt_id=?2",params![binding.run_id,binding.attempt_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
        validate_attempt_time(created, attempt_updated)?;
        if Execution::parse(&attempt_state)? != Execution::Running {
            return Err("agent event Attempt is not running".into());
        }
        let stream = crate::events::Stream {
            task_id: task,
            environment_id: environment,
            run_id: binding.run_id.clone(),
            attempt_id: current,
        };
        let cursor =
            crate::events::append(&tx, &stream, &event_id, "agent.event", payload, now_ms)?;
        tx.commit().map_err(err)?;
        Ok(crate::events::Event {
            cursor,
            event_id,
            kind: "agent.event".into(),
            payload: payload.clone(),
            created_ms: now_ms,
        })
    }

    /// Scoped history, including prior Attempts. Empty legacy history means
    /// event capture was not active then, not that the Run never executed.
    pub fn replay_events(
        &mut self,
        run_id: &str,
        attempt_id: &str,
        after: Option<&crate::events::Cursor>,
        limit: usize,
    ) -> Result<Vec<crate::events::Event>, String> {
        let run = self.load(run_id)?.ok_or("event Run not found")?;
        let _active = self.has_active_task(&run.task_id)?;
        let tx = self.conn.transaction().map_err(err)?;
        let stream = authoritative_stream(&tx, run_id, attempt_id)?;
        validate_event_layout(&tx)?;
        let namespaces: i64 = tx.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('durable_events','durable_event_schema')",
            [], |row| row.get(0),
        ).map_err(err)?;
        if namespaces == 0 {
            if !(1..=1000).contains(&limit) {
                return Err("invalid event page limit".into());
            }
            if after.is_some_and(|cursor| cursor.stream != stream || cursor.sequence != 0) {
                return Err("legacy history cursor is invalid or ahead of stream".into());
            }
            return Ok(Vec::new());
        }
        crate::events::replay(&tx, &stream, after, limit)
    }

    /// Read a single current Attempt cursor without replaying its history.
    /// Legacy stores with no event namespace return None, which callers must
    /// describe as unknown rather than claiming the task never emitted events.
    pub fn latest_event_cursor(
        &mut self,
        run_id: &str,
        attempt_id: &str,
    ) -> Result<Option<crate::events::Cursor>, String> {
        let run = self.load(run_id)?.ok_or("event Run not found")?;
        let _active = self.has_active_task(&run.task_id)?;
        let tx = self.conn.transaction().map_err(err)?;
        let stream = authoritative_stream(&tx, run_id, attempt_id)?;
        validate_event_layout(&tx)?;
        let namespaces: i64 = tx.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('durable_events','durable_event_schema')",
            [], |row| row.get(0),
        ).map_err(err)?;
        if namespaces == 0 {
            return Ok(None);
        }
        let sequence: Option<i64> = tx.query_row(
            "SELECT MAX(sequence) FROM durable_events WHERE task_id=?1 AND run_id=?2 AND attempt_id=?3 AND environment_id=?4",
            params![stream.task_id, stream.run_id, stream.attempt_id, stream.environment_id],
            |row| row.get(0),
        ).map_err(err)?;
        match sequence {
            Some(sequence) if sequence > 0 => Ok(Some(crate::events::Cursor {stream,sequence})),
            Some(_) => Err("invalid event cursor sequence".into()),
            None => Ok(None),
        }
    }

    pub fn attempts(&self, run_id: &str) -> Result<Vec<Attempt>, String> {
        let mut query = self.conn.prepare("SELECT attempt_id,execution,created_ms,updated_ms FROM attempts WHERE run_id=?1 ORDER BY rowid").map_err(err)?;
        let rows = query
            .query_map([run_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(err)?;
        rows.map(|r| {
            let (attempt_id, state, created_ms, updated_ms) = r.map_err(err)?;
            validate_attempt_time(created_ms, updated_ms)?;
            Ok(Attempt {
                attempt_id,
                execution: Execution::parse(&state)?,
                created_ms,
                updated_ms,
            })
        })
        .collect()
    }
}

pub(crate) fn validate_budget_store(tx: &rusqlite::Transaction<'_>) -> Result<(), String> {
    let version: i64 = tx
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(err)?;
    if version != 1 {
        return Err("existing run registry schema is unsupported".into());
    }
    validate_event_layout(tx)?;
    for (table, expected) in [
        (
            "runs",
            vec![
                ("run_id", "TEXT", 1),
                ("task_id", "TEXT", 0),
                ("environment_id", "TEXT", 0),
                ("attempt_id", "TEXT", 0),
                ("execution", "TEXT", 0),
                ("delivery", "TEXT", 0),
                ("updated_ms", "INTEGER", 0),
            ],
        ),
        (
            "attempts",
            vec![
                ("attempt_id", "TEXT", 1),
                ("run_id", "TEXT", 0),
                ("execution", "TEXT", 0),
                ("created_ms", "INTEGER", 0),
                ("updated_ms", "INTEGER", 0),
            ],
        ),
    ] {
        let mut query = tx
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(err)?;
        let columns = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        if columns.len() != expected.len()
            || columns
                .iter()
                .zip(expected)
                .any(|(actual, (name, kind, pk))| {
                    actual.0 != name || actual.1 != kind || actual.2 != 1 || actual.3 != pk
                })
        {
            return Err("budget Run namespace constraints are invalid".into());
        }
    }
    tx.prepare("SELECT run_id,task_id,environment_id,attempt_id,execution,delivery,updated_ms FROM runs LIMIT 0").map_err(err)?;
    tx.prepare("SELECT run_id,attempt_id,execution,created_ms,updated_ms FROM attempts LIMIT 0")
        .map_err(err)?;
    Ok(())
}

/// Accounting attribution only; this never authorizes a new request or tool.
pub(crate) fn validate_budget_origin(
    tx: &rusqlite::Transaction<'_>,
    binding: &crate::approval::Binding,
) -> Result<(), String> {
    binding.validate()?;
    validate_budget_store(tx)?;
    validate_budget_run(tx, &binding.run_id, Some(binding))
}

pub(crate) fn validate_budget_task(
    tx: &rusqlite::Transaction<'_>,
    task: &str,
) -> Result<(), String> {
    id(task)?;
    validate_budget_store(tx)?;
    let mut query = tx
        .prepare("SELECT run_id FROM runs WHERE task_id=?1")
        .map_err(err)?;
    let runs = query
        .query_map([task], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    if runs.is_empty() {
        return Err("budget requires an existing task Run".into());
    }
    for run in runs {
        validate_budget_run(tx, &run, None)?;
    }
    Ok(())
}

fn validate_budget_run(
    tx: &rusqlite::Transaction<'_>,
    run: &str,
    binding: Option<&crate::approval::Binding>,
) -> Result<(), String> {
    id(run)?;
    let (task, environment, current, raw_state, raw_delivery, updated): (String,String,String,String,String,i64) = tx.query_row("SELECT task_id,environment_id,attempt_id,execution,delivery,updated_ms FROM runs WHERE run_id=?1", [run], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(err)?;
    id(&task)?;
    id(&environment)?;
    if binding.is_some_and(|b| task != b.task_id || environment != b.environment_id) || updated < 0
    {
        return Err("budget Run identity or timestamp mismatch".into());
    }
    id(&current)?;
    let execution = Execution::parse(&raw_state)?;
    let delivery = Delivery::parse(&raw_delivery)?;
    if (execution == Execution::Succeeded) == (delivery == Delivery::NotReady) {
        return Err("run execution and delivery are inconsistent".into());
    }
    let mut query = tx
        .prepare("SELECT attempt_id,execution,created_ms,updated_ms FROM attempts WHERE run_id=?1")
        .map_err(err)?;
    let mut rows = query.query([run]).map_err(err)?;
    let mut found_current = false;
    let mut found_origin = binding.is_none();
    while let Some(row) = rows.next().map_err(err)? {
        let attempt: String = row.get(0).map_err(err)?;
        id(&attempt)?;
        let state = Execution::parse(&row.get::<_, String>(1).map_err(err)?)?;
        validate_attempt_time(row.get(2).map_err(err)?, row.get(3).map_err(err)?)?;
        if attempt == current {
            if found_current || state != execution {
                return Err("current attempt state does not match run".into());
            }
            found_current = true;
        } else if !state.terminal() {
            return Err("historical attempt is unexpectedly active".into());
        }
        if binding.is_some_and(|b| attempt == b.attempt_id) {
            found_origin = true;
        }
    }
    if !found_current || !found_origin {
        return Err("budget Attempt does not belong to authoritative Run".into());
    }
    Ok(())
}

fn validate_attempt_time(created_ms: i64, updated_ms: i64) -> Result<(), String> {
    if created_ms < 0 || updated_ms < 0 || created_ms > updated_ms {
        return Err("invalid Attempt timestamps".into());
    }
    Ok(())
}

fn authoritative_stream(
    tx: &rusqlite::Transaction<'_>,
    run_id: &str,
    attempt_id: &str,
) -> Result<crate::events::Stream, String> {
    tx.query_row("SELECT r.task_id,r.environment_id FROM runs r JOIN attempts a ON a.run_id=r.run_id WHERE r.run_id=?1 AND a.attempt_id=?2", params![run_id,attempt_id], |row| Ok(crate::events::Stream {
        task_id: row.get(0)?, environment_id: row.get(1)?, run_id: run_id.into(), attempt_id: attempt_id.into(),
    })).optional().map_err(err)?.ok_or_else(|| "event attempt does not belong to run".into())
}

fn validate_event_layout(tx: &rusqlite::Transaction<'_>) -> Result<(), String> {
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('durable_events','durable_event_schema')", [], |row| row.get(0)).map_err(err)?;
    if count == 0 {
        return Ok(());
    }
    if count != 2 {
        return Err("partial event namespace requires explicit recovery".into());
    }
    let (versions, version): (i64, Option<i64>) = tx
        .query_row(
            "SELECT COUNT(*),MAX(version) FROM durable_event_schema",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(err)?;
    if versions != 1 || version != Some(1) {
        return Err("existing event namespace metadata requires explicit recovery".into());
    }
    for (table, expected) in [
        (
            "durable_events",
            vec![
                ("task_id", "TEXT", 1),
                ("run_id", "TEXT", 2),
                ("attempt_id", "TEXT", 3),
                ("environment_id", "TEXT", 4),
                ("sequence", "INTEGER", 5),
                ("event_id", "TEXT", 0),
                ("kind", "TEXT", 0),
                ("payload", "TEXT", 0),
                ("created_ms", "INTEGER", 0),
            ],
        ),
        ("durable_event_schema", vec![("version", "INTEGER", 1)]),
    ] {
        let mut query = tx
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(err)?;
        let columns: Vec<(String, String, i64)> = query
            .query_map([], |row| Ok((row.get(1)?, row.get(2)?, row.get(5)?)))
            .map_err(err)?
            .collect::<Result<_, _>>()
            .map_err(err)?;
        if columns
            != expected
                .into_iter()
                .map(|(name, kind, key)| (name.to_string(), kind.to_string(), key))
                .collect::<Vec<_>>()
        {
            return Err("event namespace layout requires explicit migration".into());
        }
        if table == "durable_events" {
            let nullable: i64 = tx.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('durable_events') WHERE \"notnull\" != 1", [], |row| row.get(0),
            ).map_err(err)?;
            if nullable != 0 {
                return Err("event namespace requires non-null event fields".into());
            }
        }
    }
    let mut query = tx
        .prepare(
            "SELECT name FROM pragma_index_list('durable_events') WHERE \"unique\"=1 AND partial=0",
        )
        .map_err(err)?;
    let indexes = query
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let required = [
        "task_id",
        "run_id",
        "attempt_id",
        "environment_id",
        "event_id",
    ];
    let mut dedup_unique = false;
    for index in indexes {
        let mut query = tx
            .prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
            .map_err(err)?;
        let columns = query
            .query_map([index], |row| row.get::<_, Option<String>>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        if columns
            == required
                .iter()
                .map(|column| Some(column.to_string()))
                .collect::<Vec<_>>()
        {
            dedup_unique = true;
        }
    }
    if !dedup_unique {
        return Err("event namespace requires scoped event-id uniqueness".into());
    }
    Ok(())
}

fn append_lifecycle(
    tx: &rusqlite::Transaction<'_>,
    run_id: &str,
    attempt_id: &str,
    event_id: &str,
    kind: &str,
    now_ms: i64,
    details: serde_json::Value,
) -> Result<(), String> {
    let stream = authoritative_stream(tx, run_id, attempt_id)?;
    let (execution, delivery): (String, String) = tx
        .query_row(
            "SELECT execution,delivery FROM runs WHERE run_id=?1 AND attempt_id=?2",
            params![run_id, attempt_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(err)?;
    Execution::parse(&execution)?;
    Delivery::parse(&delivery)?;
    crate::events::append(
        tx,
        &stream,
        event_id,
        kind,
        &serde_json::json!({"execution":execution,"delivery":delivery,"details":details}),
        now_ms,
    )?;
    Ok(())
}
