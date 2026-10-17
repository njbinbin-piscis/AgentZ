//! Candidate durable event primitives. The caller owns the transaction and
//! must append lifecycle events in the same transaction as their state change.
//! Nothing in this module commits, publishes, or resumes an external action.
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_PAYLOAD_BYTES: usize = 1_048_576;
const MAX_REPLAY_PAGE_BYTES: usize = 4 * MAX_PAYLOAD_BYTES;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stream {
    pub task_id: String,
    pub run_id: String,
    pub attempt_id: String,
    pub environment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub stream: Stream,
    pub sequence: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub cursor: Cursor,
    pub event_id: String,
    pub kind: String,
    pub payload: Value,
    pub created_ms: i64,
}

fn failure(error: impl std::fmt::Display) -> String {
    format!("event store: {error}")
}

fn require_schema(tx: &Transaction<'_>) -> Result<(), String> {
    let (count, version): (i64, Option<i64>) = tx
        .query_row(
            "SELECT COUNT(*),MAX(version) FROM durable_event_schema",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(failure)?;
    if count != 1 || version != Some(1) {
        return Err("event schema is unsupported or uninitialized".into());
    }
    Ok(())
}

fn identity(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err("invalid event identity".into());
    }
    Ok(())
}

impl Stream {
    fn validate(&self) -> Result<(), String> {
        for value in [
            &self.task_id,
            &self.run_id,
            &self.attempt_id,
            &self.environment_id,
        ] {
            identity(value)?;
        }
        Ok(())
    }
}

/// Initialize only the event namespace; do not change RunStore user_version.
/// The owning store must call this within its coordinated schema migration.
pub fn initialize(tx: &Transaction<'_>) -> Result<(), String> {
    tx.execute_batch("CREATE TABLE IF NOT EXISTS durable_event_schema (version INTEGER PRIMARY KEY CHECK(version > 0));").map_err(failure)?;
    let version: Option<i64> = tx
        .query_row("SELECT MAX(version) FROM durable_event_schema", [], |row| {
            row.get(0)
        })
        .map_err(failure)?;
    if version.is_some_and(|value| value != 1) {
        return Err("event schema is unsupported".into());
    }
    tx.execute_batch("CREATE TABLE IF NOT EXISTS durable_events (
        task_id TEXT NOT NULL, run_id TEXT NOT NULL, attempt_id TEXT NOT NULL, environment_id TEXT NOT NULL,
        sequence INTEGER NOT NULL CHECK(sequence > 0), event_id TEXT NOT NULL, kind TEXT NOT NULL,
        payload TEXT NOT NULL, created_ms INTEGER NOT NULL CHECK(created_ms >= 0),
        PRIMARY KEY(task_id,run_id,attempt_id,environment_id,sequence),
        UNIQUE(task_id,run_id,attempt_id,environment_id,event_id));
        INSERT OR IGNORE INTO durable_event_schema(version) VALUES(1);").map_err(failure)
}

/// Append or return an identical deduplicated record. Identity authorization
/// and association with an existing Run/Attempt are the owning store's duty.
/// Use an IMMEDIATE transaction for concurrent read-then-write producers.
pub fn append(
    tx: &Transaction<'_>,
    stream: &Stream,
    event_id: &str,
    kind: &str,
    payload: &Value,
    created_ms: i64,
) -> Result<Cursor, String> {
    require_schema(tx)?;
    stream.validate()?;
    identity(event_id)?;
    identity(kind)?;
    if created_ms < 0 {
        return Err("invalid event timestamp".into());
    }
    let encoded = serde_json::to_string(payload).map_err(failure)?;
    if encoded.len() > MAX_PAYLOAD_BYTES {
        return Err("event payload exceeds limit".into());
    }
    let previous: Option<(i64, String, String, i64)> = tx.query_row(
        "SELECT sequence,kind,payload,created_ms FROM durable_events WHERE task_id=?1 AND run_id=?2 AND attempt_id=?3 AND environment_id=?4 AND event_id=?5",
        params![stream.task_id,stream.run_id,stream.attempt_id,stream.environment_id,event_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
    ).optional().map_err(failure)?;
    if let Some((sequence, previous_kind, previous_payload, previous_time)) = previous {
        if previous_kind != kind || previous_payload != encoded || previous_time != created_ms {
            return Err("event deduplication identity reused with different content".into());
        }
        return Ok(Cursor {
            stream: stream.clone(),
            sequence,
        });
    }
    let last: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sequence),0) FROM durable_events WHERE task_id=?1 AND run_id=?2 AND attempt_id=?3 AND environment_id=?4",
        params![stream.task_id,stream.run_id,stream.attempt_id,stream.environment_id], |row| row.get(0),
    ).map_err(failure)?;
    let sequence = last.checked_add(1).ok_or("event sequence exhausted")?;
    tx.execute("INSERT INTO durable_events(task_id,run_id,attempt_id,environment_id,sequence,event_id,kind,payload,created_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![stream.task_id,stream.run_id,stream.attempt_id,stream.environment_id,sequence,event_id,kind,encoded,created_ms]).map_err(failure)?;
    Ok(Cursor {
        stream: stream.clone(),
        sequence,
    })
}

