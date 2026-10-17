//! Read-only durable history. Caller retains an environment lease and supplies fresh host identity.
use agentz_task_state::{
    events::{Cursor, Event, Stream},
    plan::PlanBinding,
    run::RunStore,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageDto {
    pub stream: Stream,
    pub events: Vec<Event>,
    pub next_cursor: Option<Cursor>,
    pub has_more: bool,
}
pub fn replay(
    project: &Path,
    fresh: &PlanBinding,
    run_id: &str,
    attempt_id: &str,
    cursor: Option<Cursor>,
    limit: usize,
) -> Result<PageDto, String> {
    if !(1..=200).contains(&limit) {
        return Err("event replay limit must be 1..200".into());
    }
    validate_binding(project, fresh)?;
    let mut store = RunStore::open_existing_readonly(project)?;
    replay_from_store(&mut store, fresh, run_id, attempt_id, cursor, limit)
}

fn validate_binding(project: &Path, fresh: &PlanBinding) -> Result<(), String> {
    if fresh.project_id != agentz_task_state::canonical_root(project)?
        || fresh.task_id.trim().is_empty()
        || fresh.task_id.trim() != fresh.task_id
        || fresh.binding_version.len() != 64
        || !fresh.binding_version.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("invalid fresh event replay binding".into());
    }
    match agentz_task_state::load(project, &fresh.task_id)? {
        Some(environment) if environment.environment_id == fresh.environment_id => {}
        None if fresh.environment_id == format!("direct:{}", fresh.binding_version) => {}
        _ => return Err("event replay task/environment binding mismatch".into()),
    }
    Ok(())
}

pub fn current_run(
    project: &Path,
    fresh: &PlanBinding,
) -> Result<Option<agentz_task_state::run::Run>, String> {
    validate_binding(project, fresh)?;
    if !project
        .join(".agentz/runs.db")
        .try_exists()
        .map_err(|error| format!("cannot inspect current Run database: {error}"))?
    {
        return Ok(None);
    }
    let mut store = RunStore::open_existing_readonly(project)?;
    store.current_task_run(&fresh.task_id, &fresh.environment_id)
}

