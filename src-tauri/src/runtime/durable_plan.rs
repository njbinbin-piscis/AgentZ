//! Draft host service. Not exported or registered: plans never grant tool permission.
use agentz_task_state::plan::{ItemState, Plan, PlanBinding, PlanItem, PlanStatus, PlanStore};
use piscis_kernel::agent::plan::{merge_todos, validate_todos, PlanTodoItem};

pub trait PlanRepository {
    fn current(&self, binding: &PlanBinding) -> Result<Option<Plan>, String>;
    fn create(
        &mut self,
        binding: &PlanBinding,
        id: &str,
        objective: &str,
        items: Vec<PlanItem>,
        now: u64,
    ) -> Result<Plan, String>;
    fn update(
        &mut self,
        binding: &PlanBinding,
        id: &str,
        expected: u64,
        items: Vec<PlanItem>,
        now: u64,
    ) -> Result<Plan, String>;
}
impl PlanRepository for PlanStore {
    fn current(&self, binding: &PlanBinding) -> Result<Option<Plan>, String> {
        self.continue_plan(binding)
    }
    fn create(
        &mut self,
        binding: &PlanBinding,
        id: &str,
        objective: &str,
        items: Vec<PlanItem>,
        now: u64,
    ) -> Result<Plan, String> {
        self.create(binding, id, objective, items, now)
    }
    fn update(
        &mut self,
        binding: &PlanBinding,
        id: &str,
        expected: u64,
        items: Vec<PlanItem>,
        now: u64,
    ) -> Result<Plan, String> {
        self.update(binding, id, expected, items, now)
    }
}

pub fn to_kernel(items: &[PlanItem]) -> Vec<PlanTodoItem> {
    items
        .iter()
        .map(|item| PlanTodoItem {
            id: item.id.clone(),
            content: item.description.clone(),
            status: match item.state {
                ItemState::Pending => "pending",
                ItemState::InProgress => "in_progress",
                ItemState::Done => "completed",
                ItemState::Cancelled => "cancelled",
            }
            .into(),
        })
        .collect()
}
pub fn from_kernel(items: &[PlanTodoItem]) -> Result<Vec<PlanItem>, String> {
    let normalized: Vec<_> = items.iter().map(PlanTodoItem::normalized).collect();
    validate_todos(&normalized)?;
    normalized
        .into_iter()
        .map(|item| {
            Ok(PlanItem {
                id: item.id,
                description: item.content,
                state: match item.status.as_str() {
                    "pending" => ItemState::Pending,
                    "in_progress" => ItemState::InProgress,
                    "completed" => ItemState::Done,
                    "cancelled" => ItemState::Cancelled,
                    _ => return Err("unknown plan item state".into()),
                },
            })
        })
        .collect()
}

