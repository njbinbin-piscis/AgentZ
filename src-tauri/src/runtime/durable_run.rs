//! Host-bound task execution persistence, including direct CodeZ snapshots.
use super::owner_lease::OwnerLease;
use agentz_task_state::inbox::{InboxStore, Notice, NoticeKind};
use agentz_task_state::run::{Execution, RunStore};
use std::path::Path;

pub struct ManagedRun {
    store: RunStore,
    run: String,
    attempt: String,
    state: Execution,
    finished: bool,
    binding: Option<agentz_task_state::approval::Binding>,
    /// Cross-process owner evidence for this attempt; released after the terminal state
    /// is persisted (fields drop after `Drop::drop`).
    lease: Option<OwnerLease>,
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
impl ManagedRun {
    pub fn queue_bound(
        project: &Path,
        binding: &agentz_task_state::approval::Binding,
    ) -> Result<Self, String> {
        binding.validate()?;
        if binding.project_id != agentz_task_state::canonical_root(project)? {
            return Err("queued run project binding mismatch".into());
        }
        if let Some(environment) = agentz_task_state::load(project, &binding.task_id)? {
            if environment.environment_id != binding.environment_id {
                return Err("queued run environment binding mismatch".into());
            }
        }
        let mut guard = if let Some(run) = Self::queue(
            project,
            &binding.task_id,
            &binding.run_id,
            &binding.attempt_id,
        )? {
            run
        } else {
            let mut store = RunStore::open(project)?;
            store.create_direct(binding, now())?;
            Self {
                store,
                run: binding.run_id.clone(),
                attempt: binding.attempt_id.clone(),
                state: Execution::Queued,
                finished: false,
                binding: None,
                lease: None,
            }
        };
        guard.attach_lease(project, binding)?;
        guard.binding = Some(binding.clone());
        Ok(guard)
    }
    /// Takes the cross-process owner lease before this attempt can be seen as running.
    /// On failure the guard is disarmed: a live owner elsewhere must never see its Run
    /// marked interrupted by this process's cleanup.
    fn attach_lease(
        &mut self,
        project: &Path,
        binding: &agentz_task_state::approval::Binding,
    ) -> Result<(), String> {
        match OwnerLease::acquire(project, binding) {
            Ok(lease) => {
                self.lease = Some(lease);
                Ok(())
            }
            Err(error) => {
                self.finished = true;
                Err(error)
            }
        }
    }
    pub fn start_bound(
        &mut self,
        binding: &agentz_task_state::approval::Binding,
    ) -> Result<(), String> {
        if self
            .binding
            .as_ref()
            .is_some_and(|expected| expected != binding)
        {
            return Err("queued run environment binding changed".into());
        }
        self.start()
    }
    pub fn begin_bound(
        project: &Path,
        binding: &agentz_task_state::approval::Binding,
    ) -> Result<Self, String> {
        let mut guard = if let Some(run) = Self::queue(
            project,
            &binding.task_id,
            &binding.run_id,
            &binding.attempt_id,
        )? {
            run
        } else {
            let mut store = RunStore::open(project)?;
            store.create_direct(binding, now())?;
            Self {
                store,
                run: binding.run_id.clone(),
                attempt: binding.attempt_id.clone(),
                state: Execution::Queued,
                finished: false,
                binding: None,
                lease: None,
            }
        };
        // Lease before Running: a crash at any point after this is attributable to a live owner.
        guard.attach_lease(project, binding)?;
        guard.start()?;
        guard.binding = Some(binding.clone());
        Ok(guard)
    }
    pub fn begin(
        project: &Path,
        task: &str,
        run: &str,
        attempt: &str,
    ) -> Result<Option<Self>, String> {
        let Some(mut guard) = Self::queue(project, task, run, attempt)? else {
            return Ok(None);
        };
        guard.start()?;
        Ok(Some(guard))
    }
    pub fn queue(
        project: &Path,
        task: &str,
        run: &str,
        attempt: &str,
    ) -> Result<Option<Self>, String> {
        let Some(environment) = agentz_task_state::load(project, task)? else {
            return Ok(None);
        };
        let mut store = RunStore::open(project)?;
        store.create(run, task, &environment.environment_id, attempt, now())?;
        let guard = Self {
            store,
            run: run.into(),
            attempt: attempt.into(),
            state: Execution::Queued,
            finished: false,
            binding: None,
            lease: None,
        };
        Ok(Some(guard))
    }
    pub fn identity(&self) -> (&str, &str) {
        (&self.run, &self.attempt)
    }
    /// Task and environment the run is bound to, when the run was queued against a task.
    pub fn task_binding(&self) -> Option<(&str, &str)> {
        self.binding
            .as_ref()
            .map(|b| (b.task_id.as_str(), b.environment_id.as_str()))
    }
    pub fn start(&mut self) -> Result<(), String> {
        self.move_to(Execution::Running)
    }
    fn move_to(&mut self, next: Execution) -> Result<(), String> {
        self.store
            .transition(&self.run, &self.attempt, self.state, next, now())?;
        self.state = next;
        Ok(())
    }
    pub fn finish(mut self, cancelled: bool, failed: bool, timed_out: bool) -> Result<(), String> {
        let terminal = if timed_out {
            Execution::Interrupted
        } else if cancelled {
            if self.state == Execution::Running {
                self.move_to(Execution::Cancelling)?;
            }
            Execution::Cancelled
        } else if failed {
            Execution::Failed
        } else {
            Execution::Succeeded
        };
        self.move_to(terminal)?;
        self.finished = true;
        if let Err(error) = self.record_terminal_notice(terminal) {
            tracing::warn!(run_id = %self.run, attempt_id = %self.attempt, %error,
                "terminal run persisted but notification inbox post failed");
        }
        Ok(())
    }
    fn record_terminal_notice(&self, terminal: Execution) -> Result<(), String> {
        let Some(binding) = &self.binding else {
            return Ok(());
        };
        let (kind, summary) = match terminal {
            Execution::Succeeded => (NoticeKind::Completed, "Task completed"),
            Execution::Failed => (NoticeKind::Failed, "Task failed"),
            Execution::Interrupted => (
                NoticeKind::ActionRequired,
                "Task interrupted; review before resuming",
            ),
            _ => return Ok(()),
        };
        let run = self.store.load(&self.run)?.ok_or("terminal run missing")?;
        if run.task_id != binding.task_id
            || run.attempt_id != binding.attempt_id
            || run.execution != terminal
        {
            return Err("terminal notification binding mismatch".into());
        }
        let notice = Notice {
            event_key: format!(
                "run-terminal:{}:{}:{}",
                self.run.len(),
                self.run,
                self.attempt
            ),
            task_id: binding.task_id.clone(),
            kind,
            summary: summary.into(),
            created_ms: run.updated_ms,
            read_ms: None,
        };
        InboxStore::open(Path::new(&binding.project_id))?.post(&notice)?;
        Ok(())
    }
    pub fn request_cancelling(&mut self) -> Result<(), String> {
        if self.state == Execution::Running {
            self.move_to(Execution::Cancelling)?;
        }
        Ok(())
    }
}
impl Drop for ManagedRun {
    fn drop(&mut self) {
        if !self.finished {
            if let Err(error) = self.move_to(Execution::Interrupted) {
                tracing::error!(run_id = %self.run, attempt_id = %self.attempt, %error,
                    "failed to persist interrupted managed run; reconciliation required");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_guard_persists_success_cancel_failure_and_early_drop() {
        let root =
            std::env::temp_dir().join(format!("agentz-managed-run-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let canonical = agentz_task_state::canonical_root(&root).unwrap();
        agentz_task_state::register(
            &root,
            &agentz_task_state::TaskEnvironment {
                schema_version: 1,
                task_id: "task".into(),
                environment_id: "env".into(),
                project_root: canonical.clone(),
                workspace_root: canonical,
                kind: "local".into(),
                branch: None,
                base_ref: String::new(),
                base_sha: String::new(),
                created_at: "fixture".into(),
            },
        )
        .unwrap();
        for (run, cancelled, failed, timeout, expected) in [
            ("success", false, false, false, Execution::Succeeded),
            ("cancel", true, false, false, Execution::Cancelled),
            ("failure", false, true, false, Execution::Failed),
            ("timeout", true, true, true, Execution::Interrupted),
        ] {
            ManagedRun::begin(&root, "task", run, run)
                .unwrap()
                .unwrap()
                .finish(cancelled, failed, timeout)
                .unwrap();
            assert_eq!(
                RunStore::open(&root)
                    .unwrap()
                    .load(run)
                    .unwrap()
                    .unwrap()
                    .execution,
                expected
            );
        }
        drop(
            ManagedRun::begin(&root, "task", "early", "early")
                .unwrap()
                .unwrap(),
        );
        let store = RunStore::open(&root).unwrap();
        assert_eq!(
            store.load("early").unwrap().unwrap().execution,
            Execution::Interrupted
        );
        assert_eq!(
            store.load("success").unwrap().unwrap().delivery,
            agentz_task_state::run::Delivery::AwaitingReview
        );
        assert!(ManagedRun::begin(&root, "unregistered", "direct", "direct")
            .unwrap()
            .is_none());
        assert!(store.load("direct").unwrap().is_none());
        let binding = agentz_task_state::approval::Binding {
            project_id: agentz_task_state::canonical_root(&root).unwrap(),
            task_id: "unregistered".into(),
            environment_id: format!("direct:{}", "b".repeat(64)),
            binding_version: "b".repeat(64),
            run_id: "direct-bound".into(),
            attempt_id: "direct-bound-attempt".into(),
            capability_version: "fixture".into(),
        };
        ManagedRun::begin_bound(&root, &binding)
            .unwrap()
            .finish(false, false, false)
            .unwrap();
        assert_eq!(
            store.load("direct-bound").unwrap().unwrap().execution,
            Execution::Succeeded
        );
        let inbox = InboxStore::open(&root).unwrap();
        let notices = inbox.recent(10).unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].task_id, "unregistered");
        assert_eq!(notices[0].kind, NoticeKind::Completed);
        drop(inbox);
        assert!(agentz_task_state::load(&root, "unregistered")
            .unwrap()
            .is_none());
        let mut queued_binding = binding.clone();
        queued_binding.run_id = "direct-queued".into();
        queued_binding.attempt_id = "direct-queued-attempt".into();
        let mut queued_direct = ManagedRun::queue_bound(&root, &queued_binding).unwrap();
        assert_eq!(
            store.load("direct-queued").unwrap().unwrap().execution,
            Execution::Queued
        );
        let mut changed = queued_binding.clone();
        changed.binding_version = "c".repeat(64);
        changed.environment_id = format!("direct:{}", changed.binding_version);
        assert!(queued_direct.start_bound(&changed).is_err());
        drop(queued_direct);
        assert_eq!(
            store.load("direct-queued").unwrap().unwrap().execution,
            Execution::Interrupted
        );
        let queued = ManagedRun::queue(&root, "task", "queued-cancel", "queued-attempt")
            .unwrap()
            .unwrap();
        assert_eq!(
            store.load("queued-cancel").unwrap().unwrap().execution,
            Execution::Queued
        );
        assert_eq!(queued.identity(), ("queued-cancel", "queued-attempt"));
        queued.finish(true, false, false).unwrap();
        assert_eq!(
            store.load("queued-cancel").unwrap().unwrap().execution,
            Execution::Cancelled
        );
        let mut queued = ManagedRun::queue(&root, "task", "queued-start", "start-attempt")
            .unwrap()
            .unwrap();
        queued.start().unwrap();
        assert!(queued.start().is_err());
        queued.finish(false, false, false).unwrap();
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
