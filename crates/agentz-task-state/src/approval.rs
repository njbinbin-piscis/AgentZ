//! Host-issued, exact-action approvals. No renderer or model may issue these records.
//! This store grants no OS capability; executors must claim before their side effect.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, time::Duration};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Binding {
    pub project_id: String,
    pub task_id: String,
    pub environment_id: String,
    pub binding_version: String,
    pub run_id: String,
    pub attempt_id: String,
    pub capability_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Intent {
    pub schema_version: u32,
    pub request_id: String,
    pub boot_id: String,
    pub binding: Binding,
    pub action: String,
    pub digest: String,
    pub created_ms: i64,
    pub expires_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Approved,
    Denied,
    Expired,
    Revoked,
    Claimed,
    Executed,
    Failed,
    Unknown,
}
impl Status {
    fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
            Self::Claimed => "claimed",
            Self::Executed => "executed",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

fn identifier(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err("invalid approval identity".into());
    }
    Ok(())
}
impl Binding {
    /// Validate identity shape; this does not grant approval or authorize execution.
    pub fn validate(&self) -> Result<(), String> {
        for id in [
            &self.project_id,
            &self.task_id,
            &self.environment_id,
            &self.binding_version,
            &self.run_id,
            &self.attempt_id,
            &self.capability_version,
        ] {
            identifier(id)?;
        }
        Ok(())
    }
}

// Do not depend on serde_json's preserve_order feature. Arrays retain semantic order.
fn canonical(value: &Value) -> Result<String, String> {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<_, _> = map.iter().collect();
            let mut parts = Vec::new();
            for (key, value) in sorted {
                parts.push(format!(
                    "{}:{}",
                    serde_json::to_string(key).map_err(err)?,
                    canonical(value)?
                ));
            }
            Ok(format!("{{{}}}", parts.join(",")))
        }
        Value::Array(values) => Ok(format!(
            "[{}]",
            values
                .iter()
                .map(canonical)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        )),
        _ => serde_json::to_string(value).map_err(err),
    }
}
fn err(error: impl std::fmt::Display) -> String {
    format!("approval store: {error}")
}

/// Resource must include provider/account/surface identity and resolved targets.
/// Parameter/resource values are hashed in memory, never persisted by this store.
pub fn action_digest(
    binding: &Binding,
    action: &str,
    resource: &Value,
    parameters: &Value,
) -> Result<String, String> {
    binding.validate()?;
    identifier(action)?;
    let payload = serde_json::json!({"schema":1,"binding":binding,"action":action,
        "resource":resource,"parameters":parameters});
    Ok(format!(
        "{:x}",
        Sha256::digest(canonical(&payload)?.as_bytes())
    ))
}

pub struct ApprovalStore {
    conn: Connection,
    boot_id: String,
}
impl ApprovalStore {
    /// Path is chosen by the host project resolver, never from an approval card.
    /// A new process boot invalidates all old attempts; claimed work becomes unknown.
    pub fn open(path: &Path, boot_id: &str, now_ms: i64) -> Result<Self, String> {
        Self::open_impl(path, boot_id, now_ms, true)
    }
    pub fn open_existing(path: &Path, boot_id: &str, now_ms: i64) -> Result<Self, String> {
        Self::open_impl(path, boot_id, now_ms, false)
    }
    fn open_impl(path: &Path, boot_id: &str, now_ms: i64, create: bool) -> Result<Self, String> {
        identifier(boot_id)?;
        if now_ms < 0 {
            return Err("invalid host clock".into());
        }
        if create {
            if let Some(parent) = path.parent() {
                if !parent.is_dir() {
                    std::fs::create_dir(parent).map_err(err)?;
                }
            }
        }
        let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
            | if create {
                rusqlite::OpenFlags::SQLITE_OPEN_CREATE
            } else {
                rusqlite::OpenFlags::empty()
            };
        let mut conn = Connection::open_with_flags(path, flags).map_err(err)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(err)?;
        if version > 1 {
            return Err("unsupported approval schema; no downgrade permitted".into());
        }
        if !create && version != 1 {
            return Err("existing approval database has no supported schema".into());
        }
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        if version == 0 {
            tx.execute_batch("CREATE TABLE intents (request_id TEXT PRIMARY KEY, boot_id TEXT NOT NULL,
                record TEXT NOT NULL, status TEXT NOT NULL);
                CREATE TABLE audit (sequence INTEGER PRIMARY KEY AUTOINCREMENT, request_id TEXT NOT NULL,
                state TEXT NOT NULL, timestamp_ms INTEGER NOT NULL);
                PRAGMA user_version=1;").map_err(err)?;
        }
        tx.execute(
            "INSERT INTO audit(request_id,state,timestamp_ms)
            SELECT request_id,CASE WHEN status='claimed' THEN 'unknown' ELSE 'expired' END,?1
            FROM intents WHERE boot_id!=?2 AND status IN ('pending','approved','claimed')",
            params![now_ms, boot_id],
        )
        .map_err(err)?;
        tx.execute(
            "UPDATE intents SET status=CASE WHEN status='claimed' THEN 'unknown' ELSE 'expired' END
            WHERE boot_id!=?1 AND status IN ('pending','approved','claimed')",
            [boot_id],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Self {
            conn,
            boot_id: boot_id.into(),
        })
    }