/// Turn-local CAS token; fresh reads must never silently advance its revision.
pub struct DurablePlanSession {
    binding: PlanBinding,
    captured: Option<(String, u64)>,
    projection: Vec<PlanTodoItem>,
}
impl DurablePlanSession {
    /// Caller supplies a fresh trusted resolver result. No session/run authorization is inferred here.
    pub fn recover(repository: &impl PlanRepository, binding: PlanBinding) -> Result<Self, String> {
        let plan = repository.current(&binding)?;
        validate_current(plan.as_ref(), &binding)?;
        Ok(Self {
            binding,
            captured: plan.as_ref().map(|p| (p.id.clone(), p.revision)),
            projection: plan.as_ref().map(project).unwrap_or_default(),
        })
    }
    pub fn projection(&self) -> &[PlanTodoItem] {
        &self.projection
    }
    pub fn captured_revision(&self) -> Option<(&str, u64)> {
        self.captured
            .as_ref()
            .map(|(id, revision)| (id.as_str(), *revision))
    }
    /// Run/environment lease and fresh binding resolution happen in the caller before this operation.
    /// Only a successful database return advances this token/cache; caller emits after success.
    pub fn write(
        &mut self,
        repository: &mut impl PlanRepository,
        fresh: impl FnOnce() -> Result<PlanBinding, String>,
        updates: &[PlanTodoItem],
        merge: bool,
        new_id: &str,
        objective: &str,
        now: u64,
    ) -> Result<Plan, String> {
        if fresh()? != self.binding {
            return Err("plan binding changed during turn".into());
        }
        let current = repository.current(&self.binding)?;
        validate_current(current.as_ref(), &self.binding)?;
        let identity = current.as_ref().map(|p| (p.id.clone(), p.revision));
        if identity != self.captured {
            return Err("stale captured plan identity/revision".into());
        }
        if current
            .as_ref()
            .is_some_and(|p| matches!(p.status, PlanStatus::Cancelled | PlanStatus::Replaced))
        {
            return Err("closed plan requires explicit replacement".into());
        }
        let normalized = to_kernel(&from_kernel(updates)?);
        let proposed = if merge {
            merge_todos(
                &current
                    .as_ref()
                    .map(|p| to_kernel(&p.items))
                    .unwrap_or_default(),
                &normalized,
            )
        } else {
            // A replacement must not silently drop work that is still open. Callers either
            // merge the open items or mark them cancelled first.
            let dropped_open = current
                .as_ref()
                .map(|plan| {
                    plan.items
                        .iter()
                        .filter(|item| {
                            matches!(
                                item.state,
                                agentz_task_state::plan::ItemState::Pending
                                    | agentz_task_state::plan::ItemState::InProgress
                            ) && !normalized.iter().any(|update| update.id == item.id)
                        })
                        .count()
                })
                .unwrap_or(0);
            if dropped_open > 0 {
                return Err(format!(
                    "replacement would drop {dropped_open} unfinished plan item(s); send merge=true to update them, or mark them cancelled before replacing"
                ));
            }
            normalized
        };
        let items = from_kernel(&proposed)?;
        let committed = match &self.captured {
            Some((id, revision)) => repository.update(&self.binding, id, *revision, items, now)?,
            None => repository.create(&self.binding, new_id, objective, items, now)?,
        };
        self.captured = Some((committed.id.clone(), committed.revision));
        self.projection = project(&committed);
        Ok(committed)
    }
}
fn validate_current(plan: Option<&Plan>, binding: &PlanBinding) -> Result<(), String> {
    if let Some(plan) = plan {
        if &plan.binding != binding {
            return Err("repository returned a different plan binding".into());
        }
        if !plan.items.is_empty() {
            validate_todos(&to_kernel(&plan.items))?;
        }
    }
    Ok(())
}
fn project(plan: &Plan) -> Vec<PlanTodoItem> {
    if matches!(plan.status, PlanStatus::Cancelled | PlanStatus::Replaced) {
        Vec::new()
    } else {
        to_kernel(&plan.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn sqlite_fixture() -> (Directory, PlanBinding) {
        let directory = Directory(
            std::env::temp_dir().join(format!("agentz-host-plan-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir_all(&directory.0).unwrap();
        let binding = PlanBinding {
            project_id: agentz_task_state::canonical_root(&directory.0).unwrap(),
            task_id: "task".into(),
            environment_id: format!("direct:{}", "a".repeat(64)),
            binding_version: "a".repeat(64),
        };
        (directory, binding)
    }
    #[test]
    fn sqlite_restart_stale_revision_and_trigger_failure_preserve_projection() {
        let (directory, b) = sqlite_fixture();
        let mut repository = PlanStore::open(&directory.0).unwrap();
        let mut old = DurablePlanSession::recover(&repository, b.clone()).unwrap();
        old.write(
            &mut repository,
            || Ok(b.clone()),
            &todos(),
            false,
            "p",
            "goal",
            1,
        )
        .unwrap();
        drop(repository);
        let mut reopened = PlanStore::open(&directory.0).unwrap();
        let mut next = DurablePlanSession::recover(&reopened, b.clone()).unwrap();
        assert_eq!(next.projection(), todos());
        next.write(
            &mut reopened,
            || Ok(b.clone()),
            &todos(),
            true,
            "unused",
            "unused",
            2,
        )
        .unwrap();
        assert!(old
            .write(
                &mut reopened,
                || Ok(b.clone()),
                &todos(),
                false,
                "unused",
                "unused",
                3
            )
            .is_err());
        let before = next.projection().to_vec();
        let connection = rusqlite::Connection::open(directory.0.join(".agentz/plans.db")).unwrap();
        connection.execute_batch("CREATE TRIGGER deny_plan_update BEFORE UPDATE ON plans BEGIN SELECT RAISE(ABORT,'owned test failure'); END;").unwrap();
        let mut changed = todos();
        changed[0].content = "not committed".into();
        assert!(next
            .write(
                &mut reopened,
                || Ok(b.clone()),
                &changed,
                false,
                "unused",
                "unused",
                3
            )
            .is_err());
        assert_eq!(next.projection(), before);
        assert_eq!(next.captured_revision(), Some(("p", 2)));
        assert_eq!(
            reopened.continue_plan(&b).unwrap().unwrap().items[0].description,
            "step"
        );
    }
    #[test]
    fn sqlite_multiple_in_progress_and_wrong_repository_binding_fail_recovery() {
        let (directory, b) = sqlite_fixture();
        let mut repository = PlanStore::open(&directory.0).unwrap();
        let items = vec![
            PlanItem {
                id: "a".into(),
                description: "a".into(),
                state: ItemState::InProgress,
            },
            PlanItem {
                id: "b".into(),
                description: "b".into(),
                state: ItemState::InProgress,
            },
        ];
        let mut plan = repository.create(&b, "p", "goal", items, 1).unwrap();
        assert!(DurablePlanSession::recover(&repository, b.clone()).is_err());
        plan.binding.environment_id = "different".into();
        let fake = Fake {
            plan: Some(plan),
            fail: false,
            writes: 0,
        };
        assert!(DurablePlanSession::recover(&fake, b).is_err());
    }
    struct Fake {
        plan: Option<Plan>,
        fail: bool,
        writes: usize,
    }
    impl PlanRepository for Fake {
        fn current(&self, _: &PlanBinding) -> Result<Option<Plan>, String> {
            Ok(self.plan.clone())
        }
        fn create(
            &mut self,
            b: &PlanBinding,
            id: &str,
            objective: &str,
            items: Vec<PlanItem>,
            now: u64,
        ) -> Result<Plan, String> {
            if self.fail {
                return Err("disk failed".into());
            }
            let p = Plan {
                id: id.into(),
                binding: b.clone(),
                revision: 1,
                objective: objective.into(),
                items,
                status: PlanStatus::Active,
                created_ms: now,
                updated_ms: now,
            };
            self.writes += 1;
            self.plan = Some(p.clone());
            Ok(p)
        }
        fn update(
            &mut self,
            _: &PlanBinding,
            id: &str,
            expected: u64,
            items: Vec<PlanItem>,
            now: u64,
        ) -> Result<Plan, String> {
            if self.fail {
                return Err("disk failed".into());
            }
            let p = self.plan.as_mut().ok_or("missing")?;
            if p.id != id || p.revision != expected {
                return Err("stale".into());
            }
            p.revision += 1;
            p.items = items;
            p.updated_ms = now;
            self.writes += 1;
            Ok(p.clone())
        }
    }
    fn binding() -> PlanBinding {
        PlanBinding {
            project_id: "project".into(),
            task_id: "task".into(),
            environment_id: "env".into(),
            binding_version: "a".repeat(64),
        }
    }
    fn todos() -> Vec<PlanTodoItem> {
        vec![PlanTodoItem {
            id: "a".into(),
            content: "step".into(),
            status: "pending".into(),
        }]
    }
    fn todo_items(entries: &[(&str, &str)]) -> Vec<PlanTodoItem> {
        entries
            .iter()
            .map(|(id, status)| PlanTodoItem {
                id: (*id).into(),
                content: format!("step {id}"),
                status: (*status).into(),
            })
            .collect()
    }

    /// Restart, then a continuation that lists only one step. The open sibling must survive
    /// an omitted-merge write, and a replacement that would drop it must be refused.
    #[test]
    fn replacement_cannot_drop_open_items_across_restart() {
        let (directory, b) = sqlite_fixture();
        let mut repository = PlanStore::open(&directory.0).unwrap();
        let mut session = DurablePlanSession::recover(&repository, b.clone()).unwrap();
        session
            .write(
                &mut repository,
                || Ok(b.clone()),
                &todo_items(&[("one", "pending"), ("two", "in_progress")]),
                false,
                "p",
                "goal",
                1,
            )
            .unwrap();
        drop(session);
        drop(repository);

        let mut repository = PlanStore::open(&directory.0).unwrap();
        let mut session = DurablePlanSession::recover(&repository, b.clone()).unwrap();
        assert!(session
            .write(
                &mut repository,
                || Ok(b.clone()),
                &todo_items(&[("one", "completed")]),
                false,
                "unused",
                "unused",
                2,
            )
            .is_err());
        assert_eq!(
            repository.continue_plan(&b).unwrap().unwrap().items.len(),
            2,
            "a refused replacement must leave the plan unchanged"
        );

        session
            .write(
                &mut repository,
                || Ok(b.clone()),
                &todo_items(&[("one", "completed")]),
                true,
                "unused",
                "unused",
                3,
            )
            .unwrap();
        let plan = repository.continue_plan(&b).unwrap().unwrap();
        assert_eq!(plan.items.len(), 2);
        assert_eq!(plan.items[0].state, ItemState::Done);
        assert_eq!(plan.items[1].id, "two");
        assert_eq!(plan.items[1].state, ItemState::InProgress);

        session
            .write(
                &mut repository,
                || Ok(b.clone()),
                &todo_items(&[("one", "completed"), ("two", "cancelled")]),
                false,
                "unused",
                "unused",
                4,
            )
            .unwrap();
    }

    #[test]
    fn cancelled_mapping_and_unknown_rejection() {
        let mut t = todos();
        t[0].status = " cancelled ".into();
        assert_eq!(from_kernel(&t).unwrap()[0].state, ItemState::Cancelled);
        assert_eq!(to_kernel(&from_kernel(&t).unwrap())[0].status, "cancelled");
        t[0].status = "unknown".into();
        assert!(from_kernel(&t).is_err());
    }
    #[test]
    fn failed_commit_and_fresh_binding_leave_projection_unchanged() {
        let mut r = Fake {
            plan: None,
            fail: true,
            writes: 0,
        };
        let mut s = DurablePlanSession::recover(&r, binding()).unwrap();
        assert!(s
            .write(&mut r, || Ok(binding()), &todos(), false, "p", "goal", 1)
            .is_err());
        assert!(s.projection().is_empty());
        assert!(s.captured_revision().is_none());
        r.fail = false;
        let mut different = binding();
        different.environment_id = "other".into();
        assert!(s
            .write(&mut r, || Ok(different), &todos(), false, "p", "goal", 1)
            .is_err());
        assert_eq!(r.writes, 0);
    }
    #[test]
    fn old_turn_cannot_adopt_new_revision_and_recovery_reads_database() {
        let mut r = Fake {
            plan: None,
            fail: false,
            writes: 0,
        };
        let mut old = DurablePlanSession::recover(&r, binding()).unwrap();
        old.write(&mut r, || Ok(binding()), &todos(), false, "p", "goal", 1)
            .unwrap();
        let mut next = DurablePlanSession::recover(&r, binding()).unwrap();
        next.write(
            &mut r,
            || Ok(binding()),
            &todos(),
            true,
            "ignored",
            "ignored",
            2,
        )
        .unwrap();
        assert!(old
            .write(
                &mut r,
                || Ok(binding()),
                &todos(),
                true,
                "ignored",
                "ignored",
                3
            )
            .is_err());
        assert_eq!(old.captured_revision(), Some(("p", 1)));
        assert_eq!(r.writes, 2);
        let recovered = DurablePlanSession::recover(&r, binding()).unwrap();
        assert_eq!(recovered.captured_revision(), Some(("p", 2)));
    }
    #[test]
    fn cancelled_plan_cannot_be_reopened_by_tool() {
        let mut r = Fake {
            plan: None,
            fail: false,
            writes: 0,
        };
        let mut s = DurablePlanSession::recover(&r, binding()).unwrap();
        s.write(&mut r, || Ok(binding()), &todos(), false, "p", "goal", 1)
            .unwrap();
        r.plan.as_mut().unwrap().status = PlanStatus::Cancelled;
        let mut closed = DurablePlanSession::recover(&r, binding()).unwrap();
        assert!(closed.projection().is_empty());
        assert!(closed
            .write(&mut r, || Ok(binding()), &todos(), false, "p", "goal", 2)
            .is_err());
        assert_eq!(r.writes, 1);
    }
}
