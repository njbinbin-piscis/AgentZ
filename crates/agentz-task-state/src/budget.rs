//! Observational task accounting candidate. No request permits, fee enforcement or retries.
use crate::{approval::Binding, run};
use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Money {
    pub currency: String,
    pub micro_units: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub revision: i64,
    pub max_total_tokens: Option<i64>,
    pub max_active_ms: Option<i64>,
    pub max_cost: Option<Money>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub active_ms: Option<i64>,
    pub cost: Option<Money>,
    /// Source of token/duration measurement, not a claim of billing precision.
    pub source: String,
    /// Required for known cost; a caller-supplied immutable provenance snapshot.
    pub price_source: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub project_id: String,
    pub root_task_id: String,
    pub member_id: String,
    pub usage_id: String,
    pub operation_id: String,
    pub binding: Binding,
    pub measurement: Measurement,
    pub created_ms: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    project_id: String,
    root_task_id: String,
    policy: Policy,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub root_task_id: String,
    pub policy: Policy,
    pub usage_count: i64,
    pub known_input_tokens: i64,
    pub known_output_tokens: i64,
    pub unknown_token_records: i64,
    pub known_active_ms: i64,
    pub unknown_duration_records: i64,
    pub known_cost_micro_units: BTreeMap<String, i64>,
    pub unknown_price_records: i64,
    /// Observed excess only. This phase does not prevent a model call.
    pub observed_limits_exceeded: Vec<String>,
}

fn error(value: impl std::fmt::Display) -> String {
    format!("budget store: {value}")
}
fn identity(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err("invalid budget identity".into());
    }
    Ok(())
}
fn nonnegative(value: Option<i64>) -> Result<(), String> {
    if value.is_some_and(|value| value < 0) {
        return Err("negative budget measurement or limit".into());
    }
    Ok(())
}
fn money(value: &Money) -> Result<(), String> {
    if value.currency.len() != 3 || !value.currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err("invalid budget currency".into());
    }
    nonnegative(Some(value.micro_units))
}
fn validate_policy(value: &Policy) -> Result<(), String> {
    if value.revision < 1 {
        return Err("invalid budget policy revision".into());
    }
    nonnegative(value.max_total_tokens)?;
    nonnegative(value.max_active_ms)?;
    if let Some(cost) = &value.max_cost {
        money(cost)?;
    }
    Ok(())
}
pub(crate) fn validate_usage(value: &Usage, project: &str, task: &str) -> Result<(), String> {
    value.binding.validate()?;
    for id in [
        &value.root_task_id,
        &value.member_id,
        &value.usage_id,
        &value.operation_id,
        &value.measurement.source,
    ] {
        identity(id)?;
    }
    if value.project_id != project
        || value.binding.project_id != project
        || value.root_task_id != task
        || value.member_id != task
        || value.binding.task_id != task
    {
        return Err("external member or cross-task budget usage is unsupported".into());
    }
    nonnegative(Some(value.created_ms))?;
    nonnegative(value.measurement.input_tokens)?;
    nonnegative(value.measurement.output_tokens)?;
    nonnegative(value.measurement.active_ms)?;
    if let Some(price) = &value.measurement.price_source {
        identity(price)?;
    }
    if let Some(cost) = &value.measurement.cost {
        money(cost)?;
        if value.measurement.price_source.is_none() {
            return Err("known cost requires price provenance".into());
        }
    }
    Ok(())
}
fn add(total: &mut i64, value: i64) -> Result<(), String> {
    *total = total.checked_add(value).ok_or("budget integer overflow")?;
    Ok(())
}