/// Snapshot replay after an exclusive scoped cursor, with a bounded page.
/// No retention/deletion is supported: a gap or future cursor fails closed.
pub fn replay(
    tx: &Transaction<'_>,
    stream: &Stream,
    after: Option<&Cursor>,
    limit: usize,
) -> Result<Vec<Event>, String> {
    require_schema(tx)?;
    stream.validate()?;
    if !(1..=1000).contains(&limit) {
        return Err("invalid event page limit".into());
    }
    let sequence = if let Some(cursor) = after {
        if &cursor.stream != stream || cursor.sequence < 0 {
            return Err("cursor belongs to another stream or is invalid".into());
        }
        cursor.sequence
    } else {
        0
    };
    let (count, last, first, non_integer): (i64, i64, i64, i64) = tx.query_row("SELECT COUNT(*),COALESCE(MAX(sequence),0),COALESCE(MIN(sequence),1),COUNT(CASE WHEN typeof(sequence)!='integer' THEN 1 END) FROM durable_events WHERE task_id=?1 AND run_id=?2 AND attempt_id=?3 AND environment_id=?4",
        params![stream.task_id,stream.run_id,stream.attempt_id,stream.environment_id], |row| Ok((row.get(0)?, row.get(1)?,row.get(2)?,row.get(3)?))).map_err(failure)?;
    if count != last || first != 1 || non_integer != 0 {
        return Err("durable event sequence gap".into());
    }
    if sequence > last {
        return Err("cursor is ahead of durable stream".into());
    }
    // Read metadata/byte lengths before allocating payload strings or parsing JSON.
    let mut statement = tx.prepare("SELECT sequence,event_id,kind,length(CAST(payload AS BLOB)),created_ms,typeof(sequence),typeof(event_id),typeof(kind),typeof(payload),typeof(created_ms) FROM durable_events WHERE task_id=?1 AND run_id=?2 AND attempt_id=?3 AND environment_id=?4 AND sequence>?5 ORDER BY sequence LIMIT ?6").map_err(failure)?;
    let mut rows = statement
        .query(params![
            stream.task_id,
            stream.run_id,
            stream.attempt_id,
            stream.environment_id,
            sequence,
            limit as i64
        ])
        .map_err(failure)?;
    let mut metadata = Vec::new();
    let mut expected = sequence;
    let mut page_bytes = 0usize;
    while let Some(row) = rows.next().map_err(failure)? {
        for (column, expected_type) in [
            (5, "integer"),
            (6, "text"),
            (7, "text"),
            (8, "text"),
            (9, "integer"),
        ] {
            if row.get::<_, String>(column).map_err(failure)? != expected_type {
                return Err("invalid persisted event storage type".into());
            }
        }
        let sequence: i64 = row.get(0).map_err(failure)?;
        expected = expected.checked_add(1).ok_or("event sequence exhausted")?;
        if sequence != expected {
            return Err("durable event sequence gap".into());
        }
        let event_id: String = row.get(1).map_err(failure)?;
        let kind: String = row.get(2).map_err(failure)?;
        identity(&event_id)?;
        identity(&kind)?;
        let created_ms: i64 = row.get(4).map_err(failure)?;
        if created_ms < 0 {
            return Err("invalid persisted event timestamp".into());
        }
        let payload_bytes = usize::try_from(row.get::<_, i64>(3).map_err(failure)?)
            .map_err(|_| "invalid persisted event payload length")?;
        if payload_bytes > MAX_PAYLOAD_BYTES {
            return Err("persisted event payload exceeds limit".into());
        }
        page_bytes = page_bytes
            .checked_add(payload_bytes)
            .ok_or("event replay page budget exhausted")?;
        if page_bytes > MAX_REPLAY_PAGE_BYTES {
            return Err("event replay page exceeds byte budget; request a smaller page".into());
        }
        metadata.push((sequence, event_id, kind, created_ms, payload_bytes));
    }
    drop(rows);
    drop(statement);
    if metadata.is_empty() {
        return Ok(Vec::new());
    }
    // Only allocate payloads once the entire bounded page passed metadata/budget checks.
    // The owning transaction keeps both queries on the same SQLite snapshot.
    let mut statement = tx.prepare("SELECT sequence,event_id,kind,payload,created_ms,typeof(sequence),typeof(event_id),typeof(kind),typeof(payload),typeof(created_ms) FROM durable_events WHERE task_id=?1 AND run_id=?2 AND attempt_id=?3 AND environment_id=?4 AND sequence>?5 AND sequence<=?6 ORDER BY sequence LIMIT ?7").map_err(failure)?;
    let mut rows = statement
        .query(params![
            stream.task_id,
            stream.run_id,
            stream.attempt_id,
            stream.environment_id,
            sequence,
            expected,
            limit as i64
        ])
        .map_err(failure)?;
    let mut events = Vec::with_capacity(metadata.len());
    for (sequence, event_id, kind, created_ms, payload_bytes) in metadata {
        let row = rows
            .next()
            .map_err(failure)?
            .ok_or("event page changed within snapshot")?;
        for (column, expected_type) in [
            (5, "integer"),
            (6, "text"),
            (7, "text"),
            (8, "text"),
            (9, "integer"),
        ] {
            if row.get::<_, String>(column).map_err(failure)? != expected_type {
                return Err("invalid persisted event storage type".into());
            }
        }
        if row.get::<_, i64>(0).map_err(failure)? != sequence
            || row.get::<_, String>(1).map_err(failure)? != event_id
            || row.get::<_, String>(2).map_err(failure)? != kind
            || row.get::<_, i64>(4).map_err(failure)? != created_ms
        {
            return Err("event page metadata changed within snapshot".into());
        }
        let payload: String = row.get(3).map_err(failure)?;
        if payload.len() != payload_bytes {
            return Err("event payload length changed within snapshot".into());
        }
        events.push(Event {
            cursor: Cursor {
                stream: stream.clone(),
                sequence,
            },
            event_id,
            kind,
            payload: serde_json::from_str(&payload).map_err(failure)?,
            created_ms,
        });
    }
    if rows.next().map_err(failure)?.is_some() {
        return Err("event page changed within snapshot".into());
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, TransactionBehavior};
    #[test]
    fn hidden_zero_and_real_sequences_reject_cursor_after_corruption() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        let tx = connection.transaction().unwrap();
        initialize(&tx).unwrap();
        for i in 1..=3 {
            append(
                &tx,
                &stream(),
                &format!("event-{i}"),
                "kind",
                &Value::Null,
                i,
            )
            .unwrap();
        }
        let after = Cursor {
            stream: stream(),
            sequence: 2,
        };
        tx.execute("UPDATE durable_events SET sequence=0 WHERE sequence=1", [])
            .unwrap();
        let (count, max): (i64, i64) = tx
            .query_row(
                "SELECT COUNT(*),MAX(sequence) FROM durable_events",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((count, max), (3, 3));
        assert!(replay(&tx, &stream(), Some(&after), 10).is_err());
        tx.execute("UPDATE durable_events SET sequence=1 WHERE sequence=0", [])
            .unwrap();
        tx.execute(
            "UPDATE durable_events SET sequence=1.5 WHERE sequence=2",
            [],
        )
        .unwrap();
        let storage_class: String = tx
            .query_row(
                "SELECT typeof(sequence) FROM durable_events WHERE sequence=1.5",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(storage_class, "real");
        assert!(replay(&tx, &stream(), Some(&after), 10).is_err());
        assert!(replay(&tx, &stream(), None, 10).is_err());
    }
    #[test]
    fn persisted_invalid_identity_timestamp_and_oversize_payload_fail_replay() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        let tx = connection.transaction().unwrap();
        initialize(&tx).unwrap();
        append(&tx, &stream(), "one", "kind", &Value::Null, 1).unwrap();
        for (column, value) in [("event_id", "bad\n"), ("kind", " ")] {
            tx.execute(&format!("UPDATE durable_events SET {column}=?1"), [value])
                .unwrap();
            assert!(replay(&tx, &stream(), None, 10).is_err());
            tx.execute("UPDATE durable_events SET event_id='one',kind='kind'", [])
                .unwrap();
        }
        tx.execute("UPDATE durable_events SET created_ms=-1", [])
            .unwrap();
        assert!(replay(&tx, &stream(), None, 10).is_err());
        tx.execute(
            "UPDATE durable_events SET created_ms=1,payload=?1",
            [format!("\"{}\"", "x".repeat(MAX_PAYLOAD_BYTES))],
        )
        .unwrap();
        assert!(replay(&tx, &stream(), None, 10).is_err());
    }
    #[test]
    fn replay_page_total_budget_errors_instead_of_silently_returning_prefix() {
        let mut connection = Connection::open_in_memory().unwrap();
        let tx = connection.transaction().unwrap();
        initialize(&tx).unwrap();
        let payload = Value::String("x".repeat(1_000_000));
        for i in 0..5 {
            append(&tx, &stream(), &format!("event-{i}"), "kind", &payload, i).unwrap();
        }
        assert_eq!(replay(&tx, &stream(), None, 4).unwrap().len(), 4);
        let error = replay(&tx, &stream(), None, 5).unwrap_err();
        assert!(error.contains("byte budget"));
        let after = Cursor {
            stream: stream(),
            sequence: 4,
        };
        assert_eq!(replay(&tx, &stream(), Some(&after), 5).unwrap().len(), 1);
    }
    #[test]
    fn page_budget_is_checked_before_parsing_any_payload() {
        let mut connection = Connection::open_in_memory().unwrap();
        let tx = connection.transaction().unwrap();
        initialize(&tx).unwrap();
        for i in 0..5 {
            append(
                &tx,
                &stream(),
                &format!("event-{i}"),
                "kind",
                &Value::String("x".repeat(1_000_000)),
                i,
            )
            .unwrap();
        }
        // Corrupt the first JSON without changing its byte count; the whole-page
        // budget must reject before even this first payload is allocated/parsed.
        tx.execute(
            "UPDATE durable_events SET payload=?1 WHERE sequence=1",
            ["!".repeat(1_000_002)],
        )
        .unwrap();
        assert!(replay(&tx, &stream(), None, 5)
            .unwrap_err()
            .contains("byte budget"));
        assert!(replay(&tx, &stream(), None, 4).is_err());
        tx.execute(
            "UPDATE durable_events SET payload=?1 WHERE sequence=1",
            [serde_json::to_string(&Value::String("x".repeat(1_000_000))).unwrap()],
        )
        .unwrap();
        assert_eq!(replay(&tx, &stream(), None, 4).unwrap().len(), 4);
    }
    #[test]
    fn persisted_blob_payload_and_real_timestamp_are_rejected_before_read() {
        let mut connection = Connection::open_in_memory().unwrap();
        let tx = connection.transaction().unwrap();
        initialize(&tx).unwrap();
        append(&tx, &stream(), "one", "kind", &Value::Null, 1).unwrap();
        tx.execute("UPDATE durable_events SET payload=CAST('null' AS BLOB)", [])
            .unwrap();
        assert!(replay(&tx, &stream(), None, 10)
            .unwrap_err()
            .contains("storage type"));
        tx.execute(
            "UPDATE durable_events SET payload='null',created_ms=1.5",
            [],
        )
        .unwrap();
        assert!(replay(&tx, &stream(), None, 10)
            .unwrap_err()
            .contains("storage type"));
        tx.execute("UPDATE durable_events SET created_ms=1", [])
            .unwrap();
        assert_eq!(
            replay(&tx, &stream(), None, 10).unwrap()[0].payload,
            Value::Null
        );
    }
    #[test]
    fn absent_or_unsupported_namespace_cannot_be_used() {
        let mut connection = Connection::open_in_memory().unwrap();
        let tx = connection.transaction().unwrap();
        assert!(append(&tx, &stream(), "one", "kind", &Value::Null, 1).is_err());
        assert!(replay(&tx, &stream(), None, 10).is_err());
        initialize(&tx).unwrap();
        tx.execute("UPDATE durable_event_schema SET version=2", [])
            .unwrap();
        assert!(initialize(&tx).is_err());
        assert!(append(&tx, &stream(), "one", "kind", &Value::Null, 1).is_err());
        assert!(replay(&tx, &stream(), None, 10).is_err());
        tx.execute("DELETE FROM durable_event_schema", []).unwrap();
        assert!(replay(&tx, &stream(), None, 10).is_err());
    }

    #[test]
    fn immediate_writers_serialize_sequence_allocation() {
        let file =
            std::env::temp_dir().join(format!("agentz-events-writers-{}.db", uuid::Uuid::new_v4()));
        {
            let mut first = Connection::open(&file).unwrap();
            let mut second = Connection::open(&file).unwrap();
            second.busy_timeout(std::time::Duration::ZERO).unwrap();
            let tx = first
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            initialize(&tx).unwrap();
            append(&tx, &stream(), "one", "kind", &Value::Null, 1).unwrap();
            assert!(second
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .is_err());
            tx.commit().unwrap();
            let tx = second
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            let cursor = append(&tx, &stream(), "two", "kind", &Value::Null, 2).unwrap();
            assert_eq!(cursor.sequence, 2);
            assert_eq!(replay(&tx, &stream(), None, 10).unwrap().len(), 2);
            tx.commit().unwrap();
        }
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn committed_events_survive_database_reopen() {
        let file = std::env::temp_dir().join(format!("agentz-events-{}.db", uuid::Uuid::new_v4()));
        {
            let mut connection = Connection::open(&file).unwrap();
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            initialize(&tx).unwrap();
            append(&tx, &stream(), "persist", "completed", &Value::Null, 1).unwrap();
            tx.commit().unwrap();
        }
        {
            let mut connection = Connection::open(&file).unwrap();
            let tx = connection.transaction().unwrap();
            assert_eq!(
                replay(&tx, &stream(), None, 10).unwrap()[0].event_id,
                "persist"
            );
        }
        std::fs::remove_file(file).unwrap();
    }
    fn stream() -> Stream {
        Stream {
            task_id: "task".into(),
            run_id: "run".into(),
            attempt_id: "attempt".into(),
            environment_id: "env".into(),
        }
    }
    #[test]
    fn rollback_preserves_state_event_atomicity_and_committed_replay() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE state(value TEXT); INSERT INTO state VALUES('queued');")
            .unwrap();
        {
            let tx = connection.transaction().unwrap();
            initialize(&tx).unwrap();
            tx.commit().unwrap();
        }
        {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            tx.execute("UPDATE state SET value='running'", []).unwrap();
            append(
                &tx,
                &stream(),
                "start",
                "running",
                &serde_json::json!({}),
                1,
            )
            .unwrap();
        }
        assert_eq!(
            connection
                .query_row("SELECT value FROM state", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "queued"
        );
        let tx = connection.transaction().unwrap();
        assert!(replay(&tx, &stream(), None, 10).unwrap().is_empty());
        tx.execute("UPDATE state SET value='running'", []).unwrap();
        append(&tx, &stream(), "start", "running", &Value::Null, 2).unwrap();
        tx.commit().unwrap();
        let tx = connection.transaction().unwrap();
        assert_eq!(replay(&tx, &stream(), None, 10).unwrap()[0].kind, "running");
    }
    #[test]
    fn dedup_paging_and_cross_attempt_cursor_rejection() {
        let mut connection = Connection::open_in_memory().unwrap();
        let tx = connection.transaction().unwrap();
        initialize(&tx).unwrap();
        let first = append(&tx, &stream(), "one", "progress", &Value::Null, 1).unwrap();
        assert_eq!(
            append(&tx, &stream(), "one", "progress", &Value::Null, 1).unwrap(),
            first
        );
        assert!(append(&tx, &stream(), "one", "progress", &Value::Bool(true), 1).is_err());
        append(&tx, &stream(), "two", "progress", &Value::Null, 2).unwrap();
        assert_eq!(replay(&tx, &stream(), None, 1).unwrap().len(), 1);
        assert_eq!(
            replay(&tx, &stream(), Some(&first), 1).unwrap()[0]
                .cursor
                .sequence,
            2
        );
        let mut other = stream();
        other.attempt_id = "other".into();
        assert!(replay(&tx, &other, Some(&first), 1).is_err());
        assert!(replay(
            &tx,
            &stream(),
            Some(&Cursor {
                stream: stream(),
                sequence: 3
            }),
            1
        )
        .is_err());
    }
    #[test]
    fn malformed_input_and_deleted_sequence_fail_closed() {
        let mut connection = Connection::open_in_memory().unwrap();
        let tx = connection.transaction().unwrap();
        initialize(&tx).unwrap();
        assert!(append(&tx, &stream(), "bad\n", "kind", &Value::Null, 1).is_err());
        assert!(append(
            &tx,
            &stream(),
            "one",
            "kind",
            &Value::String("x".repeat(MAX_PAYLOAD_BYTES)),
            1
        )
        .is_err());
        assert!(replay(&tx, &stream(), None, 0).is_err());
        append(&tx, &stream(), "one", "kind", &Value::Null, 1).unwrap();
        append(&tx, &stream(), "two", "kind", &Value::Null, 2).unwrap();
        tx.execute("DELETE FROM durable_events WHERE sequence=1", [])
            .unwrap();
        assert!(replay(
            &tx,
            &stream(),
            Some(&Cursor {
                stream: stream(),
                sequence: 1
            }),
            10
        )
        .unwrap_err()
        .contains("gap"));
        assert!(replay(&tx, &stream(), None, 10)
            .unwrap_err()
            .contains("gap"));
    }
}