    pub fn issue(
        &mut self,
        request_id: &str,
        binding: Binding,
        action: &str,
        resource: &Value,
        parameters: &Value,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<Intent, String> {
        identifier(request_id)?;
        if now_ms < 0 || !(1..=600_000).contains(&ttl_ms) {
            return Err("invalid approval lifetime".into());
        }
        let intent = Intent {
            schema_version: 1,
            request_id: request_id.into(),
            boot_id: self.boot_id.clone(),
            digest: action_digest(&binding, action, resource, parameters)?,
            binding,
            action: action.into(),
            created_ms: now_ms,
            expires_ms: now_ms
                .checked_add(ttl_ms)
                .ok_or("approval clock overflow")?,
        };
        let record = serde_json::to_string(&intent).map_err(err)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        tx.execute(
            "INSERT INTO intents(request_id,boot_id,record,status) VALUES(?1,?2,?3,'pending')",
            params![request_id, self.boot_id, record],
        )
        .map_err(err)?;
        audit(&tx, request_id, Status::Pending, now_ms)?;
        tx.commit().map_err(err)?;
        Ok(intent)
    }

    /// Caller must be the host's protected human UI, never a tool-provided actor string.
    pub fn decide(&mut self, expected: &Intent, approve: bool, now_ms: i64) -> Result<(), String> {
        self.transition(
            expected,
            Status::Pending,
            if approve {
                Status::Approved
            } else {
                Status::Denied
            },
            now_ms,
        )
    }

    /// A successful return is one durable consumption, not an execution result.
    /// Executor supplies freshly resolved identity/parameters, not just the old card.
    pub fn claim(
        &mut self,
        expected: &Intent,
        binding: &Binding,
        action: &str,
        resource: &Value,
        parameters: &Value,
        now_ms: i64,
    ) -> Result<(), String> {
        let mut actual = expected.clone();
        actual.binding = binding.clone();
        actual.action = action.into();
        actual.digest = action_digest(binding, action, resource, parameters)?;
        self.transition(&actual, Status::Approved, Status::Claimed, now_ms)
    }

    pub fn finish(&mut self, expected: &Intent, result: Status, now_ms: i64) -> Result<(), String> {
        if !matches!(result, Status::Executed | Status::Failed | Status::Unknown) {
            return Err("invalid approval execution result".into());
        }
        // Completion is permitted after expiry: the side effect may already have occurred.
        self.finalize(expected, result, now_ms)
    }

    pub fn revoke(&mut self, expected: &Intent, now_ms: i64) -> Result<(), String> {
        let (_, state) = self
            .get(&expected.request_id)?
            .ok_or("approval not found")?;
        if !matches!(state, Status::Pending | Status::Approved) {
            return Err(
                "approval already consumed or terminal; running work requires cancellation".into(),
            );
        }
        self.transition(expected, state, Status::Revoked, now_ms)
    }

    fn transition(
        &mut self,
        expected: &Intent,
        from: Status,
        to: Status,
        now_ms: i64,
    ) -> Result<(), String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        let (stored, status) = read(&tx, &expected.request_id)?.ok_or("approval not found")?;
        if status != from {
            return Err("approval is not in the required state".into());
        }
        let last_ms: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(timestamp_ms),?2) FROM audit WHERE request_id=?1",
                params![stored.request_id, stored.created_ms],
                |row| row.get(0),
            )
            .map_err(err)?;
        let failure =
            if stored.boot_id != self.boot_id || now_ms < last_ms || now_ms >= stored.expires_ms {
                Some((
                    Status::Expired,
                    "approval expired or belongs to an earlier process",
                ))
            } else if stored != *expected {
                Some((
                    Status::Revoked,
                    "approval scope or action changed; new approval required",
                ))
            } else {
                None
            };
        let state = failure.map(|f| f.0).unwrap_or(to);
        update(&tx, &stored.request_id, state, now_ms)?;
        tx.commit().map_err(err)?;
        if let Some((_, message)) = failure {
            Err(message.into())
        } else {
            Ok(())
        }
    }

    fn finalize(&mut self, expected: &Intent, result: Status, now_ms: i64) -> Result<(), String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        let (stored, status) = read(&tx, &expected.request_id)?.ok_or("approval not found")?;
        if status != Status::Claimed
            || stored != *expected
            || stored.boot_id != self.boot_id
            || now_ms < stored.created_ms
        {
            return Err("cannot finalize an unrelated or unclaimed approval".into());
        }
        update(&tx, &stored.request_id, result, now_ms)?;
        tx.commit().map_err(err)
    }

    pub fn get(&self, request_id: &str) -> Result<Option<(Intent, Status)>, String> {
        read(&self.conn, request_id)
    }
    pub fn history(&self, request_id: &str) -> Result<Vec<(Status, i64)>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT state,timestamp_ms FROM audit WHERE request_id=?1 ORDER BY sequence")
            .map_err(err)?;
        let rows = statement
            .query_map([request_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map_err(err)?;
        rows.map(|row| {
            let (state, time) = row.map_err(err)?;
            Ok((parse_status(&state)?, time))
        })
        .collect()
    }
}
fn parse_status(state: &str) -> Result<Status, String> {
    serde_json::from_value(Value::String(state.into())).map_err(err)
}
fn read(conn: &Connection, request_id: &str) -> Result<Option<(Intent, Status)>, String> {
    let row = conn
        .query_row(
            "SELECT record,status FROM intents WHERE request_id=?1",
            [request_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(err)?;
    row.map(|(raw, state)| {
        let intent: Intent = serde_json::from_str(&raw).map_err(err)?;
        if intent.schema_version != 1 || intent.request_id != request_id {
            return Err("corrupt approval record".into());
        }
        intent.binding.validate()?;
        Ok((intent, parse_status(&state)?))
    })
    .transpose()
}
fn audit(conn: &Connection, id: &str, status: Status, time: i64) -> Result<(), String> {
    conn.execute(
        "INSERT INTO audit(request_id,state,timestamp_ms) VALUES(?1,?2,?3)",
        params![id, status.name(), time],
    )
    .map_err(err)?;
    Ok(())
}
fn update(conn: &Connection, id: &str, status: Status, time: i64) -> Result<(), String> {
    if conn
        .execute(
            "UPDATE intents SET status=?1 WHERE request_id=?2",
            params![status.name(), id],
        )
        .map_err(err)?
        != 1
    {
        return Err("approval disappeared".into());
    }
    audit(conn, id, status, time)
}
/// Legacy response transport only. Delivery is not a grant or an execution result.
/// Keep receiver closure visible to the UI while durable executor integration lands.
pub async fn deliver_response(
    map: &std::sync::Arc<
        tokio::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    >,
    request_id: &str,
    approved: bool,
) -> Result<(), String> {
    let sender = map
        .lock()
        .await
        .remove(request_id)
        .ok_or("Permission request not found or expired")?;
    sender
        .send(approved)
        .map_err(|_| "Permission request receiver closed; decision was not delivered".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (std::path::PathBuf, ApprovalStore) {
        let path = std::env::temp_dir()
            .join(format!("agentz-approval-{}", uuid::Uuid::new_v4()))
            .join("approvals.db");
        let store = ApprovalStore::open(&path, "boot-a", 100).unwrap();
        (path, store)
    }
    fn binding() -> Binding {
        Binding {
            project_id: "project-a".into(),
            task_id: "task-a".into(),
            environment_id: "env-a".into(),
            binding_version: "root-sha-a".into(),
            run_id: "run-a".into(),
            attempt_id: "attempt-a".into(),
            capability_version: "provider-v1".into(),
        }
    }
    fn resource() -> Value {
        serde_json::json!({"provider":"fixture","account":"account-a","target":"channel-a"})
    }
    fn input() -> Value {
        serde_json::json!({"body":"test message","files":["a","b"]})
    }
    fn issue(store: &mut ApprovalStore, id: &str) -> Intent {
        store
            .issue(
                id,
                binding(),
                "message.send",
                &resource(),
                &input(),
                100,
                1000,
            )
            .unwrap()
    }
    fn claim(store: &mut ApprovalStore, intent: &Intent, now: i64) -> Result<(), String> {
        store.claim(
            intent,
            &binding(),
            "message.send",
            &resource(),
            &input(),
            now,
        )
    }

    #[test]
    fn canonical_digest_is_order_independent_but_binds_array_order_and_every_identity() {
        let left = serde_json::from_str(r#"{"a":1,"b":{"y":2,"x":3}}"#).unwrap();
        let right = serde_json::from_str(r#"{"b":{"x":3,"y":2},"a":1}"#).unwrap();
        assert_eq!(
            action_digest(&binding(), "send", &resource(), &left).unwrap(),
            action_digest(&binding(), "send", &resource(), &right).unwrap()
        );
        assert_ne!(
            action_digest(&binding(), "send", &resource(), &serde_json::json!([1, 2])).unwrap(),
            action_digest(&binding(), "send", &resource(), &serde_json::json!([2, 1])).unwrap()
        );
        let original = action_digest(&binding(), "send", &resource(), &input()).unwrap();
        for field in [
            "project_id",
            "task_id",
            "environment_id",
            "binding_version",
            "run_id",
            "attempt_id",
            "capability_version",
        ] {
            let mut json = serde_json::to_value(binding()).unwrap();
            json[field] = Value::String("changed".into());
            let changed = serde_json::from_value(json).unwrap();
            assert_ne!(
                action_digest(&changed, "send", &resource(), &input()).unwrap(),
                original,
                "{field}"
            );
        }
    }

    #[test]
    fn durable_decision_claim_and_completion_are_distinct_and_once_only() {
        let (path, mut store) = fixture();
        let intent = issue(&mut store, "request-a");
        assert!(claim(&mut store, &intent, 101).is_err());
        store.decide(&intent, true, 102).unwrap();
        drop(store);
        let mut store = ApprovalStore::open(&path, "boot-a", 103).unwrap();
        assert_eq!(store.get("request-a").unwrap().unwrap().1, Status::Approved);
        claim(&mut store, &intent, 104).unwrap();
        assert!(claim(&mut store, &intent, 105).is_err());
        assert!(store.decide(&intent, true, 105).is_err());
        store.finish(&intent, Status::Executed, 2000).unwrap();
        assert!(store.finish(&intent, Status::Failed, 2001).is_err());
        assert_eq!(
            store.history("request-a").unwrap(),
            vec![
                (Status::Pending, 100),
                (Status::Approved, 102),
                (Status::Claimed, 104),
                (Status::Executed, 2000)
            ]
        );
    }

    #[test]
    fn changed_parameters_task_environment_account_or_target_revokes_old_approval() {
        for variant in 0..6 {
            let (_, mut store) = fixture();
            let intent = issue(&mut store, "request-a");
            store.decide(&intent, true, 101).unwrap();
            let mut binding = binding();
            let mut resource = resource();
            let mut input = input();
            let mut action = "message.send";
            match variant {
                0 => input["body"] = Value::String("changed".into()),
                1 => binding.task_id = "task-b".into(),
                2 => binding.environment_id = "env-b".into(),
                3 => resource["account"] = Value::String("account-b".into()),
                4 => resource["target"] = Value::String("channel-b".into()),
                _ => action = "message.delete",
            }
            assert!(store
                .claim(&intent, &binding, action, &resource, &input, 102)
                .is_err());
            assert_eq!(store.get("request-a").unwrap().unwrap().1, Status::Revoked);
            assert!(claim(&mut store, &intent, 103).is_err());
        }
    }

    #[test]
    fn expiry_clock_regression_denial_and_revocation_never_allow_claim() {
        for variant in 0..4 {
            let (_, mut store) = fixture();
            let intent = issue(&mut store, "request-a");
            if variant == 2 {
                store.decide(&intent, false, 101).unwrap();
            } else {
                store.decide(&intent, true, 101).unwrap();
            }
            if variant == 3 {
                store.revoke(&intent, 102).unwrap();
            }
            let now = match variant {
                0 => 1100,
                1 => 99,
                _ => 103,
            };
            assert!(claim(&mut store, &intent, now).is_err());
            let status = store.get("request-a").unwrap().unwrap().1;
            assert_eq!(
                status,
                match variant {
                    0 | 1 => Status::Expired,
                    2 => Status::Denied,
                    _ => Status::Revoked,
                }
            );
        }
    }

    #[test]
    fn restart_expires_pending_and_approved_and_marks_claimed_unknown() {
        let (path, mut store) = fixture();
        let pending = issue(&mut store, "pending");
        let approved = issue(&mut store, "approved");
        store.decide(&approved, true, 101).unwrap();
        let claimed = issue(&mut store, "claimed");
        store.decide(&claimed, true, 101).unwrap();
        claim(&mut store, &claimed, 102).unwrap();
        drop(store);
        let mut restored = ApprovalStore::open(&path, "boot-b", 103).unwrap();
        for (intent, status) in [
            (&pending, Status::Expired),
            (&approved, Status::Expired),
            (&claimed, Status::Unknown),
        ] {
            assert_eq!(restored.get(&intent.request_id).unwrap().unwrap().1, status);
            assert!(claim(&mut restored, intent, 104).is_err());
            let count = restored.history(&intent.request_id).unwrap().len();
            let again = ApprovalStore::open(&path, "boot-b", 105).unwrap();
            assert_eq!(again.history(&intent.request_id).unwrap().len(), count);
        }
        assert!(restored.finish(&claimed, Status::Executed, 106).is_err());
    }

    #[test]
    fn concurrent_consumers_produce_exactly_one_durable_claim() {
        let (path, mut store) = fixture();
        let intent = issue(&mut store, "request-a");
        store.decide(&intent, true, 101).unwrap();
        drop(store);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let path = path.clone();
            let intent = intent.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                let mut store = ApprovalStore::open(&path, "boot-a", 102).unwrap();
                barrier.wait();
                claim(&mut store, &intent, 103).is_ok()
            }));
        }
        assert_eq!(
            threads
                .into_iter()
                .map(|thread| usize::from(thread.join().unwrap()))
                .sum::<usize>(),
            1
        );
        let store = ApprovalStore::open(&path, "boot-a", 104).unwrap();
        assert_eq!(
            store
                .history("request-a")
                .unwrap()
                .iter()
                .filter(|(state, _)| *state == Status::Claimed)
                .count(),
            1
        );
    }

    #[test]
    fn audit_failure_rolls_back_claim_and_no_payload_is_persisted() {
        let (path, mut store) = fixture();
        let secret = "unique-fixture-secret-not-for-storage";
        let intent = store
            .issue(
                "request-a",
                binding(),
                "send",
                &resource(),
                &serde_json::json!({"credential":secret}),
                100,
                1000,
            )
            .unwrap();
        store.decide(&intent, true, 101).unwrap();
        store
            .conn
            .execute_batch(
                "CREATE TRIGGER fail_claim BEFORE INSERT ON audit WHEN NEW.state='claimed'
            BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;",
            )
            .unwrap();
        assert!(store
            .claim(
                &intent,
                &binding(),
                "send",
                &resource(),
                &serde_json::json!({"credential":secret}),
                102
            )
            .is_err());
        assert_eq!(store.get("request-a").unwrap().unwrap().1, Status::Approved);
        assert_eq!(store.history("request-a").unwrap().len(), 2);
        drop(store);
        for entry in std::fs::read_dir(path.parent().unwrap()).unwrap() {
            let bytes = std::fs::read(entry.unwrap().path()).unwrap();
            assert!(!bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()));
        }
    }

    #[test]
    fn duplicate_ids_unknown_schema_and_corrupt_records_fail_closed() {
        let (path, mut store) = fixture();
        let intent = issue(&mut store, "request-a");
        assert!(store
            .issue(
                "request-a",
                binding(),
                "send",
                &resource(),
                &input(),
                101,
                1000
            )
            .is_err());
        assert!(store
            .decide(
                &Intent {
                    expires_ms: 5000,
                    ..intent.clone()
                },
                true,
                102
            )
            .is_err());
        assert_eq!(store.get("request-a").unwrap().unwrap().1, Status::Revoked);
        store
            .conn
            .execute("UPDATE intents SET record='not-json'", [])
            .unwrap();
        assert!(store.get("request-a").is_err());
        store.conn.pragma_update(None, "user_version", 99).unwrap();
        drop(store);
        assert!(ApprovalStore::open(&path, "boot-b", 103).is_err());
        let conn = Connection::open(&path).unwrap();
        assert_eq!(
            conn.pragma_query_value::<u32, _>(None, "user_version", |r| r.get(0))
                .unwrap(),
            99
        );
    }
    #[test]
    fn existing_database_never_initializes_a_replacement_or_recreates_a_missing_file() {
        let (path, store) = fixture();
        drop(store);
        std::fs::remove_file(&path).unwrap();
        assert!(ApprovalStore::open_existing(&path, "boot-a", 101).is_err());
        assert!(!path.exists());
        let conn = Connection::open(&path).unwrap();
        drop(conn);
        assert!(ApprovalStore::open_existing(&path, "boot-a", 102).is_err());
        let conn = Connection::open(&path).unwrap();
        assert_eq!(
            conn.pragma_query_value::<u32, _>(None, "user_version", |r| r.get(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        drop(conn);
        assert!(ApprovalStore::open(&path, "boot-a", 103).is_ok());
    }
    #[tokio::test]
    async fn legacy_delivery_reports_closed_receiver_and_never_replays() {
        use std::sync::Arc;
        use tokio::sync::{oneshot, Mutex};
        let map = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let (tx, rx) = oneshot::channel();
        map.lock().await.insert("closed".into(), tx);
        drop(rx);
        let error = deliver_response(&map, "closed", true).await.unwrap_err();
        assert!(error.contains("not delivered"));
        assert!(deliver_response(&map, "closed", true)
            .await
            .unwrap_err()
            .contains("not found"));
        for approved in [true, false] {
            let (tx, rx) = oneshot::channel();
            map.lock().await.insert("live".into(), tx);
            deliver_response(&map, "live", approved).await.unwrap();
            assert_eq!(rx.await.unwrap(), approved);
            assert!(deliver_response(&map, "live", !approved).await.is_err());
        }
    }

    #[test]
    fn clock_regression_after_decision_expires_the_grant() {
        let (_, mut store) = fixture();
        let intent = issue(&mut store, "request-a");
        store.decide(&intent, true, 200).unwrap();
        assert!(claim(&mut store, &intent, 199).is_err());
        assert_eq!(store.get("request-a").unwrap().unwrap().1, Status::Expired);
    }
}
