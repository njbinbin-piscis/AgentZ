//! Explicit local-owner reconciliation candidate. No startup/PID inference or execution resume.
use crate::{
    approval::Binding,
    events,
    run::{self, Execution},
};
use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use std::{path::Path, time::Duration};

/// Trusted host assertion, never renderer/model supplied. The host must independently
/// prove the captured execution owner cannot still execute. This crate cannot do so:
/// existing Run records contain no persisted boot/PID ownership.
pub enum OwnerEvidence {
    Alive,
    Unverifiable,
    ConfirmedGone { evidence_id: String },
}
pub struct CapturedRun {
    pub binding: Binding,
    pub execution: Execution,
    pub updated_ms: i64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    PreservedAlive,
    PreservedUnverifiable,
    Interrupted(events::Cursor),
}
pub struct RecoveryStore {
    conn: Connection,
    project: String,
}
fn error(e: impl std::fmt::Display) -> String {
    format!("recovery store: {e}")
}
fn state(e: Execution) -> Result<String, String> {
    serde_json::to_value(e)
        .map_err(error)?
        .as_str()
        .map(str::to_owned)
        .ok_or("invalid execution encoding".into())
}
impl RecoveryStore {
    /// Requires an existing initialized event-capable Run registry; never creates or repairs.
    pub fn open_existing(project: &Path) -> Result<Self, String> {
        let project = crate::canonical_root(project)?;
        let mut conn = Connection::open_with_flags(
            Path::new(&project).join(".agentz/runs.db"),
            OpenFlags::SQLITE_OPEN_READ_WRITE,
        )
        .map_err(error)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(error)?;
        {
            let tx = conn.transaction().map_err(error)?;
            run::validate_budget_store(&tx)?;
            let count: i64 = tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('durable_events','durable_event_schema')", [], |r| r.get(0)).map_err(error)?;
            if count != 2 {
                return Err("recovery requires existing durable event namespace".into());
            }
        }
        Ok(Self { conn, project })
    }
    /// State and lifecycle event commit together. ConfirmedGone is an explicit host
    /// proof precondition, not a fact inferred from Running, clock age or app startup.
    pub fn reconcile(
        &mut self,
        captured: &CapturedRun,
        evidence: OwnerEvidence,
        now_ms: i64,
    ) -> Result<Outcome, String> {
        captured.binding.validate()?;
        if captured.binding.project_id != self.project
            || captured.updated_ms < 0
            || now_ms < captured.updated_ms
        {
            return Err("invalid recovery project or timestamp".into());
        }
        if !matches!(
            captured.execution,
            Execution::Queued | Execution::Running | Execution::Cancelling
        ) {
            return Err("recovery only reconciles captured active execution".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        run::validate_budget_origin(&tx, &captured.binding)?;
        let (attempt, current, updated): (String, String, i64) = tx
            .query_row(
                "SELECT attempt_id,execution,updated_ms FROM runs WHERE run_id=?1",
                [&captured.binding.run_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(error)?;
        let expected = state(captured.execution)?;
        if attempt != captured.binding.attempt_id
            || current != expected
            || updated != captured.updated_ms
        {
            return Err("stale recovery capture".into());
        }
        let evidence_id = match evidence {
            OwnerEvidence::Alive => return Ok(Outcome::PreservedAlive),
            OwnerEvidence::Unverifiable => return Ok(Outcome::PreservedUnverifiable),
            OwnerEvidence::ConfirmedGone { evidence_id } => evidence_id,
        };
        if evidence_id.is_empty()
            || evidence_id.len() > 256
            || evidence_id.trim() != evidence_id
            || evidence_id.chars().any(char::is_control)
        {
            return Err("invalid trusted recovery evidence identity".into());
        }
        let changed = tx.execute("UPDATE runs SET execution='interrupted',delivery='not_ready',updated_ms=?1 WHERE run_id=?2 AND attempt_id=?3 AND execution=?4 AND updated_ms=?5", params![now_ms,captured.binding.run_id,captured.binding.attempt_id,expected,captured.updated_ms]).map_err(error)?;
        if changed != 1 {
            return Err("stale recovery Run CAS".into());
        }
        let changed = tx.execute("UPDATE attempts SET execution='interrupted',updated_ms=?1 WHERE run_id=?2 AND attempt_id=?3 AND execution=?4 AND updated_ms<=?1", params![now_ms,captured.binding.run_id,captured.binding.attempt_id,expected]).map_err(error)?;
        if changed != 1 {
            return Err("stale recovery Attempt CAS".into());
        }
        let stream = events::Stream {
            task_id: captured.binding.task_id.clone(),
            run_id: captured.binding.run_id.clone(),
            attempt_id: captured.binding.attempt_id.clone(),
            environment_id: captured.binding.environment_id.clone(),
        };
        let cursor = events::append(
            &tx,
            &stream,
            "execution.interrupted",
            "run.execution",
            &serde_json::json!({"execution":"interrupted","delivery":"not_ready","details":{"previous_execution":expected,"recovery_evidence_id":evidence_id,"automatic_resume":false}}),
            now_ms,
        )?;
        tx.commit().map_err(error)?;
        Ok(Outcome::Interrupted(cursor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::RunStore;
    struct Fixture {
        path: std::path::PathBuf,
        captured: CapturedRun,
    }
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("agentz-recovery-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            let binding = Binding {
                project_id: crate::canonical_root(&path).unwrap(),
                task_id: "task".into(),
                environment_id: format!("direct:{}", "a".repeat(64)),
                binding_version: "a".repeat(64),
                run_id: "run".into(),
                attempt_id: "a1".into(),
                capability_version: "fixture".into(),
            };
            let mut runs = RunStore::open(&path).unwrap();
            runs.create_direct(&binding, 1).unwrap();
            runs.transition("run", "a1", Execution::Queued, Execution::Running, 2)
                .unwrap();
            Self {
                path,
                captured: CapturedRun {
                    binding,
                    execution: Execution::Running,
                    updated_ms: 2,
                },
            }
        }
        fn gone() -> OwnerEvidence {
            OwnerEvidence::ConfirmedGone {
                evidence_id: "host-proof-fixture".into(),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert!(self.path.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    #[test]
    fn alive_and_unverifiable_preserve_state_and_bytes() {
        let f = Fixture::new();
        let file = f.path.join(".agentz/runs.db");
        let before = std::fs::read(&file).unwrap();
        let mut recovery = RecoveryStore::open_existing(&f.path).unwrap();
        assert_eq!(
            recovery
                .reconcile(&f.captured, OwnerEvidence::Alive, 3)
                .unwrap(),
            Outcome::PreservedAlive
        );
        assert_eq!(
            recovery
                .reconcile(&f.captured, OwnerEvidence::Unverifiable, 3)
                .unwrap(),
            Outcome::PreservedUnverifiable
        );
        drop(recovery);
        assert_eq!(std::fs::read(file).unwrap(), before);
    }
    #[test]
    fn confirmed_interruption_reopens_with_one_lifecycle_event_and_no_retry() {
        let f = Fixture::new();
        let mut recovery = RecoveryStore::open_existing(&f.path).unwrap();
        assert!(matches!(
            recovery.reconcile(&f.captured, Fixture::gone(), 3).unwrap(),
            Outcome::Interrupted(_)
        ));
        assert!(recovery.reconcile(&f.captured, Fixture::gone(), 3).is_err());
        drop(recovery);
        let mut runs = RunStore::open_existing_readonly(&f.path).unwrap();
        assert_eq!(
            runs.load("run").unwrap().unwrap().execution,
            Execution::Interrupted
        );
        assert_eq!(runs.attempts("run").unwrap().len(), 1);
        let events = runs.replay_events("run", "a1", None, 10).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[2].event_id, "execution.interrupted");
    }
    #[test]
    fn competing_captures_only_one_can_interrupt() {
        let f = Fixture::new();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads = (0..2)
            .map(|_| {
                let path = f.path.clone();
                let binding = f.captured.binding.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let captured = CapturedRun {
                        binding,
                        execution: Execution::Running,
                        updated_ms: 2,
                    };
                    barrier.wait();
                    RecoveryStore::open_existing(&path)
                        .unwrap()
                        .reconcile(&captured, Fixture::gone(), 3)
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            threads
                .into_iter()
                .map(|t| usize::from(t.join().unwrap()))
                .sum::<usize>(),
            1
        );
    }
    #[test]
    fn event_failure_rolls_back_both_state_rows() {
        let f = Fixture::new();
        let conn = Connection::open(f.path.join(".agentz/runs.db")).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_recovery BEFORE INSERT ON durable_events WHEN NEW.event_id='execution.interrupted' BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        drop(conn);
        assert!(RecoveryStore::open_existing(&f.path)
            .unwrap()
            .reconcile(&f.captured, Fixture::gone(), 3)
            .is_err());
        let mut runs = RunStore::open_existing_readonly(&f.path).unwrap();
        assert_eq!(
            runs.load("run").unwrap().unwrap().execution,
            Execution::Running
        );
        assert_eq!(
            runs.attempts("run").unwrap()[0].execution,
            Execution::Running
        );
        assert_eq!(runs.replay_events("run", "a1", None, 10).unwrap().len(), 2);
    }
    #[test]
    fn wrong_scope_corruption_and_retry_capture_are_refused() {
        let f = Fixture::new();
        let mut recovery = RecoveryStore::open_existing(&f.path).unwrap();
        let mut wrong = CapturedRun {
            binding: f.captured.binding.clone(),
            execution: Execution::Running,
            updated_ms: 2,
        };
        wrong.binding.environment_id = "other".into();
        assert!(recovery.reconcile(&wrong, Fixture::gone(), 3).is_err());
        let mut runs = RunStore::open_existing_readwrite(&f.path).unwrap();
        runs.transition("run", "a1", Execution::Running, Execution::Failed, 3)
            .unwrap();
        runs.retry("run", "a1", "a2", 4).unwrap();
        assert!(recovery.reconcile(&f.captured, Fixture::gone(), 5).is_err());
        let conn = Connection::open(f.path.join(".agentz/runs.db")).unwrap();
        conn.execute(
            "UPDATE attempts SET created_ms=-1 WHERE attempt_id='a1'",
            [],
        )
        .unwrap();
        drop(conn);
        assert!(recovery
            .reconcile(&f.captured, OwnerEvidence::Unverifiable, 5)
            .is_err());
    }
    #[test]
    fn missing_database_or_event_namespace_is_not_created_or_repaired() {
        let path =
            std::env::temp_dir().join(format!("agentz-recovery-missing-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        assert!(RecoveryStore::open_existing(&path).is_err());
        assert!(!path.join(".agentz").exists());
        std::fs::remove_dir(&path).unwrap();
        let f = Fixture::new();
        let file = f.path.join(".agentz/runs.db");
        let conn = Connection::open(&file).unwrap();
        conn.execute_batch("DROP TABLE durable_events;DROP TABLE durable_event_schema;")
            .unwrap();
        drop(conn);
        let before = std::fs::read(&file).unwrap();
        assert!(RecoveryStore::open_existing(&f.path).is_err());
        assert_eq!(std::fs::read(file).unwrap(), before);
    }
}
