//! Local single-dispatch intent ledger. No provider calls, automatic replay or OS authorization.
use crate::{approval::Binding, events, run};
use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub task_id: String,
    pub action_key: String,
    pub provider: String,
    pub kind: String,
    pub target_digest: String,
    pub payload_digest: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::{Execution, RunStore};
    struct Fixture {
        path: std::path::PathBuf,
        binding: Binding,
    }
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("external-action-{}", uuid::Uuid::new_v4()));
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
            Self { path, binding }
        }
        fn store(&self) -> ExternalActionStore {
            let mut s = ExternalActionStore::open_existing(&self.path).unwrap();
            s.initialize_namespace().unwrap();
            s
        }
        fn scope() -> Scope {
            Scope {
                task_id: "task".into(),
                action_key: "send-1".into(),
                provider: "controlled-local".into(),
                kind: "synthetic-post".into(),
                target_digest: "b".repeat(64),
                payload_digest: "c".repeat(64),
            }
        }
        fn prepared(&self, s: &mut ExternalActionStore) {
            assert!(matches!(
                s.prepare(Self::scope(), &self.binding, 3).unwrap(),
                Prepared::Created(_)
            ));
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    #[test]
    fn duplicate_prepare_is_existing_and_immutable_scope_conflicts() {
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        assert!(matches!(
            s.prepare(Fixture::scope(), &f.binding, 4).unwrap(),
            Prepared::Existing(_)
        ));
        let mut changed = Fixture::scope();
        changed.payload_digest = "d".repeat(64);
        assert!(s.prepare(changed, &f.binding, 4).is_err());
        assert_eq!(s.query("task", "send-1").unwrap().unwrap().revision, 1);
    }
    #[test]
    fn competing_connections_issue_exactly_one_permit() {
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        drop(s);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers = (0..2)
            .map(|_| {
                let path = f.path.clone();
                let binding = f.binding.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut s = ExternalActionStore::open_existing(&path).unwrap();
                    barrier.wait();
                    s.start("task", "send-1", 1, &binding, 4).is_ok()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            workers
                .into_iter()
                .filter_map(|w| w.join().ok())
                .filter(|ok| *ok)
                .count(),
            1
        );
    }
    #[test]
    fn controlled_service_loses_result_reopen_never_replays() {
        struct Service {
            effects: usize,
        }
        impl Service {
            fn dispatch(&mut self, _permit: StartPermit) {
                self.effects += 1; /* response lost after effect */
            }
        }
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        let permit = s.start("task", "send-1", 1, &f.binding, 4).unwrap();
        let mut service = Service { effects: 0 };
        service.dispatch(permit);
        drop(s);
        let mut s = ExternalActionStore::open_existing(&f.path).unwrap();
        let a = s.query("task", "send-1").unwrap().unwrap();
        assert_eq!(a.state, State::Started);
        assert!(s
            .start("task", "send-1", a.revision, &f.binding, 5)
            .is_err());
        assert!(matches!(
            s.prepare(Fixture::scope(), &f.binding, 5).unwrap(),
            Prepared::Existing(_)
        ));
        assert_eq!(
            s.reconcile(
                "task",
                "send-1",
                a.revision,
                Evidence::Confirmed {
                    receipt_digest: "d".repeat(64)
                },
                6
            )
            .unwrap()
            .state,
            State::Confirmed
        );
        assert_eq!(service.effects, 1);
        assert!(s.start("task", "send-1", 3, &f.binding, 7).is_err());
    }
    #[test]
    fn unknown_requires_evidence_and_failed_is_terminal() {
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        let permit = s.start("task", "send-1", 1, &f.binding, 4).unwrap();
        let a = s.mark_unknown(permit, 5).unwrap();
        assert_eq!(a.state, State::Unknown);
        assert!(s
            .start("task", "send-1", a.revision, &f.binding, 6)
            .is_err());
        let done = s
            .reconcile(
                "task",
                "send-1",
                a.revision,
                Evidence::NoEffect {
                    evidence_digest: "e".repeat(64),
                },
                6,
            )
            .unwrap();
        assert_eq!(done.state, State::Failed);
        assert!(s
            .start("task", "send-1", done.revision, &f.binding, 7)
            .is_err());
    }
    #[test]
    fn historical_attempt_cannot_start_and_wrong_scope_rejected() {
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        let mut runs = RunStore::open_existing_readwrite(&f.path).unwrap();
        runs.transition("run", "a1", Execution::Running, Execution::Failed, 4)
            .unwrap();
        runs.retry("run", "a1", "a2", 5).unwrap();
        assert!(s.start("task", "send-1", 1, &f.binding, 6).is_err());
        let mut wrong = f.binding.clone();
        wrong.task_id = "other".into();
        assert!(s.prepare(Fixture::scope(), &wrong, 6).is_err());
        assert_eq!(
            s.query("task", "send-1").unwrap().unwrap().state,
            State::Prepared
        );
    }
    #[test]
    fn sql_completion_failure_keeps_started_and_no_replay() {
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        let permit = s.start("task", "send-1", 1, &f.binding, 4).unwrap();
        let c = Connection::open(f.path.join(".agentz/runs.db")).unwrap();
        c.execute_batch("CREATE TRIGGER reject_external BEFORE UPDATE ON external_actions BEGIN SELECT RAISE(ABORT,'fixture completion failure'); END;").unwrap();
        drop(c);
        assert!(s
            .complete(
                permit,
                Evidence::Confirmed {
                    receipt_digest: "d".repeat(64)
                },
                5
            )
            .is_err());
        assert_eq!(
            s.query("task", "send-1").unwrap().unwrap().state,
            State::Started
        );
        assert!(s.start("task", "send-1", 2, &f.binding, 6).is_err());
    }
    #[test]
    fn missing_namespace_readonly_and_bad_version_never_repair() {
        let f = Fixture::new();
        let file = f.path.join(".agentz/runs.db");
        let before = std::fs::read(&file).unwrap();
        let mut r = ExternalActionStore::open_existing_readonly(&f.path).unwrap();
        assert!(r.query("task", "missing").unwrap().is_none());
        drop(r);
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let mut s = f.store();
        f.prepared(&mut s);
        drop(s);
        let c = Connection::open(&file).unwrap();
        c.execute_batch("UPDATE external_action_schema SET version=2;")
            .unwrap();
        drop(c);
        let before = std::fs::read(&file).unwrap();
        assert!(ExternalActionStore::open_existing(&f.path).is_err());
        assert_eq!(std::fs::read(file).unwrap(), before);
        let path = std::env::temp_dir().join(format!("missing-action-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        assert!(ExternalActionStore::open_existing(&path).is_err());
        assert!(!path.join(".agentz").exists());
        std::fs::remove_dir(path).unwrap();
    }
    #[test]
    fn partial_layout_and_corrupt_record_fail_closed() {
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        let c = Connection::open(f.path.join(".agentz/runs.db")).unwrap();
        c.execute("UPDATE external_actions SET record='{}'", [])
            .unwrap();
        assert!(s.query("task", "send-1").is_err());
        drop(s);
        c.execute_batch("DROP TABLE external_actions;").unwrap();
        drop(c);
        assert!(ExternalActionStore::open_existing(&f.path).is_err());
    }
    #[test]
    fn namespace_view_is_not_a_table_and_never_repaired() {
        let f = Fixture::new();
        let s = f.store();
        drop(s);
        let file = f.path.join(".agentz/runs.db");
        let c = Connection::open(&file).unwrap();
        c.execute_batch("DROP TABLE external_actions;CREATE VIEW external_actions AS SELECT 'task' AS task_id,'key' AS action_key,'{}' AS record;").unwrap();
        drop(c);
        let before = std::fs::read(&file).unwrap();
        assert!(ExternalActionStore::open_existing(&f.path).is_err());
        assert_eq!(std::fs::read(file).unwrap(), before);
    }
    #[test]
    fn audit_events_atomic_secret_free_and_existing_does_not_append() {
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        s.prepare(Fixture::scope(), &f.binding, 4).unwrap();
        let p = s.start("task", "send-1", 1, &f.binding, 4).unwrap();
        let a = s.mark_unknown(p, 5).unwrap();
        s.reconcile(
            "task",
            "send-1",
            a.revision,
            Evidence::Confirmed {
                receipt_digest: "d".repeat(64),
            },
            6,
        )
        .unwrap();
        let mut runs = RunStore::open_existing_readonly(&f.path).unwrap();
        let events = runs.replay_events("run", "a1", None, 20).unwrap();
        let audit = events
            .iter()
            .filter(|e| e.kind == "external.action")
            .collect::<Vec<_>>();
        assert_eq!(audit.len(), 4);
        for (i, e) in audit.iter().enumerate() {
            assert_eq!(e.payload["revision"], (i + 1) as i64);
            assert_eq!(e.payload.as_object().unwrap().len(), 5);
            assert_eq!(e.payload["schema_version"], 1);
            assert!(!e.payload.to_string().contains("send-1"));
            assert!(!e.payload.to_string().contains("controlled-local"));
            assert!(!e.payload.to_string().contains(&"c".repeat(64)));
            assert!(e.event_id.len() < 256);
        }
        assert_eq!(audit[0].payload["state"], "prepared");
        assert_eq!(audit[3].payload["state"], "confirmed");
    }
    #[test]
    fn event_insert_failure_rolls_back_start_and_completion() {
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        let c = Connection::open(f.path.join(".agentz/runs.db")).unwrap();
        c.execute_batch("CREATE TRIGGER reject_action_audit BEFORE INSERT ON durable_events WHEN NEW.kind='external.action' BEGIN SELECT RAISE(ABORT,'audit failure'); END;").unwrap();
        assert!(s.start("task", "send-1", 1, &f.binding, 4).is_err());
        assert_eq!(
            s.query("task", "send-1").unwrap().unwrap().state,
            State::Prepared
        );
        c.execute_batch("DROP TRIGGER reject_action_audit;")
            .unwrap();
        let p = s.start("task", "send-1", 1, &f.binding, 4).unwrap();
        c.execute_batch("CREATE TRIGGER reject_action_audit BEFORE INSERT ON durable_events WHEN NEW.kind='external.action' BEGIN SELECT RAISE(ABORT,'audit failure'); END;").unwrap();
        assert!(s
            .complete(
                p,
                Evidence::Confirmed {
                    receipt_digest: "d".repeat(64)
                },
                5
            )
            .is_err());
        let a = s.query("task", "send-1").unwrap().unwrap();
        assert_eq!(a.state, State::Started);
        assert_eq!(a.revision, 2);
        let mut runs = RunStore::open_existing_readonly(&f.path).unwrap();
        assert_eq!(
            runs.replay_events("run", "a1", None, 20)
                .unwrap()
                .iter()
                .filter(|e| e.kind == "external.action")
                .count(),
            2
        );
    }
    #[test]
    fn revision_exhaustion_and_missing_event_namespace_do_not_mutate() {
        let f = Fixture::new();
        let mut s = f.store();
        f.prepared(&mut s);
        let mut a = s.query("task", "send-1").unwrap().unwrap();
        a.revision = i64::MAX;
        let c = Connection::open(f.path.join(".agentz/runs.db")).unwrap();
        c.execute(
            "UPDATE external_actions SET record=?1",
            [serde_json::to_string(&a).unwrap()],
        )
        .unwrap();
        assert!(s.start("task", "send-1", i64::MAX, &f.binding, 4).is_err());
        assert_eq!(s.query("task", "send-1").unwrap().unwrap(), a);
        c.execute_batch("DROP TABLE durable_events;DROP TABLE durable_event_schema;")
            .unwrap();
        let db = f.path.join(".agentz/runs.db");
        c.execute_batch("PRAGMA wal_checkpoint(FULL);").unwrap();
        let before = std::fs::read(&db).unwrap();
        let mut other = Fixture::scope();
        other.action_key = "send-2".into();
        assert!(s.prepare(other, &f.binding, 4).is_err());
        assert!(s.query("task", "send-2").unwrap().is_none());
        drop(c);
        drop(s);
        let checkpoint = Connection::open(&db).unwrap();
        checkpoint
            .execute_batch("PRAGMA wal_checkpoint(FULL);")
            .unwrap();
        drop(checkpoint);
        assert_eq!(std::fs::read(&db).unwrap(), before);
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Prepared,
    Started,
    Unknown,
    Confirmed,
    Failed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub schema_version: u32,
    pub scope: Scope,
    pub origin: Binding,
    pub state: State,
    pub revision: i64,
    pub created_ms: i64,
    pub updated_ms: i64,
    pub receipt_digest: Option<String>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Prepared {
    Created(Action),
    Existing(Action),
}
/// Owned local dispatch capability; not Clone, Serialize or recoverable from a record.
pub struct StartPermit {
    action: Action,
    project: String,
}
impl StartPermit {
    /// Inspect immutable request identity; does not duplicate dispatch authorization.
    pub fn scope(&self) -> &Scope {
        &self.action.scope
    }
}
/// Trusted provider query/manual evidence. Never infer NoEffect from ordinary transport errors.
pub enum Evidence {
    Confirmed { receipt_digest: String },
    NoEffect { evidence_digest: String },
}
pub struct ExternalActionStore {
    conn: Connection,
    project: String,
}
fn err(e: impl std::fmt::Display) -> String {
    format!("external action: {e}")
}
fn token(s: &str) -> Result<(), String> {
    if s.is_empty() || s.len() > 256 || s.trim() != s || s.chars().any(char::is_control) {
        return Err(err("invalid identity"));
    }
    Ok(())
}
fn digest(s: &str) -> Result<(), String> {
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(err("invalid digest"));
    }
    Ok(())
}
fn scope(s: &Scope) -> Result<(), String> {
    for v in [&s.task_id, &s.action_key, &s.provider, &s.kind] {
        token(v)?;
    }
    digest(&s.target_digest)?;
    digest(&s.payload_digest)
}
const CREATE:&str="CREATE TABLE external_action_schema (singleton INTEGER NOT NULL PRIMARY KEY CHECK(singleton=1),version INTEGER NOT NULL);INSERT INTO external_action_schema VALUES(1,1);CREATE TABLE external_actions (task_id TEXT NOT NULL,action_key TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(task_id,action_key));";
fn layout(tx: &Transaction<'_>) -> Result<bool, String> {
    run::validate_budget_store(tx)?;
    let n:i64=tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name IN ('external_action_schema','external_actions')",[],|r|r.get(0)).map_err(err)?;
    if n == 0 {
        return Ok(false);
    }
    if n != 2 {
        return Err(err("partial namespace"));
    }
    let tables: i64 = tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('external_action_schema','external_actions')", [], |r| r.get(0)).map_err(err)?;
    if tables != 2 {
        return Err(err("namespace requires real tables"));
    }
    for (table, expected) in [
        (
            "external_action_schema",
            vec![("singleton", "INTEGER", 1, 1), ("version", "INTEGER", 1, 0)],
        ),
        (
            "external_actions",
            vec![
                ("task_id", "TEXT", 1, 1),
                ("action_key", "TEXT", 1, 2),
                ("record", "TEXT", 1, 0),
            ],
        ),
    ] {
        let mut q = tx
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(err)?;
        let actual = q
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
        let want = expected
            .into_iter()
            .map(|(a, b, c, d)| (a.to_owned(), b.to_owned(), c, d))
            .collect::<Vec<_>>();
        if actual != want {
            return Err(err("unsupported namespace layout"));
        }
    }
    let rows: Vec<(i64, i64)> = {
        let mut q = tx
            .prepare("SELECT singleton,version FROM external_action_schema")
            .map_err(err)?;
        let rows = q
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        rows
    };
    if rows != vec![(1, 1)] {
        return Err(err("unsupported namespace version"));
    }
    Ok(true)
}
fn read(
    tx: &Transaction<'_>,
    task: &str,
    key: &str,
    project: &str,
) -> Result<Option<Action>, String> {
    let raw: Option<String> = tx
        .query_row(
            "SELECT record FROM external_actions WHERE task_id=?1 AND action_key=?2",
            params![task, key],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    let Some(raw) = raw else { return Ok(None) };
    if raw.len() > 16384 {
        return Err(err("record budget"));
    }
    let a: Action = serde_json::from_str(&raw).map_err(err)?;
    scope(&a.scope)?;
    if a.schema_version != 1
        || a.scope.task_id != task
        || a.scope.action_key != key
        || a.origin.project_id != project
        || a.origin.task_id != task
        || a.revision < 1
        || a.created_ms < 0
        || a.updated_ms < a.created_ms
    {
        return Err(err("invalid record identity or revision"));
    }
    run::validate_budget_task(tx, task)?;
    run::validate_budget_origin(tx, &a.origin)?;
    if matches!(a.state, State::Confirmed | State::Failed) != a.receipt_digest.is_some() {
        return Err(err("invalid receipt state"));
    }
    if let Some(d) = &a.receipt_digest {
        digest(d)?;
    }
    Ok(Some(a))
}
fn save(tx: &Transaction<'_>, a: &Action) -> Result<(), String> {
    let changed = tx
        .execute(
            "UPDATE external_actions SET record=?1 WHERE task_id=?2 AND action_key=?3",
            params![
                serde_json::to_string(a).map_err(err)?,
                a.scope.task_id,
                a.scope.action_key
            ],
        )
        .map_err(err)?;
    if changed != 1 {
        return Err(err("action update did not persist"));
    }
    Ok(())
}
fn audit(tx: &Transaction<'_>, a: &Action) -> Result<(), String> {
    let identity = serde_json::to_vec(&(&a.scope.task_id, &a.scope.action_key)).map_err(err)?;
    let key_digest = format!("{:x}", Sha256::digest(identity));
    let scope_digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&a.scope).map_err(err)?)
    );
    events::append(
        tx,
        &events::Stream {
            task_id: a.origin.task_id.clone(),
            run_id: a.origin.run_id.clone(),
            attempt_id: a.origin.attempt_id.clone(),
            environment_id: a.origin.environment_id.clone(),
        },
        &format!("external.action.{key_digest}.{}", a.revision),
        "external.action",
        &serde_json::json!({"schema_version":1,"action_digest":key_digest,"scope_digest":scope_digest,"state":a.state,"revision":a.revision}),
        a.updated_ms,
    )?;
    Ok(())
}
impl ExternalActionStore {
    pub fn open_existing(project: &Path) -> Result<Self, String> {
        Self::open(project, OpenFlags::SQLITE_OPEN_READ_WRITE)
    }
    pub fn open_existing_readonly(project: &Path) -> Result<Self, String> {
        Self::open(project, OpenFlags::SQLITE_OPEN_READ_ONLY)
    }
    fn open(project: &Path, flags: OpenFlags) -> Result<Self, String> {
        let project = crate::canonical_root(project)?;
        let mut conn =
            Connection::open_with_flags(Path::new(&project).join(".agentz/runs.db"), flags)
                .map_err(err)?;
        crate::run::prepare_runs_connection(
            &conn,
            !flags.contains(OpenFlags::SQLITE_OPEN_READ_ONLY),
        )?;
        {
            let tx = conn.transaction().map_err(err)?;
            layout(&tx)?;
        }
        Ok(Self { conn, project })
    }
    pub fn initialize_namespace(&mut self) -> Result<(), String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        if !layout(&tx)? {
            tx.execute_batch(CREATE).map_err(err)?;
        }
        layout(&tx)?;
        tx.commit().map_err(err)
    }
    pub fn query(&mut self, task: &str, key: &str) -> Result<Option<Action>, String> {
        token(task)?;
        token(key)?;
        let tx = self.conn.transaction().map_err(err)?;
        if !layout(&tx)? {
            return Ok(None);
        }
        read(&tx, task, key, &self.project)
    }
    pub fn prepare(&mut self, s: Scope, binding: &Binding, now: i64) -> Result<Prepared, String> {
        scope(&s)?;
        binding.validate()?;
        if binding.project_id != self.project || binding.task_id != s.task_id || now < 0 {
            return Err(err("wrong scope"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        if !layout(&tx)? {
            return Err(err("namespace requires explicit initialization"));
        }
        run::validate_budget_task(&tx, &s.task_id)?;
        run::validate_budget_origin(&tx, binding)?;
        if let Some(a) = read(&tx, &s.task_id, &s.action_key, &self.project)? {
            if a.scope != s {
                return Err(err("immutable action key conflict"));
            }
            return Ok(Prepared::Existing(a));
        }
        let a = Action {
            schema_version: 1,
            scope: s,
            origin: binding.clone(),
            state: State::Prepared,
            revision: 1,
            created_ms: now,
            updated_ms: now,
            receipt_digest: None,
        };
        tx.execute(
            "INSERT INTO external_actions VALUES(?1,?2,?3)",
            params![
                a.scope.task_id,
                a.scope.action_key,
                serde_json::to_string(&a).map_err(err)?
            ],
        )
        .map_err(err)?;
        audit(&tx, &a)?;
        tx.commit().map_err(err)?;
        Ok(Prepared::Created(a))
    }
    pub fn start(
        &mut self,
        task: &str,
        key: &str,
        revision: i64,
        binding: &Binding,
        now: i64,
    ) -> Result<StartPermit, String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        if !layout(&tx)? {
            return Err(err("namespace missing"));
        }
        let mut a = read(&tx, task, key, &self.project)?.ok_or(err("action missing"))?;
        run::validate_budget_origin(&tx, binding)?;
        if binding.project_id != self.project
            || binding.task_id != task
            || binding.environment_id != a.origin.environment_id
            || a.state != State::Prepared
            || a.revision != revision
            || now < a.updated_ms
        {
            return Err(err("stale action start"));
        }
        let active: (String, String) = tx
            .query_row(
                "SELECT attempt_id,execution FROM runs WHERE run_id=?1",
                [&binding.run_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;
        if active != (binding.attempt_id.clone(), "running".into()) {
            return Err(err("start requires current Running attempt"));
        }
        a.origin = binding.clone();
        a.state = State::Started;
        a.revision = a.revision.checked_add(1).ok_or(err("revision overflow"))?;
        a.updated_ms = now;
        save(&tx, &a)?;
        audit(&tx, &a)?;
        tx.commit().map_err(err)?;
        Ok(StartPermit {
            action: a,
            project: self.project.clone(),
        })
    }
    pub fn mark_unknown(&mut self, permit: StartPermit, now: i64) -> Result<Action, String> {
        if permit.project != self.project {
            return Err(err("wrong permit project"));
        }
        self.update(
            &permit.action.scope.task_id,
            &permit.action.scope.action_key,
            permit.action.revision,
            None,
            now,
        )
    }
    pub fn complete(
        &mut self,
        permit: StartPermit,
        evidence: Evidence,
        now: i64,
    ) -> Result<Action, String> {
        if permit.project != self.project {
            return Err(err("wrong permit project"));
        }
        self.update(
            &permit.action.scope.task_id,
            &permit.action.scope.action_key,
            permit.action.revision,
            Some(evidence),
            now,
        )
    }
    /// Host must independently query/reconcile provider; this function grants no dispatch permit.
    pub fn reconcile(
        &mut self,
        task: &str,
        key: &str,
        revision: i64,
        evidence: Evidence,
        now: i64,
    ) -> Result<Action, String> {
        self.update(task, key, revision, Some(evidence), now)
    }
    fn update(
        &mut self,
        task: &str,
        key: &str,
        revision: i64,
        evidence: Option<Evidence>,
        now: i64,
    ) -> Result<Action, String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        if !layout(&tx)? {
            return Err(err("namespace missing"));
        }
        let mut a = read(&tx, task, key, &self.project)?.ok_or(err("action missing"))?;
        if !matches!(a.state, State::Started | State::Unknown)
            || a.revision != revision
            || now < a.updated_ms
        {
            return Err(err("stale reconciliation"));
        }
        let (state, receipt) = match evidence {
            None => (State::Unknown, None),
            Some(Evidence::Confirmed { receipt_digest }) => {
                digest(&receipt_digest)?;
                (State::Confirmed, Some(receipt_digest))
            }
            Some(Evidence::NoEffect { evidence_digest }) => {
                digest(&evidence_digest)?;
                (State::Failed, Some(evidence_digest))
            }
        };
        a.state = state;
        a.receipt_digest = receipt;
        a.revision = a.revision.checked_add(1).ok_or(err("revision overflow"))?;
        a.updated_ms = now;
        save(&tx, &a)?;
        audit(&tx, &a)?;
        tx.commit().map_err(err)?;
        Ok(a)
    }
}
