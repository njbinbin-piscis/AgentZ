//! Independent reservation candidate. No host/client is wired to consume these permits.
use crate::{
    approval::Binding,
    budget::{self, Money, Snapshot, Usage},
    run,
};
use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub revision: i64,
    pub max_total_tokens: Option<i64>,
    pub max_active_ms: Option<i64>,
    pub max_cost: Option<Money>,
    pub max_in_flight: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub total_tokens: Option<i64>,
    pub active_ms: Option<i64>,
    pub cost: Option<Money>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Reserved,
    Started,
    Unknown,
    Settled,
    ReleasedUnstarted,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reservation {
    pub binding: Binding,
    pub member_id: String,
    pub operation_id: String,
    pub boot_id: String,
    pub kind: String,
    pub intent_digest: String,
    pub claim_digest: String,
    pub policy_revision: i64,
    pub resources: Resources,
    pub status: Status,
    pub usage_id: Option<String>,
    pub reason: Option<String>,
}
pub struct Request {
    pub reservation: Reservation,
    pub claim: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ReserveOutcome {
    Created,
    Replay(Status),
}
/// Non-Clone local result of a single database CAS; never an OS/tool capability.
#[derive(Debug)]
pub struct StartedPermit {
    operation_id: String,
    binding: Binding,
}
impl StartedPermit {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
}
#[derive(Debug)]
pub enum StartOutcome {
    Granted(StartedPermit),
    Replay(Status),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReservationSnapshot {
    pub usage: Snapshot,
    pub policy: Policy,
    pub outstanding: Vec<Reservation>,
    pub in_flight: i64,
    pub unknown_operations: i64,
}
fn error(value: impl std::fmt::Display) -> String {
    format!("reservation store: {value}")
}
fn id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err("invalid reservation identity".into())
    } else {
        Ok(())
    }
}
fn nonnegative(value: Option<i64>) -> Result<(), String> {
    if value.is_some_and(|value| value < 0) {
        Err("negative reservation resource".into())
    } else {
        Ok(())
    }
}
fn money(value: &Money) -> Result<(), String> {
    if value.currency.len() != 3
        || !value
            .currency
            .bytes()
            .all(|value| value.is_ascii_uppercase())
    {
        return Err("invalid reservation currency".into());
    }
    nonnegative(Some(value.micro_units))
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn digest(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Err("invalid reservation digest".into())
    } else {
        Ok(())
    }
}
fn policy(value: &Policy) -> Result<(), String> {
    if value.revision < 1 || value.max_in_flight < 1 {
        return Err("invalid reservation policy".into());
    }
    nonnegative(value.max_total_tokens)?;
    nonnegative(value.max_active_ms)?;
    if let Some(value) = &value.max_cost {
        money(value)?;
    }
    Ok(())
}
fn validate(value: &Reservation, project: &str, task: &str) -> Result<(), String> {
    value.binding.validate()?;
    for value in [
        &value.member_id,
        &value.operation_id,
        &value.boot_id,
        &value.kind,
    ] {
        id(value)?;
    }
    digest(&value.intent_digest)?;
    digest(&value.claim_digest)?;
    if value.policy_revision < 1
        || value.binding.project_id != project
        || value.binding.task_id != task
        || value.member_id != task
    {
        return Err("external member or cross-task reservation unsupported".into());
    }
    nonnegative(value.resources.total_tokens)?;
    nonnegative(value.resources.active_ms)?;
    if let Some(value) = &value.resources.cost {
        money(value)?;
    }
    if let Some(value) = &value.usage_id {
        id(value)?;
    }
    if let Some(value) = &value.reason {
        id(value)?;
    }
    if (value.status == Status::Settled) != value.usage_id.is_some() {
        return Err("reservation settlement identity inconsistent".into());
    }
    if matches!(value.status, Status::Unknown | Status::ReleasedUnstarted) && value.reason.is_none()
    {
        return Err("reservation failure reason missing".into());
    }
    Ok(())
}
fn existing(tx: &Transaction<'_>) -> Result<bool, String> {
    if !budget::namespace(tx)? {
        return Err("observational budget namespace must exist first".into());
    }
    let count:i64=tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('resource_reservation_schema','resource_reservation_policy','resource_reservations')",[],|r|r.get(0)).map_err(error)?;
    if count == 0 {
        return Ok(false);
    }
    if count != 3 {
        return Err("partial reservation namespace".into());
    }
    let (count,version,invalid):(i64,Option<i64>,i64)=tx.query_row("SELECT COUNT(*),MAX(version),COUNT(CASE WHEN typeof(version)!='integer' THEN 1 END) FROM resource_reservation_schema",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(error)?;
    if count != 1 || version != Some(1) || invalid != 0 {
        return Err("unsupported reservation schema".into());
    }
    for (table, columns) in [
        (
            "resource_reservation_schema",
            vec![("version", "INTEGER", 1)],
        ),
        (
            "resource_reservation_policy",
            vec![("task_id", "TEXT", 1), ("record", "TEXT", 0)],
        ),
        (
            "resource_reservations",
            vec![
                ("task_id", "TEXT", 1),
                ("operation_id", "TEXT", 2),
                ("record", "TEXT", 0),
            ],
        ),
    ] {
        let mut statement = tx
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(error)?;
        let actual = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        let wanted = columns
            .into_iter()
            .map(|(name, kind, key)| (name.to_owned(), kind.to_owned(), 1, key))
            .collect::<Vec<_>>();
        if actual != wanted {
            return Err("unsupported reservation table layout".into());
        }
    }
    Ok(true)
}
fn read_policy(tx: &Transaction<'_>, task: &str) -> Result<Option<Policy>, String> {
    let row:Option<(String,String,String)>=tx.query_row("SELECT record,typeof(task_id),typeof(record) FROM resource_reservation_policy WHERE task_id=?1",[task],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(error)?;
    row.map(|(record, key_type, record_type)| {
        if key_type != "text" || record_type != "text" || record.len() > 65536 {
            return Err("invalid reservation policy storage".into());
        }
        let value: Policy = serde_json::from_str(&record).map_err(error)?;
        policy(&value)?;
        Ok(value)
    })
    .transpose()
}
fn overshoot(record: &Reservation, usage: &Usage) -> Result<bool, String> {
    let tokens = match (
        usage.measurement.input_tokens,
        usage.measurement.output_tokens,
        record.resources.total_tokens,
    ) {
        (Some(input), Some(output), Some(bound)) => total(input, output)? > bound,
        _ => false,
    };
    let duration = match (usage.measurement.active_ms, record.resources.active_ms) {
        (Some(actual), Some(bound)) => actual > bound,
        _ => false,
    };
    let cost = match (&usage.measurement.cost, &record.resources.cost) {
        (Some(actual), Some(bound)) => {
            actual.currency == bound.currency && actual.micro_units > bound.micro_units
        }
        _ => false,
    };
    Ok(tokens || duration || cost)
}
fn records(tx: &Transaction<'_>, project: &str, task: &str) -> Result<Vec<Reservation>, String> {
    let configured = read_policy(tx, task)?;
    let mut query=tx.prepare("SELECT operation_id,record,typeof(task_id),typeof(operation_id),typeof(record) FROM resource_reservations WHERE task_id=?1 ORDER BY rowid").map_err(error)?;
    let mut rows = query.query([task]).map_err(error)?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().map_err(error)? {
        for index in 2..=4 {
            if row.get::<_, String>(index).map_err(error)? != "text" {
                return Err("invalid reservation record storage".into());
            }
        }
        let record: String = row.get(1).map_err(error)?;
        if record.len() > 65536 {
            return Err("reservation record exceeds limit".into());
        }
        let value: Reservation = serde_json::from_str(&record).map_err(error)?;
        validate(&value, project, task)?;
        // Historical records stay valid after a revision. A record from a future revision is not.
        let current = configured
            .as_ref()
            .map(|p| p.revision)
            .ok_or("reservation policy missing")?;
        if value.policy_revision < 1 || value.policy_revision > current {
            return Err("reservation policy missing or revision mismatch".into());
        }
        if value.operation_id != row.get::<_, String>(0).map_err(error)? {
            return Err("reservation key mismatch".into());
        }
        run::validate_budget_origin(tx, &value.binding)?;
        let usage: Option<String> = tx
            .query_row(
                "SELECT usage_id FROM resource_budget_usage WHERE task_id=?1 AND operation_id=?2",
                params![task, value.operation_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        if usage != value.usage_id {
            return Err("reservation and usage settlement mismatch".into());
        }
        if value.status == Status::Settled {
            let encoded: String = tx
                .query_row(
                    "SELECT record FROM resource_budget_usage WHERE task_id=?1 AND operation_id=?2",
                    params![task, value.operation_id],
                    |row| row.get(0),
                )
                .map_err(error)?;
            if encoded.len() > 65536 {
                return Err("settlement record exceeds limit".into());
            }
            let settled: Usage = serde_json::from_str(&encoded).map_err(error)?;
            budget::validate_usage(&settled, project, task)?;
            if settled.binding != value.binding
                || settled.member_id != value.member_id
                || settled.operation_id != value.operation_id
                || Some(&settled.usage_id) != value.usage_id.as_ref()
            {
                return Err("settlement captured identity mismatch".into());
            }
            if overshoot(&value, &settled)?
                != (value.reason.as_deref() == Some("settlement exceeded reservation"))
            {
                return Err("settlement overshoot metadata is inconsistent".into());
            }
        }
        result.push(value);
    }
    Ok(result)
}
fn current(tx: &Transaction<'_>, binding: &Binding) -> Result<(), String> {
    run::validate_budget_origin(tx, binding)?;
    let (attempt, execution, delivery): (String, String, String) = tx
        .query_row(
            "SELECT attempt_id,execution,delivery FROM runs WHERE run_id=?1",
            [&binding.run_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(error)?;
    if attempt != binding.attempt_id || execution != "running" || delivery != "not_ready" {
        return Err("reservation requires current Running undelivered Attempt".into());
    }
    Ok(())
}
fn total(a: i64, b: i64) -> Result<i64, String> {
    a.checked_add(b)
        .ok_or("reservation integer overflow".into())
}
/// Resolve reservations left by an earlier process. Only records whose run is no longer
/// `running` are touched, so a live process is never mistaken for a crashed one.
/// An unstarted call is released. A started call is charged at its reserved estimate
/// (conservative) and recorded as crash recovery. Returns the refreshed records.
fn reconcile_orphans(
    tx: &Transaction<'_>,
    project: &str,
    task: &str,
    boot: &str,
) -> Result<Vec<Reservation>, String> {
    let mut all = records(tx, project, task)?;
    for record in all.iter_mut() {
        if record.boot_id == boot || !matches!(record.status, Status::Reserved | Status::Started) {
            continue;
        }
        let execution: String = tx
            .query_row(
                "SELECT execution FROM runs WHERE run_id=?1",
                [&record.binding.run_id],
                |r| r.get(0),
            )
            .map_err(error)?;
        if execution == "running" {
            continue;
        }
        if record.status == Status::Reserved {
            record.status = Status::ReleasedUnstarted;
            record.reason = Some("recovered after restart: never started".into());
        } else {
            let usage_id = format!("recovered-{}", record.operation_id);
            let created_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "invalid recovery clock")?
                .as_millis();
            let usage = Usage {
                project_id: project.into(),
                root_task_id: task.into(),
                member_id: record.member_id.clone(),
                usage_id: usage_id.clone(),
                operation_id: record.operation_id.clone(),
                binding: record.binding.clone(),
                measurement: budget::Measurement {
                    input_tokens: record.resources.total_tokens,
                    output_tokens: record.resources.total_tokens.map(|_| 0),
                    active_ms: record.resources.active_ms,
                    cost: None,
                    source: "crash-recovery-reserved-estimate".into(),
                    price_source: None,
                },
                created_ms: i64::try_from(created_ms).map_err(|_| "recovery clock overflow")?,
            };
            tx.execute(
                "INSERT INTO resource_budget_usage VALUES(?1,?2,?3,?4)",
                params![
                    task,
                    usage_id,
                    record.operation_id,
                    serde_json::to_string(&usage).map_err(error)?
                ],
            )
            .map_err(error)?;
            record.status = Status::Settled;
            record.usage_id = Some(usage_id);
            record.reason = Some("recovered after restart: settled at reserved estimate".into());
        }
        save(tx, task, record)?;
    }
    Ok(all)
}
fn check_budget(
    snapshot: &Snapshot,
    policy: &Policy,
    records: &[Reservation],
    proposed: &Resources,
    boot: &str,
) -> Result<(), String> {
    // An overshoot blocks reservations only while the policy it happened under still enforces a
    // limit, and only under that revision. Raising the limit (a new revision) acknowledges it.
    let enforces_limit = policy.max_total_tokens.is_some()
        || policy.max_active_ms.is_some()
        || policy.max_cost.is_some();
    if enforces_limit
        && records.iter().any(|r| {
            r.status == Status::Settled
                && r.policy_revision == policy.revision
                && r.reason.as_deref() == Some("settlement exceeded reservation")
        })
    {
        return Err("previous settlement exceeded reservation; reconciliation required".into());
    }
    // Unlimited policies enforce nothing, so in-flight or cross-boot records cannot block them.
    if !enforces_limit {
        return Ok(());
    }
    let mut tokens = total(snapshot.known_input_tokens, snapshot.known_output_tokens)?;
    let mut active = snapshot.known_active_ms;
    let mut cost = 0;
    let mut slots = 0;
    if let Some(limit) = &policy.max_cost {
        if snapshot.unknown_price_records != 0
            || snapshot
                .known_cost_micro_units
                .keys()
                .any(|currency| currency != &limit.currency)
        {
            return Err("price or currency is unknown for strict cost budget".into());
        }
        cost = snapshot
            .known_cost_micro_units
            .get(&limit.currency)
            .copied()
            .unwrap_or(0);
    }
    if policy.max_total_tokens.is_some() && snapshot.unknown_token_records != 0 {
        return Err("historical token usage is unknown".into());
    }
    if policy.max_active_ms.is_some() && snapshot.unknown_duration_records != 0 {
        return Err("historical duration is unknown".into());
    }
    for resources in records
        .iter()
        .filter(|r| {
            matches!(
                r.status,
                Status::Reserved | Status::Started | Status::Unknown
            )
        })
        .map(|r| {
            if r.status == Status::Unknown || (r.status == Status::Started && r.boot_id != boot) {
                return Err("unsettled previous execution requires reconciliation".to_string());
            }
            Ok(&r.resources)
        })
        .chain(std::iter::once(Ok(proposed)))
    {
        let resources = resources?;
        slots = total(slots, 1)?;
        if policy.max_total_tokens.is_some() {
            tokens = total(
                tokens,
                resources
                    .total_tokens
                    .ok_or("token upper bound is unknown")?,
            )?;
        }
        if policy.max_active_ms.is_some() {
            active = total(
                active,
                resources
                    .active_ms
                    .ok_or("duration upper bound is unknown")?,
            )?;
        }
        if let Some(limit) = &policy.max_cost {
            let value = resources
                .cost
                .as_ref()
                .ok_or("price upper bound is unknown")?;
            if value.currency != limit.currency {
                return Err("reservation currency mismatch".into());
            }
            cost = total(cost, value.micro_units)?;
        }
    }
    if slots > policy.max_in_flight
        || policy.max_total_tokens.is_some_and(|limit| tokens > limit)
        || policy.max_active_ms.is_some_and(|limit| active > limit)
        || policy
            .max_cost
            .as_ref()
            .is_some_and(|limit| cost > limit.micro_units)
    {
        return Err("task resource budget exhausted".into());
    }
    Ok(())
}
fn save(tx: &Transaction<'_>, task: &str, value: &Reservation) -> Result<(), String> {
    let record = serde_json::to_string(value).map_err(error)?;
    if record.len() > 65536 {
        return Err("reservation record exceeds limit".into());
    }
    let count = tx
        .execute(
            "UPDATE resource_reservations SET record=?3 WHERE task_id=?1 AND operation_id=?2",
            params![task, value.operation_id, record],
        )
        .map_err(error)?;
    if count != 1 {
        return Err("reservation missing".into());
    }
    Ok(())
}

pub struct ReservationStore {
    conn: Connection,
    project: String,
}
impl ReservationStore {
    pub fn open_existing(project: &Path, readonly: bool) -> Result<Self, String> {
        let project = crate::canonical_root(project)?;
        let mut conn = Connection::open_with_flags(
            Path::new(&project).join(".agentz/runs.db"),
            if readonly {
                OpenFlags::SQLITE_OPEN_READ_ONLY
            } else {
                OpenFlags::SQLITE_OPEN_READ_WRITE
            },
        )
        .map_err(error)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(error)?;
        {
            let tx = conn.transaction().map_err(error)?;
            run::validate_budget_store(&tx)?;
        }
        Ok(Self { conn, project })
    }
    pub fn initialize(&mut self) -> Result<(), String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !existing(&tx)? {
            tx.execute_batch("CREATE TABLE resource_reservation_schema(version INTEGER PRIMARY KEY NOT NULL);INSERT INTO resource_reservation_schema VALUES(1);CREATE TABLE resource_reservation_policy(task_id TEXT PRIMARY KEY NOT NULL,record TEXT NOT NULL);CREATE TABLE resource_reservations(task_id TEXT NOT NULL,operation_id TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(task_id,operation_id));").map_err(error)?;
            existing(&tx)?;
        }
        tx.commit().map_err(error)
    }
    pub fn configure(&mut self, task: &str, value: Policy) -> Result<(), String> {
        id(task)?;
        policy(&value)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !existing(&tx)? {
            return Err("reservation namespace not initialized".into());
        }
        run::validate_budget_task(&tx, task)?;
        if budget::snapshot(&tx, &self.project, task)?.is_none() {
            return Err("observational task budget required".into());
        }
        if let Some(old) = read_policy(&tx, task)? {
            if old != value {
                return Err("reservation policy is immutable".into());
            }
        } else {
            tx.execute(
                "INSERT INTO resource_reservation_policy VALUES(?1,?2)",
                params![task, serde_json::to_string(&value).map_err(error)?],
            )
            .map_err(error)?;
        }
        records(&tx, &self.project, task)?;
        tx.commit().map_err(error)
    }
    /// Advance the task's policy to `expected + 1`, updating the budget configuration and the
    /// reservation policy in one transaction. Stale or skipped revisions are refused.
    pub fn revise_policy(&mut self, task: &str, expected: i64, value: Policy) -> Result<(), String> {
        id(task)?;
        policy(&value)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !existing(&tx)? {
            return Err("reservation namespace not initialized".into());
        }
        run::validate_budget_task(&tx, task)?;
        let current = read_policy(&tx, task)?.ok_or("reservation policy missing")?;
        if current.revision != expected {
            return Err("stale reservation policy revision".into());
        }
        budget::revise_in_tx(
            &tx,
            &self.project,
            task,
            expected,
            budget::Policy {
                revision: value.revision,
                max_total_tokens: value.max_total_tokens,
                max_active_ms: value.max_active_ms,
                max_cost: value.max_cost.clone(),
            },
        )?;
        tx.execute(
            "UPDATE resource_reservation_policy SET record=?2 WHERE task_id=?1",
            params![task, serde_json::to_string(&value).map_err(error)?],
        )
        .map_err(error)?;
        records(&tx, &self.project, task)?;
        tx.commit().map_err(error)
    }
    pub fn reserve(&mut self, request: Request) -> Result<ReserveOutcome, String> {
        let mut desired = request.reservation;
        id(&request.claim)?;
        desired.claim_digest = hash(&request.claim);
        validate(&desired, &self.project, &desired.binding.task_id)?;
        if desired.status != Status::Reserved
            || desired.usage_id.is_some()
            || desired.reason.is_some()
        {
            return Err("new reservation must be Reserved without settlement".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !existing(&tx)? {
            return Err("reservation namespace not initialized".into());
        }
        current(&tx, &desired.binding)?;
        let task = &desired.binding.task_id;
        let p = read_policy(&tx, task)?.ok_or("reservation policy missing")?;
        if p.revision != desired.policy_revision {
            return Err("stale reservation policy revision".into());
        }
        let stored = reconcile_orphans(&tx, &self.project, task, &desired.boot_id)?;
        if let Some(old) = stored
            .iter()
            .find(|r| r.operation_id == desired.operation_id)
        {
            let mut comparable = old.clone();
            comparable.status = Status::Reserved;
            comparable.usage_id = None;
            comparable.reason = None;
            if comparable != desired {
                return Err("reservation identity reused with different intent".into());
            }
            return Ok(ReserveOutcome::Replay(old.status));
        }
        let occupied: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM resource_budget_usage WHERE task_id=?1 AND operation_id=?2)",
            params![task, desired.operation_id],
            |row| row.get(0),
        ).map_err(error)?;
        if occupied {
            return Err(
                "operation already has recorded usage; cannot reserve execution again".into(),
            );
        }
        let usage = budget::snapshot(&tx, &self.project, task)?.ok_or("task budget missing")?;
        check_budget(&usage, &p, &stored, &desired.resources, &desired.boot_id)
            .map_err(|e| format!("budget denied: {e}"))?;
        tx.execute(
            "INSERT INTO resource_reservations VALUES(?1,?2,?3)",
            params![
                task,
                desired.operation_id,
                serde_json::to_string(&desired).map_err(error)?
            ],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(ReserveOutcome::Created)
    }
    pub fn start(
        &mut self,
        binding: &Binding,
        operation: &str,
        claim: &str,
        boot: &str,
    ) -> Result<StartOutcome, String> {
        id(operation)?;
        id(claim)?;
        id(boot)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !existing(&tx)? {
            return Err("reservation namespace missing".into());
        }
        current(&tx, binding)?;
        let mut record = records(&tx, &self.project, &binding.task_id)?
            .into_iter()
            .find(|r| r.operation_id == operation)
            .ok_or("reservation missing")?;
        if record.binding != *binding
            || record.claim_digest != hash(claim)
            || record.boot_id != boot
        {
            return Err("reservation claim or captured identity mismatch".into());
        }
        if record.status != Status::Reserved {
            return Ok(StartOutcome::Replay(record.status));
        }
        let p = read_policy(&tx, &binding.task_id)?.ok_or("reservation policy missing")?;
        let usage =
            budget::snapshot(&tx, &self.project, &binding.task_id)?.ok_or("task budget missing")?;
        let others = records(&tx, &self.project, &binding.task_id)?
            .into_iter()
            .filter(|r| r.operation_id != operation)
            .collect::<Vec<_>>();
        check_budget(&usage, &p, &others, &record.resources, boot)
            .map_err(|e| format!("budget denied: {e}"))?;
        record.status = Status::Started;
        save(&tx, &binding.task_id, &record)?;
        tx.commit().map_err(error)?;
        Ok(StartOutcome::Granted(StartedPermit {
            operation_id: operation.into(),
            binding: binding.clone(),
        }))
    }
    pub fn release_unstarted(
        &mut self,
        binding: &Binding,
        operation: &str,
        reason: &str,
    ) -> Result<(), String> {
        self.finish_without_usage(binding, operation, reason, false)
    }
    pub fn mark_unknown(
        &mut self,
        binding: &Binding,
        operation: &str,
        reason: &str,
    ) -> Result<(), String> {
        self.finish_without_usage(binding, operation, reason, true)
    }
    fn finish_without_usage(
        &mut self,
        binding: &Binding,
        operation: &str,
        reason: &str,
        unknown: bool,
    ) -> Result<(), String> {
        id(operation)?;
        id(reason)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !existing(&tx)? {
            return Err("reservation namespace missing".into());
        }
        run::validate_budget_origin(&tx, binding)?;
        let mut record = records(&tx, &self.project, &binding.task_id)?
            .into_iter()
            .find(|r| r.operation_id == operation)
            .ok_or("reservation missing")?;
        if record.binding != *binding {
            return Err("reservation captured identity mismatch".into());
        }
        let target = if unknown {
            Status::Unknown
        } else {
            Status::ReleasedUnstarted
        };
        if record.status == target && record.reason.as_deref() == Some(reason) {
            return Ok(());
        }
        if (unknown && record.status != Status::Started)
            || (!unknown && record.status != Status::Reserved)
        {
            return Err(
                "executed or unknown reservation cannot be released without reconciliation".into(),
            );
        }
        record.status = target;
        record.reason = Some(reason.into());
        save(&tx, &binding.task_id, &record)?;
        tx.commit().map_err(error)
    }
    pub fn settle(&mut self, usage: Usage) -> Result<Snapshot, String> {
        budget::validate_usage(&usage, &self.project, &usage.root_task_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !existing(&tx)? {
            return Err("reservation namespace missing".into());
        }
        run::validate_budget_origin(&tx, &usage.binding)?;
        let mut record = records(&tx, &self.project, &usage.root_task_id)?
            .into_iter()
            .find(|r| r.operation_id == usage.operation_id)
            .ok_or("reservation missing")?;
        if record.binding != usage.binding || record.member_id != usage.member_id {
            return Err("usage does not match captured reservation".into());
        }
        let p = read_policy(&tx, &usage.root_task_id)?.ok_or("reservation policy missing")?;
        if p.max_total_tokens.is_some()
            && (usage.measurement.input_tokens.is_none()
                || usage.measurement.output_tokens.is_none())
            || p.max_active_ms.is_some() && usage.measurement.active_ms.is_none()
            || p.max_cost.is_some() && usage.measurement.cost.is_none()
        {
            return Err("unknown usage cannot release enforced reservation".into());
        }
        if let (Some(limit), Some(actual)) = (&p.max_cost, &usage.measurement.cost) {
            if limit.currency != actual.currency {
                return Err("settlement currency mismatch".into());
            }
        }
        if !matches!(
            record.status,
            Status::Started | Status::Unknown | Status::Settled
        ) {
            return Err("unstarted reservation cannot settle executed usage".into());
        }
        let old:Option<String>=tx.query_row("SELECT record FROM resource_budget_usage WHERE task_id=?1 AND (usage_id=?2 OR operation_id=?3)",params![usage.root_task_id,usage.usage_id,usage.operation_id],|r|r.get(0)).optional().map_err(error)?;
        if let Some(old) = old {
            if serde_json::from_str::<Usage>(&old).map_err(error)? != usage {
                return Err("usage settlement identity conflict".into());
            }
        } else {
            tx.execute(
                "INSERT INTO resource_budget_usage VALUES(?1,?2,?3,?4)",
                params![
                    usage.root_task_id,
                    usage.usage_id,
                    usage.operation_id,
                    serde_json::to_string(&usage).map_err(error)?
                ],
            )
            .map_err(error)?;
        }
        if record.status != Status::Settled && overshoot(&record, &usage)? {
            record.reason = Some("settlement exceeded reservation".into());
        }
        record.status = Status::Settled;
        record.usage_id = Some(usage.usage_id.clone());
        save(&tx, &usage.root_task_id, &record)?;
        let snapshot = budget::snapshot(&tx, &self.project, &usage.root_task_id)?
            .ok_or("task budget missing")?;
        records(&tx, &self.project, &usage.root_task_id)?;
        tx.commit().map_err(error)?;
        Ok(snapshot)
    }
    pub fn query(&mut self, task: &str) -> Result<Option<ReservationSnapshot>, String> {
        id(task)?;
        let tx = self.conn.transaction().map_err(error)?;
        if !existing(&tx)? {
            return Ok(None);
        }
        let all = records(&tx, &self.project, task)?;
        let Some(policy) = read_policy(&tx, task)? else {
            return Ok(None);
        };
        let usage = budget::snapshot(&tx, &self.project, task)?.ok_or("task budget missing")?;
        let outstanding = all
            .into_iter()
            .filter(|r| {
                matches!(
                    r.status,
                    Status::Reserved | Status::Started | Status::Unknown
                )
            })
            .collect::<Vec<_>>();
        let in_flight = i64::try_from(outstanding.len()).map_err(error)?;
        let unknown_operations = i64::try_from(
            outstanding
                .iter()
                .filter(|r| r.status == Status::Unknown)
                .count(),
        )
        .map_err(error)?;
        Ok(Some(ReservationSnapshot {
            usage,
            policy,
            outstanding,
            in_flight,
            unknown_operations,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        budget::{BudgetStore, Measurement},
        run::{Execution, RunStore},
    };
    struct Fixture {
        path: std::path::PathBuf,
        binding: Binding,
        runs: RunStore,
    }
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("agentz-reservation-{}", uuid::Uuid::new_v4()));
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
            let mut budget = BudgetStore::open_existing(&path, false).unwrap();
            budget.initialize().unwrap();
            budget
                .configure(
                    "task",
                    budget::Policy {
                        revision: 1,
                        max_total_tokens: None,
                        max_active_ms: None,
                        max_cost: None,
                    },
                )
                .unwrap();
            Self {
                path,
                binding,
                runs,
            }
        }
        fn store(&self, p: Policy) -> ReservationStore {
            let mut store = ReservationStore::open_existing(&self.path, false).unwrap();
            store.initialize().unwrap();
            store.configure("task", p).unwrap();
            store
        }
        fn request(&self, operation: &str, tokens: i64) -> Request {
            request(&self.binding, operation, tokens, "boot")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert!(self.path.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    fn policy() -> Policy {
        Policy {
            revision: 1,
            max_total_tokens: Some(10),
            max_active_ms: None,
            max_cost: None,
            max_in_flight: 2,
        }
    }
    fn request(binding: &Binding, operation: &str, tokens: i64, boot: &str) -> Request {
        Request {
            reservation: Reservation {
                binding: binding.clone(),
                member_id: binding.task_id.clone(),
                operation_id: operation.into(),
                boot_id: boot.into(),
                kind: "main".into(),
                intent_digest: "b".repeat(64),
                claim_digest: "c".repeat(64),
                policy_revision: 1,
                resources: Resources {
                    total_tokens: Some(tokens),
                    active_ms: None,
                    cost: None,
                },
                status: Status::Reserved,
                usage_id: None,
                reason: None,
            },
            claim: "host-claim".into(),
        }
    }
    fn usage(binding: &Binding, operation: &str, tokens: Option<i64>) -> Usage {
        Usage {
            project_id: binding.project_id.clone(),
            root_task_id: binding.task_id.clone(),
            member_id: binding.task_id.clone(),
            usage_id: format!("usage-{operation}"),
            operation_id: operation.into(),
            binding: binding.clone(),
            measurement: Measurement {
                input_tokens: tokens,
                output_tokens: Some(0),
                active_ms: None,
                cost: None,
                source: "provider_reported".into(),
                price_source: None,
            },
            created_ms: 7,
        }
    }
    #[test]
    fn duplicate_reservation_and_start_never_issue_second_permit() {
        let f = Fixture::new();
        let mut store = f.store(policy());
        assert_eq!(
            store.reserve(f.request("op", 8)).unwrap(),
            ReserveOutcome::Created
        );
        assert_eq!(
            store.reserve(f.request("op", 8)).unwrap(),
            ReserveOutcome::Replay(Status::Reserved)
        );
        assert!(store.reserve(f.request("op", 7)).is_err());
        assert!(store.start(&f.binding, "op", "bad-claim", "boot").is_err());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers = (0..2)
            .map(|_| {
                let path = f.path.clone();
                let binding = f.binding.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    matches!(
                        ReservationStore::open_existing(&path, false)
                            .unwrap()
                            .start(&binding, "op", "host-claim", "boot")
                            .unwrap(),
                        StartOutcome::Granted(_)
                    )
                })
            })
            .collect::<Vec<_>>();
        let granted = workers
            .into_iter()
            .map(|worker| usize::from(worker.join().unwrap()))
            .sum::<usize>();
        assert_eq!(granted, 1);
        assert_eq!(
            store.reserve(f.request("op", 8)).unwrap(),
            ReserveOutcome::Replay(Status::Started)
        );
        assert_eq!(store.query("task").unwrap().unwrap().in_flight, 1);
    }
    #[test]
    fn competing_connections_cannot_overreserve_last_tokens_or_slot() {
        let f = Fixture::new();
        let mut p = policy();
        p.max_in_flight = 1;
        let mut store = f.store(p);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers = (0..2)
            .map(|index| {
                let path = f.path.clone();
                let binding = f.binding.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    let op = format!("op-{index}");
                    ReservationStore::open_existing(&path, false)
                        .unwrap()
                        .reserve(request(&binding, &op, 10, "boot"))
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            workers
                .into_iter()
                .map(|worker| usize::from(worker.join().unwrap()))
                .sum::<usize>(),
            1
        );
        let snapshot = store.query("task").unwrap().unwrap();
        assert_eq!(snapshot.in_flight, 1);
        store
            .release_unstarted(
                &f.binding,
                &snapshot.outstanding[0].operation_id,
                "queue cancelled before execution",
            )
            .unwrap();
        assert_eq!(store.query("task").unwrap().unwrap().in_flight, 0);
        assert!(store.reserve(f.request("too-large", 11)).is_err());
    }
    #[test]
    fn historical_settlement_is_atomic_idempotent_and_retry_does_not_reset() {
        let mut f = Fixture::new();
        let mut store = f.store(policy());
        store.reserve(f.request("op", 9)).unwrap();
        assert!(matches!(
            store.start(&f.binding, "op", "host-claim", "boot").unwrap(),
            StartOutcome::Granted(_)
        ));
        f.runs
            .transition("run", "a1", Execution::Running, Execution::Failed, 4)
            .unwrap();
        f.runs.retry("run", "a1", "a2", 5).unwrap();
        f.runs
            .transition("run", "a2", Execution::Queued, Execution::Running, 6)
            .unwrap();
        assert!(store.reserve(f.request("old", 1)).is_err());
        let final_usage = usage(&f.binding, "op", Some(7));
        let snapshot = store.settle(final_usage.clone()).unwrap();
        assert_eq!(store.settle(final_usage.clone()).unwrap(), snapshot);
        let mut conflict = final_usage;
        conflict.measurement.input_tokens = Some(6);
        assert!(store.settle(conflict).is_err());
        let mut fresh = f.binding.clone();
        fresh.attempt_id = "a2".into();
        assert!(store.reserve(request(&fresh, "next", 4, "boot")).is_err());
        drop(store);
        let result = ReservationStore::open_existing(&f.path, true)
            .unwrap()
            .query("task")
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                result.usage.usage_count,
                result.usage.known_input_tokens,
                result.in_flight
            ),
            (1, 7, 0)
        );
    }
    #[test]
    fn unknown_or_previous_boot_keeps_preallocation_until_known_settlement() {
        let f = Fixture::new();
        let mut store = f.store(policy());
        store.reserve(f.request("op", 8)).unwrap();
        store.start(&f.binding, "op", "host-claim", "boot").unwrap();
        drop(store);
        let mut store = ReservationStore::open_existing(&f.path, false).unwrap();
        assert!(store
            .reserve(request(&f.binding, "new", 1, "new-boot"))
            .is_err());
        assert!(store.release_unstarted(&f.binding, "op", "cancel").is_err());
        store
            .mark_unknown(&f.binding, "op", "connection lost after send")
            .unwrap();
        assert!(store.reserve(f.request("new", 1)).is_err());
        assert!(store.settle(usage(&f.binding, "op", None)).is_err());
        let result = store.query("task").unwrap().unwrap();
        assert_eq!(
            (
                result.in_flight,
                result.unknown_operations,
                result.usage.usage_count
            ),
            (1, 1, 0)
        );
        store.settle(usage(&f.binding, "op", Some(3))).unwrap();
        assert_eq!(store.query("task").unwrap().unwrap().in_flight, 0);
    }
    fn raised(revision: i64) -> Policy {
        Policy {
            revision,
            max_total_tokens: Some(20),
            max_active_ms: None,
            max_cost: None,
            max_in_flight: 2,
        }
    }

    #[test]
    fn revision_lifts_overshoot_block_and_refuses_stale_or_skipped_revisions() {
        let f = Fixture::new();
        let mut store = f.store(policy());
        store.reserve(f.request("op", 1)).unwrap();
        store.start(&f.binding, "op", "host-claim", "boot").unwrap();
        store.settle(usage(&f.binding, "op", Some(2))).unwrap();
        assert!(store.reserve(f.request("blocked", 1)).is_err());

        // Stale (expected 0) and skipped (revision 3) revisions are refused and change nothing.
        assert!(store.revise_policy("task", 0, raised(1)).is_err());
        assert!(store.revise_policy("task", 1, raised(3)).is_err());
        assert_eq!(store.query("task").unwrap().unwrap().policy.revision, 1);

        store.revise_policy("task", 1, raised(2)).unwrap();
        let mut next = f.request("after", 1);
        next.reservation.policy_revision = 2;
        store.reserve(next).unwrap();
        let snapshot = store.query("task").unwrap().unwrap();
        assert_eq!(snapshot.policy.revision, 2);
        assert_eq!(snapshot.usage.policy.revision, 2);
    }

    #[test]
    fn unlimited_policy_is_not_blocked_by_an_overshoot() {
        let f = Fixture::new();
        let mut store = f.store(Policy {
            revision: 1,
            max_total_tokens: None,
            max_active_ms: None,
            max_cost: None,
            max_in_flight: 2,
        });
        store.reserve(f.request("op", 1)).unwrap();
        store.start(&f.binding, "op", "host-claim", "boot").unwrap();
        store.settle(usage(&f.binding, "op", Some(2))).unwrap();
        store.reserve(f.request("next", 1)).unwrap();
    }

    #[test]
    fn orphaned_call_is_kept_while_its_run_runs_and_settled_at_estimate_after_interruption() {
        let mut f = Fixture::new();
        let mut store = f.store(policy());
        store.reserve(f.request("orphan", 4)).unwrap();
        store.start(&f.binding, "orphan", "host-claim", "boot").unwrap();

        // A second process while the run is still running must not reclaim the call.
        let mut live = f.store(policy());
        assert!(live
            .reserve(request(&f.binding, "next", 1, "new-boot"))
            .is_err());

        // After the run is interrupted through the Run API, recovery charges the reserved
        // estimate conservatively.
        f.runs
            .transition("run", "a1", Execution::Running, Execution::Interrupted, 3)
            .unwrap();
        let mut conn = rusqlite::Connection::open(f.path.join(".agentz/runs.db")).unwrap();
        let tx = conn.transaction().unwrap();
        let refreshed =
            reconcile_orphans(&tx, &f.binding.project_id, "task", "new-boot").unwrap();
        tx.commit().unwrap();
        let orphan = refreshed
            .iter()
            .find(|r| r.operation_id == "orphan")
            .unwrap();
        assert_eq!(orphan.status, Status::Settled);
        assert_eq!(
            orphan.reason.as_deref(),
            Some("recovered after restart: settled at reserved estimate")
        );
        let snapshot = store.query("task").unwrap().unwrap();
        assert_eq!(snapshot.usage.known_input_tokens, 4);
        assert_eq!(snapshot.in_flight, 0);
    }

    #[test]
    fn overshoot_is_recorded_and_stops_next_reservation_and_missing_price_refuses_cost_limit() {
        let f = Fixture::new();
        let mut store = f.store(policy());
        store.reserve(f.request("op", 1)).unwrap();
        store.start(&f.binding, "op", "host-claim", "boot").unwrap();
        assert_eq!(
            store
                .settle(usage(&f.binding, "op", Some(2)))
                .unwrap()
                .known_input_tokens,
            2
        );
        assert!(store.reserve(f.request("next", 1)).is_err());
        let priced = Fixture::new();
        let mut p = policy();
        p.max_cost = Some(Money {
            currency: "USD".into(),
            micro_units: 100,
        });
        let mut store = priced.store(p);
        assert!(store.reserve(priced.request("unpriced", 1)).is_err());
        let mut request = priced.request("known", 1);
        request.reservation.resources.cost = Some(Money {
            currency: "USD".into(),
            micro_units: 20,
        });
        store.reserve(request).unwrap();
        store
            .start(&priced.binding, "known", "host-claim", "boot")
            .unwrap();
        assert!(store
            .settle(usage(&priced.binding, "known", Some(1)))
            .is_err());
        assert_eq!(store.query("task").unwrap().unwrap().in_flight, 1);
    }
    #[test]
    fn readonly_missing_namespace_and_corrupt_records_do_not_initialize_or_repair() {
        let f = Fixture::new();
        let file = f.path.join(".agentz/runs.db");
        let before = std::fs::read(&file).unwrap();
        assert!(ReservationStore::open_existing(&f.path, true)
            .unwrap()
            .query("task")
            .unwrap()
            .is_none());
        assert_eq!(std::fs::read(&file).unwrap(), before);
        for corruption in [
            "UPDATE resource_reservation_schema SET version=99",
            "UPDATE resource_reservations SET record='{\"bad\":true}'",
            "DELETE FROM resource_reservation_schema",
        ] {
            let damaged = Fixture::new();
            let mut store = damaged.store(policy());
            store.reserve(damaged.request("op", 8)).unwrap();
            drop(store);
            let file = damaged.path.join(".agentz/runs.db");
            let conn = Connection::open(&file).unwrap();
            conn.execute_batch(corruption).unwrap();
            drop(conn);
            let corrupt = std::fs::read(&file).unwrap();
            let mut store = ReservationStore::open_existing(&damaged.path, true).unwrap();
            assert!(store.query("task").is_err());
            drop(store);
            assert_eq!(std::fs::read(&file).unwrap(), corrupt);
        }
        let conn = Connection::open(&file).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }
    #[test]
    fn corrupted_settlement_identity_or_overshoot_flag_fails_closed() {
        for corrupt_identity in [false, true] {
            let f = Fixture::new();
            let mut store = f.store(policy());
            store.reserve(f.request("op", 1)).unwrap();
            store.start(&f.binding, "op", "host-claim", "boot").unwrap();
            store.settle(usage(&f.binding, "op", Some(2))).unwrap();
            drop(store);
            let file = f.path.join(".agentz/runs.db");
            let conn = Connection::open(&file).unwrap();
            let encoded: String = conn
                .query_row(
                    "SELECT record FROM resource_reservations WHERE operation_id='op'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            let mut record: Reservation = serde_json::from_str(&encoded).unwrap();
            if corrupt_identity {
                record.binding.capability_version = "different-capture".into();
            } else {
                record.reason = None;
            }
            conn.execute(
                "UPDATE resource_reservations SET record=?1",
                [serde_json::to_string(&record).unwrap()],
            )
            .unwrap();
            drop(conn);
            let before = std::fs::read(&file).unwrap();
            let mut store = ReservationStore::open_existing(&f.path, true).unwrap();
            assert!(store.query("task").is_err());
            drop(store);
            assert_eq!(std::fs::read(&file).unwrap(), before);
        }
    }
    #[test]
    fn existing_observer_operation_cannot_create_inconsistent_reservation() {
        let f = Fixture::new();
        let mut store = f.store(policy());
        let mut observer = BudgetStore::open_existing(&f.path, false).unwrap();
        let before = observer
            .record_usage(usage(&f.binding, "occupied", Some(1)))
            .unwrap();
        assert!(store.reserve(f.request("occupied", 1)).is_err());
        let query = store.query("task").unwrap().unwrap();
        assert_eq!(query.in_flight, 0);
        assert_eq!(query.usage, before);
        let conn = Connection::open_with_flags(
            f.path.join(".agentz/runs.db"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM resource_reservations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(observer.query("task").unwrap().unwrap(), before);
    }
    #[test]
    fn settlement_update_failure_rolls_back_observer_insert_and_keeps_started_reservation() {
        let f = Fixture::new();
        let mut store = f.store(policy());
        store.reserve(f.request("op", 8)).unwrap();
        store.start(&f.binding, "op", "host-claim", "boot").unwrap();
        let before = store.query("task").unwrap().unwrap();
        let conn = Connection::open(f.path.join(".agentz/runs.db")).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_reservation_update BEFORE UPDATE ON resource_reservations BEGIN SELECT RAISE(ABORT,'fixture reservation update failure'); END;").unwrap();
        assert!(store.settle(usage(&f.binding, "op", Some(3))).is_err());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM resource_budget_usage", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(store.query("task").unwrap().unwrap(), before);
        conn.execute_batch("DROP TRIGGER reject_reservation_update;")
            .unwrap();
        store.settle(usage(&f.binding, "op", Some(3))).unwrap();
        assert_eq!(store.query("task").unwrap().unwrap().usage.usage_count, 1);
    }
    #[test]
    fn new_observer_consumption_before_start_keeps_reserved_when_balance_insufficient() {
        let f = Fixture::new();
        let mut store = f.store(policy());
        store.reserve(f.request("op", 8)).unwrap();
        BudgetStore::open_existing(&f.path, false)
            .unwrap()
            .record_usage(usage(&f.binding, "other-operation", Some(3)))
            .unwrap();
        assert!(store.start(&f.binding, "op", "host-claim", "boot").is_err());
        let query = store.query("task").unwrap().unwrap();
        assert_eq!(query.in_flight, 1);
        assert_eq!(query.outstanding[0].status, Status::Reserved);
        assert_eq!(query.usage.known_input_tokens, 3);
        store
            .release_unstarted(&f.binding, "op", "budget changed before request")
            .unwrap();
        assert_eq!(store.query("task").unwrap().unwrap().in_flight, 0);
    }
}