fn replay_from_store(
    store: &mut RunStore,
    fresh: &PlanBinding,
    run_id: &str,
    attempt_id: &str,
    cursor: Option<Cursor>,
    limit: usize,
) -> Result<PageDto, String> {
    let run = store.load(run_id)?.ok_or("event replay Run not found")?;
    if run.task_id != fresh.task_id || run.environment_id != fresh.environment_id {
        return Err("event replay Run task/environment mismatch".into());
    }
    // Validate same-task Run/Attempt invariants, without rejecting a healthy live Run.
    // Historical replay must not silently accept corrupt current lifecycle metadata.
    let _active = store.has_active_task(&fresh.task_id)?;
    if !store
        .attempts(run_id)?
        .iter()
        .any(|attempt| attempt.attempt_id == attempt_id)
    {
        return Err("event replay Attempt does not belong to Run".into());
    }
    let stream = Stream {
        task_id: run.task_id,
        environment_id: run.environment_id,
        run_id: run_id.into(),
        attempt_id: attempt_id.into(),
    };
    if cursor
        .as_ref()
        .is_some_and(|c| c.stream != stream || c.sequence < 0)
    {
        return Err("event replay cursor belongs to another stream".into());
    }
    let mut events = store.replay_events(run_id, attempt_id, cursor.as_ref(), limit + 1)?;
    for event in &events {
        if event.cursor.stream != stream
            || event.event_id.trim().is_empty()
            || event.kind.trim().is_empty()
            || event.created_ms < 0
        {
            return Err("invalid durable event record".into());
        }
    }
    let has_more = events.len() > limit;
    events.truncate(limit);
    let next_cursor = events.last().map(|e| e.cursor.clone()).or(cursor);
    Ok(PageDto {
        stream,
        events,
        next_cursor,
        has_more,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentz_task_state::run::Execution;
    struct Directory(std::path::PathBuf);
    #[test]
    fn corrupt_current_metadata_rejects_prior_attempt_without_repair_or_rejecting_healthy_live_run()
    {
        for corruption in [
            "UPDATE runs SET delivery='accepted' WHERE run_id='run'",
            "DELETE FROM attempts WHERE attempt_id='a2'",
        ] {
            let (d, b) = fixture();
            let path = d.0.join(".agentz/runs.db");
            // Current a2 is queued while a1 remains healthy historical data.
            assert_eq!(
                replay(&d.0, &b, "run", "a1", None, 10)
                    .unwrap()
                    .events
                    .len(),
                3
            );
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(corruption).unwrap();
            drop(conn);
            let before = std::fs::read(&path).unwrap();
            assert!(
                replay(&d.0, &b, "run", "a1", None, 10).is_err(),
                "{corruption}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }
    #[test]
    fn missing_store_is_not_created_and_future_schema_or_readonly_writes_fail() {
        let missing = Directory(
            std::env::temp_dir().join(format!("agentz-replay-missing-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir_all(&missing.0).unwrap();
        let b = PlanBinding {
            project_id: agentz_task_state::canonical_root(&missing.0).unwrap(),
            task_id: "task".into(),
            environment_id: format!("direct:{}", "a".repeat(64)),
            binding_version: "a".repeat(64),
        };
        assert!(replay(&missing.0, &b, "run", "a1", None, 1).is_err());
        assert!(current_run(&missing.0, &b).unwrap().is_none());
        assert!(!missing.0.join(".agentz/runs.db").exists());
        assert!(!missing.0.join(".agentz").exists());
        let (discovery, binding) = fixture();
        let discovery_path = discovery.0.join(".agentz/runs.db");
        let discovery_before = std::fs::read(&discovery_path).unwrap();
        let latest = current_run(&discovery.0, &binding).unwrap().unwrap();
        assert_eq!(latest.run_id, "run");
        assert_eq!(latest.attempt_id, "a2");
        assert_eq!(std::fs::read(&discovery_path).unwrap(), discovery_before);
        let (d, _) = fixture();
        let path = d.0.join(".agentz/runs.db");
        let before = std::fs::read(&path).unwrap();
        let mut readonly = RunStore::open_existing_readonly(&d.0).unwrap();
        assert!(readonly
            .transition("run", "a2", Execution::Queued, Execution::Running, 5)
            .is_err());
        drop(readonly);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        drop(conn);
        let before = std::fs::read(&path).unwrap();
        assert!(RunStore::open_existing_readonly(&d.0).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Directory, PlanBinding) {
        let directory = Directory(
            std::env::temp_dir().join(format!("agentz-event-replay-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir_all(&directory.0).unwrap();
        let project = agentz_task_state::canonical_root(&directory.0).unwrap();
        agentz_task_state::register(
            &directory.0,
            &agentz_task_state::TaskEnvironment {
                schema_version: 1,
                task_id: "task".into(),
                environment_id: "environment".into(),
                project_root: project.clone(),
                workspace_root: project.clone(),
                kind: "local".into(),
                branch: None,
                base_ref: String::new(),
                base_sha: String::new(),
                created_at: "fixture".into(),
            },
        )
        .unwrap();
        let mut store = RunStore::open(&directory.0).unwrap();
        store.create("run", "task", "environment", "a1", 1).unwrap();
        store
            .transition("run", "a1", Execution::Queued, Execution::Running, 2)
            .unwrap();
        store
            .transition("run", "a1", Execution::Running, Execution::Failed, 3)
            .unwrap();
        store.retry("run", "a1", "a2", 4).unwrap();
        drop(store);
        (
            directory,
            PlanBinding {
                project_id: project,
                task_id: "task".into(),
                environment_id: "environment".into(),
                binding_version: "a".repeat(64),
            },
        )
    }
    #[test]
    fn reopened_pages_prior_attempt_without_writing_or_inventing_events() {
        let (d, b) = fixture();
        let path = d.0.join(".agentz/runs.db");
        let before = std::fs::read(&path).unwrap();
        let first = replay(&d.0, &b, "run", "a1", None, 1).unwrap();
        assert_eq!(first.events.len(), 1);
        assert!(first.has_more);
        assert_eq!(first.next_cursor.as_ref().unwrap().sequence, 1);
        let second = replay(&d.0, &b, "run", "a1", first.next_cursor, 1).unwrap();
        assert_eq!(second.events[0].cursor.sequence, 2);
        assert!(second.has_more);
        let last = replay(&d.0, &b, "run", "a1", second.next_cursor, 1).unwrap();
        assert_eq!(last.events[0].cursor.sequence, 3);
        assert!(!last.has_more);
        let empty = replay(&d.0, &b, "run", "a1", last.next_cursor.clone(), 1).unwrap();
        assert!(empty.events.is_empty());
        assert_eq!(empty.next_cursor, last.next_cursor);
        assert!(!empty.has_more);
        let retried = replay(&d.0, &b, "run", "a2", None, 200).unwrap();
        assert_eq!(retried.events.len(), 1);
        assert_eq!(retried.events[0].cursor.sequence, 1);
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
    #[test]
    fn wrong_task_environment_attempt_cursor_limits_and_future_cursor_are_rejected() {
        let (d, b) = fixture();
        assert!(replay(&d.0, &b, "run", "a1", None, 0).is_err());
        assert!(replay(&d.0, &b, "run", "a1", None, 201).is_err());
        let mut other = b.clone();
        other.task_id = "other".into();
        assert!(replay(&d.0, &other, "run", "a1", None, 1).is_err());
        other = b.clone();
        other.environment_id = "other".into();
        assert!(replay(&d.0, &other, "run", "a1", None, 1).is_err());
        assert!(replay(&d.0, &b, "run", "wrong-attempt", None, 1).is_err());
        let page = replay(&d.0, &b, "run", "a1", None, 1).unwrap();
        let mut cursor = page.next_cursor.unwrap();
        cursor.stream.attempt_id = "a2".into();
        assert!(replay(&d.0, &b, "run", "a1", Some(cursor), 1).is_err());
        let cursor = Cursor {
            stream: page.stream,
            sequence: 999,
        };
        assert!(replay(&d.0, &b, "run", "a1", Some(cursor), 1).is_err());
    }
    #[test]
    fn legacy_empty_history_remains_read_only_and_partial_namespace_is_corrupt() {
        let (d, b) = fixture();
        let path = d.0.join(".agentz/runs.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch("DROP TABLE durable_events; DROP TABLE durable_event_schema;")
            .unwrap();
        drop(connection);
        let before = std::fs::read(&path).unwrap();
        let page = replay(&d.0, &b, "run", "a1", None, 10).unwrap();
        assert!(page.events.is_empty());
        assert!(page.next_cursor.is_none());
        assert!(!page.has_more);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(replay(
            &d.0,
            &b,
            "run",
            "a1",
            Some(Cursor {
                stream: page.stream,
                sequence: 1
            }),
            1
        )
        .is_err());
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE durable_event_schema(version INTEGER PRIMARY KEY); INSERT INTO durable_event_schema VALUES(1);").unwrap();
        drop(connection);
        assert!(replay(&d.0, &b, "run", "a1", None, 1).is_err());
    }
    #[test]
    fn corrupt_event_payload_is_explicit_error() {
        let (d, b) = fixture();
        let connection = rusqlite::Connection::open(d.0.join(".agentz/runs.db")).unwrap();
        connection.execute("UPDATE durable_events SET payload='malformed' WHERE attempt_id='a1' AND sequence=1",[]).unwrap();
        drop(connection);
        assert!(replay(&d.0, &b, "run", "a1", None, 1).is_err());
    }
}
