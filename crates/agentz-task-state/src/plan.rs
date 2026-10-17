//! Durable plans are task/environment scoped, never scoped to a conversation turn.
//! This database is independent of runs.db: the host must coordinate execution.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanBinding {
    pub project_id: String,
    pub task_id: String,
    pub environment_id: String,
    pub binding_version: String,
}
impl PlanBinding {
    /// Copies identity only. A plan never grants execution or tool permission.
    pub fn from_execution(binding: &crate::approval::Binding) -> Result<Self, String> {
        binding.validate()?;
        Ok(Self {
            project_id: binding.project_id.clone(),
            task_id: binding.task_id.clone(),
            environment_id: binding.environment_id.clone(),
            binding_version: binding.binding_version.clone(),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    Pending,
    InProgress,
    Done,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanItem {
    pub id: String,
    pub description: String,
    pub state: ItemState,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Active,
    Completed,
    Cancelled,
    Replaced,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub binding: PlanBinding,
    pub revision: u64,
    pub objective: String,
    pub items: Vec<PlanItem>,
    pub status: PlanStatus,
    pub created_ms: u64,
    pub updated_ms: u64,
}
pub struct PlanStore {
    connection: Connection,
    project: PathBuf,
    project_id: String,
}
fn valid_text(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= 65536
}
fn validate_items(items: &[PlanItem]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    if items.len() > 500
        || items.iter().any(|item| {
            !valid_text(&item.id) || !valid_text(&item.description) || !ids.insert(&item.id)
        })
    {
        return Err("invalid or duplicate plan items".into());
    }
    Ok(())
}
fn status(items: &[PlanItem]) -> PlanStatus {
    if !items.is_empty()
        && items
            .iter()
            .all(|item| matches!(item.state, ItemState::Done | ItemState::Cancelled))
    {
        PlanStatus::Completed
    } else {
        PlanStatus::Active
    }
}
fn read(connection: &Connection, id: &str) -> Result<Option<Plan>, String> {
    let row: Option<(i64, String)> = connection
        .query_row(
            "SELECT revision, record FROM plans WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    row.map(|(revision, raw)| {
        let plan: Plan =
            serde_json::from_str(&raw).map_err(|e| format!("invalid plan record: {e}"))?;
        if plan.id != id || revision <= 0 || plan.revision != revision as u64 {
            return Err("plan record identity/revision mismatch".into());
        }
        validate_items(&plan.items)?;
        if !valid_text(&plan.id)
            || !valid_text(&plan.objective)
            || !valid_text(&plan.binding.project_id)
            || !valid_text(&plan.binding.task_id)
            || !valid_text(&plan.binding.environment_id)
            || plan.binding.binding_version.len() != 64
            || !plan
                .binding
                .binding_version
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
            || plan.updated_ms < plan.created_ms
            || plan.updated_ms > i64::MAX as u64
            || (matches!(plan.status, PlanStatus::Active | PlanStatus::Completed)
                && plan.status != status(&plan.items))
        {
            return Err("invalid plan record invariants".into());
        }
        Ok(plan)
    })
    .transpose()
}
fn current(connection: &Connection, binding: &PlanBinding) -> Result<Option<Plan>, String> {
    let id: Option<String> = connection
        .query_row(
            "SELECT plan_id FROM plan_heads WHERE task_id=?1 AND environment_id=?2",
            params![binding.task_id, binding.environment_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let plan = id
        .map(|id| read(connection, &id)?.ok_or_else(|| "missing current plan".to_string()))
        .transpose()?;
    if plan.as_ref().is_some_and(|plan| &plan.binding != binding) {
        return Err("plan binding changed; explicit environment handoff required".into());
    }
    if plan
        .as_ref()
        .is_some_and(|plan| plan.status == PlanStatus::Replaced)
    {
        return Err("current plan head references replaced history".into());
    }
    Ok(plan)
}
fn insert(connection: &Connection, plan: &Plan) -> Result<(), String> {
    let record = serde_json::to_string(plan).map_err(|e| e.to_string())?;
    connection
        .execute(
            "INSERT INTO plans(id,revision,record) VALUES(?1,?2,?3)",
            params![plan.id, plan.revision as i64, record],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn save(connection: &Connection, plan: &Plan, expected: u64) -> Result<(), String> {
    let record = serde_json::to_string(plan).map_err(|e| e.to_string())?;
    if connection
        .execute(
            "UPDATE plans SET revision=?1,record=?2 WHERE id=?3 AND revision=?4",
            params![plan.revision as i64, record, plan.id, expected as i64],
        )
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("stale plan revision".into());
    }
    Ok(())
}
fn require_current(
    connection: &Connection,
    binding: &PlanBinding,
    id: &str,
    expected: u64,
    now: u64,
) -> Result<Plan, String> {
    let plan = current(connection, binding)?.ok_or("no current plan")?;
    if plan.id != id || plan.revision != expected || expected >= i64::MAX as u64 {
        return Err("stale plan identity/revision".into());
    }
    if now < plan.updated_ms || now > i64::MAX as u64 {
        return Err("invalid plan timestamp".into());
    }
    Ok(plan)
}
impl PlanStore {
    /// Passive reads never create, initialize, migrate or repair a plan database.
    pub fn open_existing_readonly(project: &Path) -> Result<Self, String> {
        let project_id = crate::canonical_root(project)?;
        let path = PathBuf::from(&project_id).join(".agentz/plans.db");
        if !path.try_exists().map_err(|e| e.to_string())? {
            return Err("plan database missing".into());
        }
        let connection =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|e| e.to_string())?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        let tx = connection
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        let version: i64 = tx
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if version != 1 {
            return Err(format!(
                "unsupported or uninitialized plan schema {version}"
            ));
        }
        for (table, expected) in [
            (
                "plans",
                vec![
                    ("id", "TEXT", 1, 1),
                    ("revision", "INTEGER", 1, 0),
                    ("record", "TEXT", 1, 0),
                ],
            ),
            (
                "plan_heads",
                vec![
                    ("task_id", "TEXT", 1, 1),
                    ("environment_id", "TEXT", 1, 2),
                    ("plan_id", "TEXT", 1, 0),
                ],
            ),
        ] {
            let mut statement = tx
                .prepare(&format!("PRAGMA table_info({table})"))
                .map_err(|e| e.to_string())?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(5)?,
                    ))
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            let expected: Vec<_> = expected
                .into_iter()
                .map(|(n, t, nn, pk)| (n.to_owned(), t.to_owned(), nn, pk))
                .collect();
            if rows != expected {
                return Err(format!("invalid plan table layout: {table}"));
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(Self {
            connection,
            project: PathBuf::from(&project_id),
            project_id,
        })
    }
    pub fn open(project: &Path) -> Result<Self, String> {
        let project_id = crate::canonical_root(project)?;
        let directory = project.join(".agentz");
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let mut connection =
            Connection::open(directory.join("plans.db")).map_err(|e| e.to_string())?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| e.to_string())?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let version: u32 = tx
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if version > 1 {
            return Err(format!("plan schema {version} is newer than supported 1"));
        }
        if version == 0 {
            tx.execute_batch("CREATE TABLE plans(id TEXT PRIMARY KEY NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),record TEXT NOT NULL); CREATE TABLE plan_heads(task_id TEXT NOT NULL,environment_id TEXT NOT NULL,plan_id TEXT NOT NULL REFERENCES plans(id),PRIMARY KEY(task_id,environment_id)); PRAGMA user_version=1;").map_err(|e| e.to_string())?;
        }
        tx.prepare("SELECT id,revision,record FROM plans")
            .map_err(|e| e.to_string())?;
        tx.prepare("SELECT task_id,environment_id,plan_id FROM plan_heads")
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(Self {
            connection,
            project: PathBuf::from(&project_id),
            project_id,
        })
    }
    fn validate_binding(&self, binding: &PlanBinding) -> Result<(), String> {
        if binding.project_id != self.project_id
            || !valid_text(&binding.task_id)
            || !valid_text(&binding.environment_id)
            || binding.binding_version.len() != 64
            || !binding
                .binding_version
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid plan binding".into());
        }
        match crate::load(&self.project, &binding.task_id)? {
            Some(record) if record.environment_id == binding.environment_id => Ok(()),
            None if binding.environment_id == format!("direct:{}", binding.binding_version) => {
                Ok(())
            }
            _ => Err("plan task/environment mismatch".into()),
        }
    }
    /// Explicit continuation is read-only, including after cancellation.
    pub fn continue_plan(&self, binding: &PlanBinding) -> Result<Option<Plan>, String> {
        self.validate_binding(binding)?;
        current(&self.connection, binding)
    }
    pub fn load(&self, binding: &PlanBinding, id: &str) -> Result<Option<Plan>, String> {
        self.validate_binding(binding)?;
        let plan = read(&self.connection, id)?;
        if plan.as_ref().is_some_and(|plan| &plan.binding != binding) {
            return Err("plan task/environment mismatch".into());
        }
        Ok(plan)
    }
    pub fn create(
        &mut self,
        binding: &PlanBinding,
        id: &str,
        objective: &str,
        items: Vec<PlanItem>,
        now: u64,
    ) -> Result<Plan, String> {
        self.validate_binding(binding)?;
        let plan = Self::new_plan(binding, id, objective, items, now)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        if current(&tx, binding)?.is_some() {
            return Err("plan exists; explicit replacement required".into());
        }
        insert(&tx, &plan)?;
        tx.execute(
            "INSERT INTO plan_heads(task_id,environment_id,plan_id) VALUES(?1,?2,?3)",
            params![binding.task_id, binding.environment_id, id],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(plan)
    }
    fn new_plan(
        binding: &PlanBinding,
        id: &str,
        objective: &str,
        items: Vec<PlanItem>,
        now: u64,
    ) -> Result<Plan, String> {
        validate_items(&items)?;
        if !valid_text(id) || !valid_text(objective) || now > i64::MAX as u64 {
            return Err("invalid plan identity/objective/timestamp".into());
        }
        Ok(Plan {
            id: id.into(),
            binding: binding.clone(),
            revision: 1,
            objective: objective.into(),
            status: status(&items),
            items,
            created_ms: now,
            updated_ms: now,
        })
    }
    pub fn update(
        &mut self,
        binding: &PlanBinding,
        id: &str,
        expected: u64,
        items: Vec<PlanItem>,
        now: u64,
    ) -> Result<Plan, String> {
        self.validate_binding(binding)?;
        validate_items(&items)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let mut plan = require_current(&tx, binding, id, expected, now)?;
        if matches!(plan.status, PlanStatus::Cancelled | PlanStatus::Replaced) {
            return Err("closed plan requires explicit replacement".into());
        }
        plan.status = status(&items);
        plan.items = items;
        plan.revision += 1;
        plan.updated_ms = now;
        save(&tx, &plan, expected)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(plan)
    }
    pub fn replace(
        &mut self,
        binding: &PlanBinding,
        old_id: &str,
        expected: u64,
        new_id: &str,
        objective: &str,
        items: Vec<PlanItem>,
        now: u64,
    ) -> Result<Plan, String> {
        self.validate_binding(binding)?;
        let new = Self::new_plan(binding, new_id, objective, items, now)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let mut old = require_current(&tx, binding, old_id, expected, now)?;
        old.status = PlanStatus::Replaced;
        old.revision += 1;
        old.updated_ms = now;
        save(&tx, &old, expected)?;
        insert(&tx, &new)?;
        tx.execute(
            "UPDATE plan_heads SET plan_id=?1 WHERE task_id=?2 AND environment_id=?3",
            params![new_id, binding.task_id, binding.environment_id],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(new)
    }
    pub fn cancel(
        &mut self,
        binding: &PlanBinding,
        id: &str,
        expected: u64,
        now: u64,
    ) -> Result<Plan, String> {
        self.validate_binding(binding)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let mut plan = require_current(&tx, binding, id, expected, now)?;
        if matches!(plan.status, PlanStatus::Cancelled | PlanStatus::Replaced) {
            return Err("plan already closed".into());
        }
        plan.status = PlanStatus::Cancelled;
        plan.revision += 1;
        plan.updated_ms = now;
        save(&tx, &plan, expected)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(plan)
    }
    /// Explicit clear closes the plan and preserves history; it never clears on turn start.
    pub fn clear(
        &mut self,
        binding: &PlanBinding,
        id: &str,
        expected: u64,
        now: u64,
    ) -> Result<Plan, String> {
        self.cancel(binding, id, expected, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readonly_missing_database_creates_neither_directory_nor_file() {
        let (f, _) = fixture();
        assert!(PlanStore::open_existing_readonly(&f.0).is_err());
        assert!(!f.0.join(".agentz").exists());
    }
    #[test]
    fn readonly_uninitialized_future_partial_and_wrong_shape_do_not_repair() {
        for sql in [
            "PRAGMA user_version=0;",
            "PRAGMA user_version=2;",
            "PRAGMA user_version=1; CREATE TABLE plans(id TEXT);",
            "PRAGMA user_version=1; CREATE TABLE plans(id TEXT NOT NULL,revision INTEGER NOT NULL,record TEXT NOT NULL); CREATE TABLE plan_heads(task_id TEXT NOT NULL,environment_id TEXT NOT NULL,plan_id TEXT NOT NULL,PRIMARY KEY(task_id,environment_id));",
        ] {
            let (f, _) = fixture();
            std::fs::create_dir_all(f.0.join(".agentz")).unwrap();
            let path = f.0.join(".agentz/plans.db");
            Connection::open(&path).unwrap().execute_batch(sql).unwrap();
            let before = std::fs::read(&path).unwrap();
            assert!(PlanStore::open_existing_readonly(&f.0).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }
    #[test]
    fn readonly_full_cancelled_plan_is_unchanged_and_writes_are_rejected() {
        let (f, b) = fixture();
        let mut writer = PlanStore::open(&f.0).unwrap();
        writer.create(&b, "p", "goal", items(), 1).unwrap();
        let cancelled = writer.cancel(&b, "p", 1, 2).unwrap();
        drop(writer);
        let path = f.0.join(".agentz/plans.db");
        let before = std::fs::read(&path).unwrap();
        let mut reader = PlanStore::open_existing_readonly(&f.0).unwrap();
        assert_eq!(reader.continue_plan(&b).unwrap(), Some(cancelled.clone()));
        assert_eq!(reader.load(&b, "p").unwrap(), Some(cancelled));
        assert!(reader
            .replace(&b, "p", 2, "new", "new objective", vec![], 3)
            .is_err());
        drop(reader);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!f.0.join(".agentz/task-environments.db").exists());
    }
    #[test]
    fn readonly_corrupt_record_is_rejected_without_repair() {
        let (f, b) = fixture();
        PlanStore::open(&f.0)
            .unwrap()
            .create(&b, "p", "goal", items(), 1)
            .unwrap();
        let path = f.0.join(".agentz/plans.db");
        Connection::open(&path)
            .unwrap()
            .execute("UPDATE plans SET record='{}'", [])
            .unwrap();
        let before = std::fs::read(&path).unwrap();
        let reader = PlanStore::open_existing_readonly(&f.0).unwrap();
        assert!(reader.continue_plan(&b).is_err());
        drop(reader);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    #[test]
    fn mixed_done_cancelled_and_all_cancelled_items_are_completed_without_cancelling_plan() {
        let (f, b) = fixture();
        let mut store = PlanStore::open(&f.0).unwrap();
        let mut mixed = items();
        mixed[0].state = ItemState::Done;
        mixed.push(PlanItem {
            id: "omitted-step".into(),
            description: "explicitly cancelled step".into(),
            state: ItemState::Cancelled,
        });
        let completed = store.create(&b, "p", "goal", mixed.clone(), 1).unwrap();
        assert_eq!(completed.status, PlanStatus::Completed);
        let mut all_cancelled = mixed;
        for item in &mut all_cancelled {
            item.state = ItemState::Cancelled;
        }
        let updated = store.update(&b, "p", 1, all_cancelled.clone(), 2).unwrap();
        assert_eq!(updated.status, PlanStatus::Completed);
        drop(store);
        let mut reopened = PlanStore::open(&f.0).unwrap();
        let restored = reopened.continue_plan(&b).unwrap().unwrap();
        assert_eq!(restored.items, all_cancelled);
        assert_eq!(restored.status, PlanStatus::Completed);
        assert!(serde_json::to_string(&restored)
            .unwrap()
            .contains("\"state\":\"cancelled\""));
        let cancelled = reopened.cancel(&b, "p", 2, 3).unwrap();
        assert_eq!(cancelled.status, PlanStatus::Cancelled);
        assert_eq!(cancelled.items, all_cancelled);
    }
    #[test]
    fn corrupt_cancelled_item_status_and_plan_state_are_rejected() {
        let (f, b) = fixture();
        let mut store = PlanStore::open(&f.0).unwrap();
        let mut cancelled_items = items();
        cancelled_items[0].state = ItemState::Cancelled;
        let mut record = store.create(&b, "p", "goal", cancelled_items, 1).unwrap();
        record.status = PlanStatus::Active;
        store
            .connection
            .execute(
                "UPDATE plans SET record=?1 WHERE id='p'",
                [serde_json::to_string(&record).unwrap()],
            )
            .unwrap();
        assert!(store.continue_plan(&b).is_err());
        record.status = PlanStatus::Completed;
        let invalid = serde_json::to_string(&record)
            .unwrap()
            .replace("\"state\":\"cancelled\"", "\"state\":\"unknown\"");
        store
            .connection
            .execute("UPDATE plans SET record=?1 WHERE id='p'", [invalid])
            .unwrap();
        assert!(store.load(&b, "p").is_err());
    }
    #[test]
    fn concurrent_first_open_uses_one_schema_initialization() {
        let (f, _) = fixture();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let path = f.0.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    PlanStore::open(&path).map(|_| ())
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
    }
    #[test]
    fn corrupt_records_and_heads_fail_closed() {
        let (f, b) = fixture();
        let mut store = PlanStore::open(&f.0).unwrap();
        let original = store.create(&b, "p", "goal", items(), 5).unwrap();
        for corrupt in 0..5 {
            let mut record = original.clone();
            match corrupt {
                0 => record.objective = " ".into(),
                1 => record.updated_ms = 4,
                2 => record.status = PlanStatus::Completed,
                3 => record.binding.binding_version = "invalid".into(),
                _ => record.status = PlanStatus::Replaced,
            }
            store
                .connection
                .execute(
                    "UPDATE plans SET record=?1 WHERE id='p'",
                    [serde_json::to_string(&record).unwrap()],
                )
                .unwrap();
            assert!(store.continue_plan(&b).is_err());
            assert!(store.update(&b, "p", 1, items(), 6).is_err());
        }
        store
            .connection
            .execute("UPDATE plans SET record='invalid json' WHERE id='p'", [])
            .unwrap();
        assert!(store.load(&b, "p").is_err());
    }
    #[test]
    fn completed_plan_can_be_explicitly_extended_cleared_and_replaced() {
        let (f, b) = fixture();
        let mut store = PlanStore::open(&f.0).unwrap();
        let mut done = items();
        done[0].state = ItemState::Done;
        assert_eq!(
            store
                .create(&b, "p", "goal", done.clone(), 1)
                .unwrap()
                .status,
            PlanStatus::Completed
        );
        assert_eq!(
            store.update(&b, "p", 1, items(), 2).unwrap().status,
            PlanStatus::Active
        );
        store.update(&b, "p", 2, done.clone(), 3).unwrap();
        let cleared = store.clear(&b, "p", 3, 4).unwrap();
        assert_eq!(cleared.items, done);
        assert_eq!(cleared.status, PlanStatus::Cancelled);
        store
            .replace(&b, "p", 4, "q", "new goal", items(), 5)
            .unwrap();
        assert_eq!(store.continue_plan(&b).unwrap().unwrap().id, "q");
    }
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Fixture, PlanBinding) {
        let path = std::env::temp_dir().join(format!("agentz-plan-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        let binding = PlanBinding {
            project_id: crate::canonical_root(&path).unwrap(),
            task_id: "task".into(),
            environment_id: format!("direct:{}", "a".repeat(64)),
            binding_version: "a".repeat(64),
        };
        (Fixture(path), binding)
    }
    fn items() -> Vec<PlanItem> {
        vec![PlanItem {
            id: "step".into(),
            description: "verify changes".into(),
            state: ItemState::Pending,
        }]
    }
    #[test]
    fn restart_and_continuation_preserve_identity_revision_and_items() {
        let (f, b) = fixture();
        let original = PlanStore::open(&f.0)
            .unwrap()
            .create(&b, "plan", "goal", items(), 1)
            .unwrap();
        let reopened = PlanStore::open(&f.0).unwrap();
        assert_eq!(reopened.continue_plan(&b).unwrap(), Some(original));
        assert!(crate::load(&f.0, "task").unwrap().is_none());
    }
    #[test]
    fn two_connections_reject_stale_revision_and_task_or_environment() {
        let (f, b) = fixture();
        let mut first = PlanStore::open(&f.0).unwrap();
        first.create(&b, "p", "goal", items(), 1).unwrap();
        let mut second = PlanStore::open(&f.0).unwrap();
        first.update(&b, "p", 1, items(), 2).unwrap();
        assert!(second.update(&b, "p", 1, items(), 3).is_err());
        let mut other = b.clone();
        other.task_id = "other".into();
        assert!(second.load(&other, "p").is_err());
        other = b.clone();
        other.environment_id = "wrong".into();
        assert!(second.continue_plan(&other).is_err());
        other = b.clone();
        other.binding_version = "b".repeat(64);
        other.environment_id = format!("direct:{}", other.binding_version);
        assert!(second.load(&other, "p").is_err());
    }
    #[test]
    fn replacement_preserves_history_and_failed_replacement_rolls_back() {
        let (f, b) = fixture();
        let mut store = PlanStore::open(&f.0).unwrap();
        store.create(&b, "p", "old", items(), 1).unwrap();
        assert!(store.replace(&b, "p", 1, "p", "new", items(), 2).is_err());
        assert_eq!(store.continue_plan(&b).unwrap().unwrap().revision, 1);
        store
            .replace(&b, "p", 1, "new", "new goal", items(), 2)
            .unwrap();
        let old = store.load(&b, "p").unwrap().unwrap();
        assert_eq!(old.status, PlanStatus::Replaced);
        assert_eq!(old.revision, 2);
        assert_eq!(old.items, items());
        assert_eq!(store.continue_plan(&b).unwrap().unwrap().id, "new");
        assert!(store.update(&b, "p", 2, items(), 3).is_err());
    }
    #[test]
    fn explicit_clear_preserves_steps_and_never_reopens_on_continuation() {
        let (f, b) = fixture();
        let mut store = PlanStore::open(&f.0).unwrap();
        store.create(&b, "p", "goal", items(), 1).unwrap();
        store.clear(&b, "p", 1, 2).unwrap();
        let closed = store.continue_plan(&b).unwrap().unwrap();
        assert_eq!(closed.status, PlanStatus::Cancelled);
        assert_eq!(closed.items, items());
        assert!(store.update(&b, "p", 2, items(), 3).is_err());
        assert!(store.create(&b, "q", "new", items(), 3).is_err());
        store.replace(&b, "p", 2, "q", "new", items(), 3).unwrap();
    }
    #[test]
    fn invalid_items_and_time_do_not_mutate() {
        let (f, b) = fixture();
        let mut store = PlanStore::open(&f.0).unwrap();
        store.create(&b, "p", "goal", items(), 5).unwrap();
        let mut duplicate = items();
        duplicate.extend(items());
        assert!(store.update(&b, "p", 1, duplicate, 6).is_err());
        assert!(store.cancel(&b, "p", 1, 4).is_err());
        assert_eq!(store.continue_plan(&b).unwrap().unwrap().revision, 1);
    }
    #[test]
    fn newer_schema_is_rejected() {
        let (f, _) = fixture();
        drop(PlanStore::open(&f.0).unwrap());
        let connection = Connection::open(f.0.join(".agentz/plans.db")).unwrap();
        connection.pragma_update(None, "user_version", 2).unwrap();
        drop(connection);
        assert!(PlanStore::open(&f.0).is_err());
    }
    #[test]
    fn registered_environment_and_execution_identity_are_checked() {
        let (f, mut binding) = fixture();
        binding.environment_id = "managed-environment".into();
        crate::register(
            &f.0,
            &crate::TaskEnvironment {
                schema_version: 1,
                task_id: binding.task_id.clone(),
                environment_id: binding.environment_id.clone(),
                project_root: binding.project_id.clone(),
                workspace_root: binding.project_id.clone(),
                kind: "local".into(),
                branch: None,
                base_ref: String::new(),
                base_sha: String::new(),
                created_at: "fixture".into(),
            },
        )
        .unwrap();
        let mut store = PlanStore::open(&f.0).unwrap();
        store.create(&binding, "p", "goal", items(), 1).unwrap();
        let mut wrong = binding.clone();
        wrong.environment_id = "another".into();
        assert!(store.continue_plan(&wrong).is_err());
        wrong = binding.clone();
        wrong.binding_version = "b".repeat(64);
        assert!(store.continue_plan(&wrong).is_err());
        let mut execution = crate::approval::Binding {
            project_id: binding.project_id.clone(),
            task_id: binding.task_id.clone(),
            environment_id: binding.environment_id.clone(),
            binding_version: binding.binding_version.clone(),
            run_id: "run-1".into(),
            attempt_id: "attempt-1".into(),
            capability_version: "c".repeat(64),
        };
        assert_eq!(PlanBinding::from_execution(&execution).unwrap(), binding);
        execution.run_id = "run-2".into();
        execution.attempt_id = "attempt-2".into();
        assert_eq!(PlanBinding::from_execution(&execution).unwrap(), binding);
    }
}
