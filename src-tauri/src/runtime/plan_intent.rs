//! Unregistered authority service. Host callers retain an environment read lease.
use agentz_task_state::plan::{Plan, PlanBinding, PlanStore};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Caller supplies a freshly host-validated binding while retaining its environment
/// lease. This acquires the actual same-host task gate itself, checks durable Run
/// activity, and retains exclusion through the synchronous plan CAS. It does not
/// attest renderer permissions or register an IPC entry point.
pub fn apply_under_task_gate(
    project: &Path,
    activity: &super::task_activity::TaskActivity,
    binding: &PlanBinding,
    expected: &ExpectedPlan,
    intent: PlanIntent,
    now: u64,
) -> Result<Plan, String> {
    let mut gate = ActiveRunGate::acquire(project, activity, binding)?;
    let mut store = PlanStore::open(project)?;
    apply(
        &mut store,
        &mut gate,
        || Ok(binding.clone()),
        expected,
        intent,
        now,
    )
}

struct ActiveRunGate {
    _guard: tokio::sync::OwnedMutexGuard<()>,
    binding: PlanBinding,
    runs: agentz_task_state::run::RunStore,
}
impl ActiveRunGate {
    fn acquire(
        project: &Path,
        activity: &super::task_activity::TaskActivity,
        binding: &PlanBinding,
    ) -> Result<Self, String> {
        if agentz_task_state::canonical_root(project)? != binding.project_id {
            return Err("plan mutation project binding mismatch".into());
        }
        if binding.task_id.is_empty() || binding.task_id.trim() != binding.task_id {
            return Err("plan mutation task binding invalid".into());
        }
        let guard = activity.try_enter(project, &binding.task_id)?;
        let runs = agentz_task_state::run::RunStore::open(project)?;
        Ok(Self {
            _guard: guard,
            binding: binding.clone(),
            runs,
        })
    }
}
impl ExclusiveTaskMutation for ActiveRunGate {
    fn require_idle(&mut self, binding: &PlanBinding) -> Result<(), String> {
        if binding != &self.binding {
            return Err("plan mutation task gate binding mismatch".into());
        }
        if self.runs.has_active_task(&binding.task_id)? {
            return Err("active or queued Run; stop execution first".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanQuery {
    pub binding: PlanBinding,
    /// Includes completed/cancelled current plan and its full persisted items.
    pub current: Option<Plan>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedPlan {
    pub id: String,
    pub revision: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlanIntent {
    Replace { objective: String },
    Cancel,
}

/// Implement only with a host-owned exclusive task mutation guard. The guard must
/// cover this call AND every Run queue/start path for this task; a stale boolean is
/// not a valid implementation. A fresh authoritative active/queued Run check occurs
/// while held; the same exclusion remains held through synchronous plan CAS.
trait ExclusiveTaskMutation {
    fn require_idle(&mut self, binding: &PlanBinding) -> Result<(), String>;
}

pub fn query(
    store: &PlanStore,
    fresh: impl FnOnce() -> Result<PlanBinding, String>,
) -> Result<PlanQuery, String> {
    let binding = fresh()?;
    let current = store.continue_plan(&binding)?;
    Ok(PlanQuery { binding, current })
}

/// Execute exactly once; the persisted return is authoritative, never auto-retry CAS.
/// Cancel neither stops a Run nor revokes execution grants. Replace is a new explicit
/// user objective, with host-generated identity and initially empty structured items.
fn apply(
    store: &mut PlanStore,
    gate: &mut impl ExclusiveTaskMutation,
    fresh: impl FnOnce() -> Result<PlanBinding, String>,
    expected: &ExpectedPlan,
    intent: PlanIntent,
    now: u64,
) -> Result<Plan, String> {
    if expected.id.trim().is_empty() || expected.revision == 0 {
        return Err("expected plan identity/revision required".into());
    }
    let binding = fresh()?;
    gate.require_idle(&binding)?;
    let current = store.continue_plan(&binding)?.ok_or("no current plan")?;
    if current.id != expected.id || current.revision != expected.revision {
        return Err("stale explicit plan identity/revision".into());
    }
    match intent {
        PlanIntent::Replace { objective } => {
            if objective.trim().is_empty() {
                return Err("explicit objective is empty".into());
            }
            let id = uuid::Uuid::new_v4().to_string();
            store.replace(
                &binding,
                &expected.id,
                expected.revision,
                &id,
                objective.trim(),
                Vec::new(),
                now,
            )
        }
        PlanIntent::Cancel => store.cancel(&binding, &expected.id, expected.revision, now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentz_task_state::plan::{ItemState, PlanItem, PlanStatus};

    #[test]
    fn actual_task_gate_rejects_queued_run_without_changing_plan() {
        let (directory, binding, store, expected) = fixture();
        let activity = super::super::task_activity::TaskActivity::default();
        let mut runs = agentz_task_state::run::RunStore::open(&directory.0).unwrap();
        runs.create(
            "run",
            &binding.task_id,
            &binding.environment_id,
            "attempt",
            1,
        )
        .unwrap();
        drop(runs);
        assert!(apply_under_task_gate(
            &directory.0,
            &activity,
            &binding,
            &expected,
            PlanIntent::Cancel,
            2
        )
        .unwrap_err()
        .contains("queued Run"));
        assert_eq!(store.continue_plan(&binding).unwrap().unwrap().revision, 1);
        assert!(activity.try_enter(&directory.0, &binding.task_id).is_ok());
    }

    #[test]
    fn actual_task_gate_busy_rejects_even_without_run_database() {
        let (directory, binding, store, expected) = fixture();
        let activity = super::super::task_activity::TaskActivity::default();
        let queued_guard = activity.try_enter(&directory.0, &binding.task_id).unwrap();
        assert!(!directory.0.join(".agentz/runs.db").exists());
        assert!(apply_under_task_gate(
            &directory.0,
            &activity,
            &binding,
            &expected,
            PlanIntent::Cancel,
            2
        )
        .is_err());
        assert!(!directory.0.join(".agentz/runs.db").exists());
        assert_eq!(store.continue_plan(&binding).unwrap().unwrap().revision, 1);
        drop(queued_guard);
    }

    #[test]
    fn actual_gate_covers_mutation_and_rejects_stale_corrupt_and_wrong_scope() {
        let (directory, binding, mut store, expected) = fixture();
        let activity = super::super::task_activity::TaskActivity::default();
        let mut gate = ActiveRunGate::acquire(&directory.0, &activity, &binding).unwrap();
        let concurrent_activity = activity.clone();
        let path = directory.0.clone();
        let task = binding.task_id.clone();
        let changed = apply(
            &mut store,
            &mut gate,
            || {
                // A real other thread using the actual queue exclusion key cannot enter
                // while the adapter's private guard protects its read+mutation scope.
                std::thread::spawn(move || {
                    assert!(concurrent_activity.try_enter(&path, &task).is_err());
                    assert!(concurrent_activity.try_enter(&path, "other-task").is_ok());
                })
                .join()
                .unwrap();
                Ok(binding.clone())
            },
            &expected,
            PlanIntent::Replace {
                objective: "new objective".into(),
            },
            2,
        )
        .unwrap();
        assert!(activity.try_enter(&directory.0, &binding.task_id).is_err());
        assert_ne!(changed.id, expected.id);
        let mut other = binding.clone();
        other.task_id = "other-task".into();
        assert!(gate.require_idle(&other).is_err());
        drop(gate);
        assert!(apply_under_task_gate(
            &directory.0,
            &activity,
            &binding,
            &expected,
            PlanIntent::Cancel,
            3
        )
        .is_err());
        let current = ExpectedPlan {
            id: changed.id.clone(),
            revision: changed.revision,
        };
        let mut runs = agentz_task_state::run::RunStore::open(&directory.0).unwrap();
        runs.create(
            "run",
            &binding.task_id,
            &binding.environment_id,
            "attempt",
            1,
        )
        .unwrap();
        let connection = rusqlite::Connection::open(directory.0.join(".agentz/runs.db")).unwrap();
        connection
            .execute("UPDATE runs SET execution='unknown' WHERE run_id='run'", [])
            .unwrap();
        assert!(apply_under_task_gate(
            &directory.0,
            &activity,
            &binding,
            &current,
            PlanIntent::Cancel,
            3
        )
        .is_err());
        assert_eq!(store.continue_plan(&binding).unwrap().unwrap(), changed);
        let mut wrong_project = binding.clone();
        wrong_project.project_id = "foreign-project".into();
        assert!(apply_under_task_gate(
            &directory.0,
            &activity,
            &wrong_project,
            &current,
            PlanIntent::Cancel,
            3
        )
        .is_err());
        assert!(activity.try_enter(&directory.0, &binding.task_id).is_ok());
    }

    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    struct OwnedFixtureGate {
        active: bool,
        checks: usize,
    }
    impl ExclusiveTaskMutation for OwnedFixtureGate {
        fn require_idle(&mut self, _: &PlanBinding) -> Result<(), String> {
            self.checks += 1;
            if self.active {
                Err("active or queued Run; stop execution first".into())
            } else {
                Ok(())
            }
        }
    }
    fn fixture() -> (Directory, PlanBinding, PlanStore, ExpectedPlan) {
        let directory = Directory(
            std::env::temp_dir().join(format!("agentz-plan-intent-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir_all(&directory.0).unwrap();
        let binding = PlanBinding {
            project_id: agentz_task_state::canonical_root(&directory.0).unwrap(),
            task_id: "task".into(),
            environment_id: "managed-fixture-environment".into(),
            binding_version: "a".repeat(64),
        };
        // RunStore::create is a managed lifecycle entry and requires the actual
        // environment registry. Do not bypass this production authority check.
        agentz_task_state::register(
            &directory.0,
            &agentz_task_state::TaskEnvironment {
                schema_version: 1,
                task_id: binding.task_id.clone(),
                environment_id: binding.environment_id.clone(),
                project_root: binding.project_id.clone(),
                workspace_root: binding.project_id.clone(),
                kind: "local".into(),
                branch: None,
                base_ref: String::new(),
                base_sha: String::new(),
                created_at: "owned-fixture".into(),
            },
        )
        .unwrap();
        let mut store = PlanStore::open(&directory.0).unwrap();
        store
            .create(
                &binding,
                "p",
                "first goal",
                vec![PlanItem {
                    id: "one".into(),
                    description: "step".into(),
                    state: ItemState::Pending,
                }],
                1,
            )
            .unwrap();
        (
            directory,
            binding,
            store,
            ExpectedPlan {
                id: "p".into(),
                revision: 1,
            },
        )
    }
    #[test]
    fn active_and_stale_intents_have_no_effect() {
        let (_directory, b, mut store, expected) = fixture();
        let before = query(&store, || Ok(b.clone())).unwrap();
        let mut gate = OwnedFixtureGate {
            active: true,
            checks: 0,
        };
        assert!(apply(
            &mut store,
            &mut gate,
            || Ok(b.clone()),
            &expected,
            PlanIntent::Cancel,
            2
        )
        .is_err());
        gate.active = false;
        let stale = ExpectedPlan {
            id: "other".into(),
            revision: 1,
        };
        assert!(apply(
            &mut store,
            &mut gate,
            || Ok(b.clone()),
            &stale,
            PlanIntent::Replace {
                objective: "new".into()
            },
            2
        )
        .is_err());
        assert_eq!(before, query(&store, || Ok(b)).unwrap());
        assert_eq!(gate.checks, 2);
    }
    #[test]
    fn failed_replace_rolls_back_history_and_reopen_query_keeps_closed_items() {
        let (directory, b, mut store, expected) = fixture();
        let mut gate = OwnedFixtureGate {
            active: false,
            checks: 0,
        };
        let connection = rusqlite::Connection::open(directory.0.join(".agentz/plans.db")).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_new_plan BEFORE INSERT ON plans BEGIN SELECT RAISE(ABORT,'owned test failure'); END;").unwrap();
        assert!(apply(
            &mut store,
            &mut gate,
            || Ok(b.clone()),
            &expected,
            PlanIntent::Replace {
                objective: "new".into()
            },
            2
        )
        .is_err());
        let unchanged = query(&store, || Ok(b.clone())).unwrap().current.unwrap();
        assert_eq!(unchanged.revision, 1);
        assert_eq!(unchanged.status, PlanStatus::Active);
        connection
            .execute_batch("DROP TRIGGER reject_new_plan;")
            .unwrap();
        apply(
            &mut store,
            &mut gate,
            || Ok(b.clone()),
            &expected,
            PlanIntent::Cancel,
            2,
        )
        .unwrap();
        drop(store);
        let reopened = PlanStore::open(&directory.0).unwrap();
        let closed = query(&reopened, || Ok(b)).unwrap().current.unwrap();
        assert_eq!(closed.status, PlanStatus::Cancelled);
        assert_eq!(closed.items, unchanged.items);
        assert_eq!(closed.revision, 2);
    }
    #[test]
    fn explicit_replace_has_new_identity_and_retains_old_history() {
        let (_directory, b, mut store, expected) = fixture();
        let mut gate = OwnedFixtureGate {
            active: false,
            checks: 0,
        };
        let new = apply(
            &mut store,
            &mut gate,
            || Ok(b.clone()),
            &expected,
            PlanIntent::Replace {
                objective: " second goal ".into(),
            },
            2,
        )
        .unwrap();
        assert_ne!(new.id, expected.id);
        assert_eq!(new.objective, "second goal");
        assert!(new.items.is_empty());
        assert_eq!(
            store.load(&b, "p").unwrap().unwrap().status,
            PlanStatus::Replaced
        );
        assert!(apply(
            &mut store,
            &mut gate,
            || Ok(b),
            &expected,
            PlanIntent::Cancel,
            3
        )
        .is_err());
    }
}