pub(crate) fn namespace(tx: &Transaction<'_>) -> Result<bool, String> {
    run::validate_budget_store(tx)?;
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('resource_budget_schema','resource_budget_tasks','resource_budget_usage')", [], |row| row.get(0)).map_err(error)?;
    if count == 0 {
        return Ok(false);
    }
    if count != 3 {
        return Err("partial budget namespace requires explicit recovery".into());
    }
    let (count, version): (i64, Option<i64>) = tx
        .query_row(
            "SELECT COUNT(*),MAX(version) FROM resource_budget_schema",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(error)?;
    if count != 1 || version != Some(1) {
        return Err("unsupported budget namespace version".into());
    }
    for (table, expected) in [
        ("resource_budget_schema", vec![("version", "INTEGER", 1)]),
        (
            "resource_budget_tasks",
            vec![("task_id", "TEXT", 1), ("record", "TEXT", 0)],
        ),
        (
            "resource_budget_usage",
            vec![
                ("task_id", "TEXT", 1),
                ("usage_id", "TEXT", 2),
                ("operation_id", "TEXT", 0),
                ("record", "TEXT", 0),
            ],
        ),
    ] {
        let mut statement = tx
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(error)?;
        let columns = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(5)?,
                ))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        let wanted = expected
            .into_iter()
            .map(|(name, kind, key)| (name.to_owned(), kind.to_owned(), 1, key))
            .collect::<Vec<_>>();
        if columns != wanted {
            return Err("unsupported budget namespace layout".into());
        }
    }
    let mut indexes = tx.prepare("SELECT name FROM pragma_index_list('resource_budget_usage') WHERE \"unique\"=1 AND partial=0").map_err(error)?;
    let names = indexes
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    let mut operation_unique = false;
    for name in names {
        let mut query = tx
            .prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
            .map_err(error)?;
        let keys = query
            .query_map([name], |row| row.get::<_, Option<String>>(0))
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        operation_unique |= keys == vec![Some("task_id".into()), Some("operation_id".into())];
    }
    if !operation_unique {
        return Err("budget operation uniqueness is missing".into());
    }
    let bad_version: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM resource_budget_schema WHERE typeof(version)!='integer'",
            [],
            |row| row.get(0),
        )
        .map_err(error)?;
    if bad_version != 0 {
        return Err("invalid budget version storage type".into());
    }
    Ok(true)
}
fn configuration(
    tx: &Transaction<'_>,
    project: &str,
    task: &str,
) -> Result<Option<Configuration>, String> {
    let record: Option<(String,String,String)> = tx.query_row("SELECT record,typeof(task_id),typeof(record) FROM resource_budget_tasks WHERE task_id=?1", [task], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional().map_err(error)?;
    record
        .map(|(json, key_type, record_type)| {
            if key_type != "text" || record_type != "text" || json.len() > 65536 {
                return Err("invalid budget policy storage".into());
            }
            let configuration: Configuration = serde_json::from_str(&json).map_err(error)?;
            if configuration.project_id != project || configuration.root_task_id != task {
                return Err("budget policy identity mismatch".into());
            }
            validate_policy(&configuration.policy)?;
            Ok(configuration)
        })
        .transpose()
}
/// Advance a task's budget policy inside the caller's transaction. Only the next revision of the
/// stored policy is accepted, so a stale or concurrent caller cannot overwrite a newer limit.
pub(crate) fn revise_in_tx(
    tx: &Transaction<'_>,
    project: &str,
    task: &str,
    expected: i64,
    value: Policy,
) -> Result<(), String> {
    identity(task)?;
    validate_policy(&value)?;
    let current = configuration(tx, project, task)?.ok_or("budget configuration missing")?;
    if current.policy.revision != expected {
        return Err("stale budget policy revision".into());
    }
    if Some(value.revision) != expected.checked_add(1) {
        return Err("budget policy revision must advance by exactly one".into());
    }
    let desired = Configuration {
        project_id: project.into(),
        root_task_id: task.into(),
        policy: value,
    };
    let count = tx
        .execute(
            "UPDATE resource_budget_tasks SET record=?2 WHERE task_id=?1",
            params![task, serde_json::to_string(&desired).map_err(error)?],
        )
        .map_err(error)?;
    if count != 1 {
        return Err("budget configuration missing".into());
    }
    snapshot(tx, project, task)?;
    Ok(())
}
pub(crate) fn snapshot(tx: &Transaction<'_>, project: &str, task: &str) -> Result<Option<Snapshot>, String> {
    let Some(configuration) = configuration(tx, project, task)? else {
        let count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM resource_budget_usage WHERE task_id=?1",
                [task],
                |row| row.get(0),
            )
            .map_err(error)?;
        if count != 0 {
            return Err("budget usage lacks policy".into());
        }
        return Ok(None);
    };
    run::validate_budget_task(tx, task)?;
    let mut result = Snapshot {
        root_task_id: task.into(),
        policy: configuration.policy,
        usage_count: 0,
        known_input_tokens: 0,
        known_output_tokens: 0,
        unknown_token_records: 0,
        known_active_ms: 0,
        unknown_duration_records: 0,
        known_cost_micro_units: BTreeMap::new(),
        unknown_price_records: 0,
        observed_limits_exceeded: Vec::new(),
    };
    let mut statement = tx.prepare("SELECT usage_id,operation_id,record,typeof(task_id),typeof(usage_id),typeof(operation_id),typeof(record) FROM resource_budget_usage WHERE task_id=?1 ORDER BY rowid").map_err(error)?;
    let mut rows = statement.query([task]).map_err(error)?;
    while let Some(row) = rows.next().map_err(error)? {
        for index in 3..=6 {
            if row.get::<_, String>(index).map_err(error)? != "text" {
                return Err("invalid budget usage storage type".into());
            }
        }
        let record: String = row.get(2).map_err(error)?;
        if record.len() > 65536 {
            return Err("budget usage record exceeds limit".into());
        }
        let usage: Usage = serde_json::from_str(&record).map_err(error)?;
        validate_usage(&usage, project, task)?;
        if usage.usage_id != row.get::<_, String>(0).map_err(error)?
            || usage.operation_id != row.get::<_, String>(1).map_err(error)?
        {
            return Err("budget usage key mismatch".into());
        }
        run::validate_budget_origin(tx, &usage.binding)?;
        add(&mut result.usage_count, 1)?;
        if let Some(value) = usage.measurement.input_tokens {
            add(&mut result.known_input_tokens, value)?;
        }
        if let Some(value) = usage.measurement.output_tokens {
            add(&mut result.known_output_tokens, value)?;
        }
        if usage.measurement.input_tokens.is_none() || usage.measurement.output_tokens.is_none() {
            add(&mut result.unknown_token_records, 1)?;
        }
        if let Some(value) = usage.measurement.active_ms {
            add(&mut result.known_active_ms, value)?;
        } else {
            add(&mut result.unknown_duration_records, 1)?;
        }
        if let Some(cost) = usage.measurement.cost {
            add(
                result
                    .known_cost_micro_units
                    .entry(cost.currency)
                    .or_default(),
                cost.micro_units,
            )?;
        } else {
            add(&mut result.unknown_price_records, 1)?;
        }
    }
    let total = result
        .known_input_tokens
        .checked_add(result.known_output_tokens)
        .ok_or("budget integer overflow")?;
    if result
        .policy
        .max_total_tokens
        .is_some_and(|limit| total > limit)
    {
        result.observed_limits_exceeded.push("tokens".into());
    }
    if result
        .policy
        .max_active_ms
        .is_some_and(|limit| result.known_active_ms > limit)
    {
        result.observed_limits_exceeded.push("active_ms".into());
    }
    if let Some(limit) = &result.policy.max_cost {
        if result
            .known_cost_micro_units
            .get(&limit.currency)
            .copied()
            .unwrap_or(0)
            > limit.micro_units
        {
            result.observed_limits_exceeded.push("cost".into());
        }
    }
    Ok(Some(result))
}

pub struct BudgetStore {
    conn: Connection,
    project: String,
}
impl BudgetStore {
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
    /// Explicit namespace initialization only; never creates or migrates RunStore.
    pub fn initialize(&mut self) -> Result<(), String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !namespace(&tx)? {
            tx.execute_batch("CREATE TABLE resource_budget_schema(version INTEGER PRIMARY KEY NOT NULL); INSERT INTO resource_budget_schema VALUES(1);
                CREATE TABLE resource_budget_tasks(task_id TEXT PRIMARY KEY NOT NULL,record TEXT NOT NULL);
                CREATE TABLE resource_budget_usage(task_id TEXT NOT NULL,usage_id TEXT NOT NULL,operation_id TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(task_id,usage_id),UNIQUE(task_id,operation_id));").map_err(error)?;
            namespace(&tx)?;
        }
        tx.commit().map_err(error)
    }
    /// First policy is immutable in phase one. Reconfiguration needs an explicit later CAS API.
    pub fn configure(&mut self, task: &str, policy: Policy) -> Result<(), String> {
        identity(task)?;
        validate_policy(&policy)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !namespace(&tx)? {
            return Err("budget namespace is not initialized".into());
        }
        let exists: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE task_id=?1",
                [task],
                |row| row.get(0),
            )
            .map_err(error)?;
        if exists == 0 {
            return Err("budget requires an existing task Run".into());
        }
        run::validate_budget_task(&tx, task)?;
        let desired = Configuration {
            project_id: self.project.clone(),
            root_task_id: task.into(),
            policy,
        };
        if let Some(existing) = configuration(&tx, &self.project, task)? {
            if existing != desired {
                return Err("budget policy is immutable; explicit revision change required".into());
            }
        } else {
            tx.execute(
                "INSERT INTO resource_budget_tasks VALUES(?1,?2)",
                params![task, serde_json::to_string(&desired).map_err(error)?],
            )
            .map_err(error)?;
        }
        snapshot(&tx, &self.project, task)?;
        tx.commit().map_err(error)
    }
    /// Immutable final measurement only, including explicit unknowns. No reservation is released.
    pub fn record_usage(&mut self, usage: Usage) -> Result<Snapshot, String> {
        validate_usage(&usage, &self.project, &usage.root_task_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        if !namespace(&tx)? {
            return Err("budget namespace is not initialized".into());
        }
        run::validate_budget_origin(&tx, &usage.binding)?;
        if configuration(&tx, &self.project, &usage.root_task_id)?.is_none() {
            return Err("task budget is not configured".into());
        }
        let encoded = serde_json::to_string(&usage).map_err(error)?;
        if encoded.len() > 65536 {
            return Err("budget usage record exceeds limit".into());
        }
        let previous:Option<String>=tx.query_row("SELECT record FROM resource_budget_usage WHERE task_id=?1 AND (usage_id=?2 OR operation_id=?3)",params![usage.root_task_id,usage.usage_id,usage.operation_id],|row|row.get(0)).optional().map_err(error)?;
        if let Some(previous) = previous {
            let existing: Usage = serde_json::from_str(&previous).map_err(error)?;
            if existing != usage {
                return Err("budget usage identity reused with different content".into());
            }
        } else {
            tx.execute(
                "INSERT INTO resource_budget_usage VALUES(?1,?2,?3,?4)",
                params![
                    usage.root_task_id,
                    usage.usage_id,
                    usage.operation_id,
                    encoded
                ],
            )
            .map_err(error)?;
        }
        let result =
            snapshot(&tx, &self.project, &usage.root_task_id)?.ok_or("task budget missing")?;
        tx.commit().map_err(error)?;
        Ok(result)
    }
    pub fn query(&mut self, task: &str) -> Result<Option<Snapshot>, String> {
        identity(task)?;
        let tx = self.conn.transaction().map_err(error)?;
        if !namespace(&tx)? {
            return Ok(None);
        }
        snapshot(&tx, &self.project, task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::{Execution, RunStore};
    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("agentz-budget-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn binding(&self) -> Binding {
            Binding {
                project_id: crate::canonical_root(&self.0).unwrap(),
                task_id: "task".into(),
                environment_id: format!("direct:{}", "a".repeat(64)),
                binding_version: "a".repeat(64),
                run_id: "run".into(),
                attempt_id: "a1".into(),
                capability_version: "fixture".into(),
            }
        }
        fn running(&self) -> (RunStore, Binding) {
            let binding = self.binding();
            let mut store = RunStore::open(&self.0).unwrap();
            store.create_direct(&binding, 1).unwrap();
            store
                .transition("run", "a1", Execution::Queued, Execution::Running, 2)
                .unwrap();
            (store, binding)
        }
        fn budget(&self) -> BudgetStore {
            let mut store = BudgetStore::open_existing(&self.0, false).unwrap();
            store.initialize().unwrap();
            store.configure("task", policy()).unwrap();
            store
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn policy() -> Policy {
        Policy {
            revision: 1,
            max_total_tokens: Some(10),
            max_active_ms: Some(100),
            max_cost: Some(Money {
                currency: "USD".into(),
                micro_units: 50,
            }),
        }
    }
    fn usage(binding: &Binding, id: &str, input: Option<i64>) -> Usage {
        Usage {
            project_id: binding.project_id.clone(),
            root_task_id: "task".into(),
            member_id: "task".into(),
            usage_id: id.into(),
            operation_id: format!("op-{id}"),
            binding: binding.clone(),
            measurement: Measurement {
                input_tokens: input,
                output_tokens: Some(2),
                active_ms: Some(10),
                cost: None,
                source: "provider_reported".into(),
                price_source: None,
            },
            created_ms: 3,
        }
    }
    #[test]
    fn retry_new_run_and_environment_do_not_reset_task_accounting() {
        let d = Directory::new();
        let (mut runs, binding) = d.running();
        let mut budget = d.budget();
        budget
            .record_usage(usage(&binding, "first", Some(3)))
            .unwrap();
        runs.transition("run", "a1", Execution::Running, Execution::Failed, 4)
            .unwrap();
        runs.retry("run", "a1", "a2", 5).unwrap();
        runs.transition("run", "a2", Execution::Queued, Execution::Running, 6)
            .unwrap();
        // Late original-attempt usage remains a valid authoritative historical origin.
        budget
            .record_usage(usage(&binding, "late", Some(1)))
            .unwrap();
        let mut retry = binding.clone();
        retry.attempt_id = "a2".into();
        budget
            .record_usage(usage(&retry, "retry", Some(2)))
            .unwrap();
        runs.transition("run", "a2", Execution::Running, Execution::Failed, 7)
            .unwrap();
        let mut next = retry.clone();
        next.run_id = "next".into();
        next.attempt_id = "a3".into();
        next.binding_version = "b".repeat(64);
        next.environment_id = format!("direct:{}", next.binding_version);
        runs.create_direct(&next, 8).unwrap();
        runs.transition("next", "a3", Execution::Queued, Execution::Running, 9)
            .unwrap();
        budget.record_usage(usage(&next, "next", Some(4))).unwrap();
        drop(budget);
        let mut readonly = BudgetStore::open_existing(&d.0, true).unwrap();
        let result = readonly.query("task").unwrap().unwrap();
        assert_eq!(
            (
                result.usage_count,
                result.known_input_tokens,
                result.known_output_tokens
            ),
            (4, 10, 8)
        );
        assert_eq!(result.unknown_price_records, 4);
        assert_eq!(result.observed_limits_exceeded, vec!["tokens"]);
        assert!(readonly.query("other-task").unwrap().is_none());
    }
    #[test]
    fn exact_ids_are_idempotent_and_conflicts_or_external_members_reject() {
        let d = Directory::new();
        let (_runs, binding) = d.running();
        let mut budget = d.budget();
        let first = usage(&binding, "first", Some(3));
        let result = budget.record_usage(first.clone()).unwrap();
        assert_eq!(budget.record_usage(first.clone()).unwrap(), result);
        let mut conflict = first.clone();
        conflict.measurement.output_tokens = Some(99);
        assert!(budget.record_usage(conflict).is_err());
        let mut conflict = first.clone();
        conflict.usage_id = "different".into();
        assert!(budget.record_usage(conflict).is_err());
        for field in ["member", "task", "run", "attempt", "environment", "project"] {
            let mut wrong = usage(&binding, field, Some(1));
            match field {
                "member" => wrong.member_id = "child".into(),
                "task" => wrong.binding.task_id = "other".into(),
                "run" => wrong.binding.run_id = "other".into(),
                "attempt" => wrong.binding.attempt_id = "other".into(),
                "environment" => wrong.binding.environment_id = "other".into(),
                _ => wrong.binding.project_id = "other".into(),
            };
            assert!(budget.record_usage(wrong).is_err(), "{field}");
        }
        assert_eq!(budget.query("task").unwrap().unwrap().usage_count, 1);
        let mut changed = policy();
        changed.revision = 2;
        assert!(budget.configure("task", changed).is_err());
    }
    #[test]
    fn unknown_measurements_and_price_remain_explicit_and_costs_do_not_mix_currency() {
        let d = Directory::new();
        let (_runs, binding) = d.running();
        let mut budget = d.budget();
        let mut unknown = usage(&binding, "unknown", None);
        unknown.measurement.output_tokens = None;
        unknown.measurement.active_ms = None;
        budget.record_usage(unknown).unwrap();
        for (id, currency) in [("usd", "USD"), ("eur", "EUR")] {
            let mut measured = usage(&binding, id, Some(0));
            measured.measurement.cost = Some(Money {
                currency: currency.into(),
                micro_units: 30,
            });
            measured.measurement.price_source = Some("fixture-price-v1-estimate".into());
            budget.record_usage(measured).unwrap();
        }
        let result = budget.query("task").unwrap().unwrap();
        assert_eq!(
            (
                result.unknown_token_records,
                result.unknown_duration_records,
                result.unknown_price_records
            ),
            (1, 1, 1)
        );
        assert_eq!(result.known_cost_micro_units.get("USD"), Some(&30));
        assert_eq!(result.known_cost_micro_units.get("EUR"), Some(&30));
        assert!(!result
            .observed_limits_exceeded
            .iter()
            .any(|name| name == "cost"));
        let mut invalid = usage(&binding, "badprice", Some(1));
        invalid.measurement.cost = Some(Money {
            currency: "USD".into(),
            micro_units: 1,
        });
        assert!(budget.record_usage(invalid).is_err());
    }
    #[test]
    fn negative_and_overflow_usage_roll_back_without_incrementing_ledger() {
        let d = Directory::new();
        let (_runs, binding) = d.running();
        let mut budget = d.budget();
        assert!(budget
            .record_usage(usage(&binding, "negative", Some(-1)))
            .is_err());
        let mut maximum = usage(&binding, "max", Some(i64::MAX));
        maximum.measurement.output_tokens = Some(0);
        budget.record_usage(maximum).unwrap();
        assert!(budget
            .record_usage(usage(&binding, "overflow", Some(1)))
            .is_err());
        assert_eq!(budget.query("task").unwrap().unwrap().usage_count, 1);
    }
    #[test]
    fn missing_and_uninitialized_read_do_not_create_or_repair_and_bad_version_rejects() {
        let d = Directory::new();
        assert!(BudgetStore::open_existing(&d.0, false).is_err());
        assert!(!d.0.join(".agentz").exists());
        let (_runs, _binding) = d.running();
        let file = d.0.join(".agentz/runs.db");
        let before = std::fs::read(&file).unwrap();
        let mut readonly = BudgetStore::open_existing(&d.0, true).unwrap();
        assert!(readonly.query("task").unwrap().is_none());
        drop(readonly);
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let mut budget = d.budget();
        assert!(budget.initialize().is_ok());
        drop(budget);
        let conn = Connection::open(&file).unwrap();
        conn.execute("UPDATE resource_budget_schema SET version=99", [])
            .unwrap();
        drop(conn);
        let before = std::fs::read(&file).unwrap();
        let mut budget = BudgetStore::open_existing(&d.0, false).unwrap();
        assert!(budget.initialize().is_err());
        assert!(budget.query("task").is_err());
        drop(budget);
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }
    #[test]
    fn damaged_records_and_partial_schema_fail_closed_without_repair() {
        for corruption in [
            "UPDATE resource_budget_usage SET record='{\"bad\":true}'",
            "UPDATE resource_budget_usage SET operation_id='wrong'",
            "DELETE FROM resource_budget_schema",
            "DROP TABLE resource_budget_tasks",
        ] {
            let d = Directory::new();
            let (_runs, binding) = d.running();
            let mut budget = d.budget();
            budget
                .record_usage(usage(&binding, "one", Some(1)))
                .unwrap();
            drop(budget);
            let file = d.0.join(".agentz/runs.db");
            let conn = Connection::open(&file).unwrap();
            conn.execute_batch(corruption).unwrap();
            drop(conn);
            let before = std::fs::read(&file).unwrap();
            let mut budget = BudgetStore::open_existing(&d.0, true).unwrap();
            assert!(budget.query("task").is_err(), "{corruption}");
            drop(budget);
            assert_eq!(std::fs::read(&file).unwrap(), before);
        }
    }
    #[test]
    fn concurrent_initialization_and_usage_dedup_are_atomic() {
        let d = Directory::new();
        let (_runs, binding) = d.running();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads = (0..2)
            .map(|_| {
                let path = d.0.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    let mut budget = BudgetStore::open_existing(&path, false).unwrap();
                    budget.initialize().unwrap();
                    budget.configure("task", policy()).unwrap();
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
        let threads = (0..2)
            .map(|_| {
                let path = d.0.clone();
                let record = usage(&binding, "same", Some(3));
                std::thread::spawn(move || {
                    BudgetStore::open_existing(&path, false)
                        .unwrap()
                        .record_usage(record)
                        .unwrap();
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
        let result = BudgetStore::open_existing(&d.0, true)
            .unwrap()
            .query("task")
            .unwrap()
            .unwrap();
        assert_eq!((result.usage_count, result.known_input_tokens), (1, 3));
    }
    #[test]
    fn zero_usage_cannot_hide_corrupt_task_during_configuration_or_query() {
        for corruption in [
            "UPDATE runs SET execution='broken' WHERE run_id='run'",
            "DELETE FROM attempts WHERE attempt_id='a1'",
            "UPDATE attempts SET created_ms=-1 WHERE attempt_id='a1'",
        ] {
            for configured in [false, true] {
                let d = Directory::new();
                let (_runs, _binding) = d.running();
                let mut budget = BudgetStore::open_existing(&d.0, false).unwrap();
                budget.initialize().unwrap();
                if configured {
                    budget.configure("task", policy()).unwrap();
                }
                let file = d.0.join(".agentz/runs.db");
                let conn = Connection::open(&file).unwrap();
                conn.execute_batch(corruption).unwrap();
                drop(conn);
                let before = std::fs::read(&file).unwrap();
                assert!(budget.configure("task", policy()).is_err(), "{corruption}");
                if configured {
                    assert!(budget.query("task").is_err(), "{corruption}");
                }
                let conn =
                    Connection::open_with_flags(&file, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
                let policies: i64 = conn
                    .query_row("SELECT COUNT(*) FROM resource_budget_tasks", [], |row| {
                        row.get(0)
                    })
                    .unwrap();
                assert_eq!(policies, i64::from(configured));
                drop(conn);
                drop(budget);
                assert_eq!(std::fs::read(&file).unwrap(), before);
            }
        }
    }
    #[test]
    fn same_columns_without_attempt_primary_key_never_become_budget_authority() {
        for duplicate_history in [false, true] {
            let d = Directory::new();
            let (mut runs, binding) = d.running();
            runs.transition("run", "a1", Execution::Running, Execution::Failed, 4)
                .unwrap();
            runs.retry("run", "a1", "a2", 5).unwrap();
            runs.transition("run", "a2", Execution::Queued, Execution::Running, 6)
                .unwrap();
            let mut budget = d.budget();
            let file = d.0.join(".agentz/runs.db");
            let conn = Connection::open(&file).unwrap();
            conn.execute_batch("ALTER TABLE attempts RENAME TO saved_attempts;
                CREATE TABLE attempts(attempt_id TEXT NOT NULL,run_id TEXT NOT NULL,execution TEXT NOT NULL,created_ms INTEGER NOT NULL,updated_ms INTEGER NOT NULL);
                INSERT INTO attempts SELECT * FROM saved_attempts; DROP TABLE saved_attempts;").unwrap();
            if duplicate_history {
                conn.execute(
                    "INSERT INTO attempts SELECT * FROM attempts WHERE attempt_id='a1'",
                    [],
                )
                .unwrap();
            }
            drop(conn);
            let before = std::fs::read(&file).unwrap();
            assert!(BudgetStore::open_existing(&d.0, false).is_err());
            assert!(BudgetStore::open_existing(&d.0, true).is_err());
            assert!(budget.initialize().is_err());
            assert!(budget.configure("task", policy()).is_err());
            assert!(budget
                .record_usage(usage(&binding, "bad-layout", Some(1)))
                .is_err());
            assert!(budget.query("task").is_err());
            drop(budget);
            assert_eq!(std::fs::read(&file).unwrap(), before);
        }
    }
}
