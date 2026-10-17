//! Finalization of an already requested cancellation for local cooperative workers.
//! Capturing a cancellation flag is not proof that a process/tool has terminated.
use agentz_task_state::approval::Binding;
use piscis_kernel::agent::tool_completion::{for_cancel, DrainOutcome, OwnedCompletionRegistry};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub struct CancellationCompletion {
    binding: Binding,
    cancel: Arc<AtomicBool>,
    registry: Arc<OwnedCompletionRegistry>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// All registered LOCAL cooperative workers acknowledged their cleanup.
    /// This does not attest arbitrary external/remote actions or model billing.
    Confirmed,
    Pending {
        active: usize,
    },
    Failed {
        active: usize,
        failures: usize,
    },
}
impl Completion {
    pub fn may_finalize_cancelled(&self) -> bool {
        matches!(self, Self::Confirmed)
    }
}
impl CancellationCompletion {
    /// Capture before tool work starts, with the fresh unique cancellation Arc
    /// belonging to this Attempt. Do not reuse that Arc for retries/other tasks.
    pub fn capture(binding: &Binding, cancel: &Arc<AtomicBool>) -> Result<Self, String> {
        binding.validate()?;
        Ok(Self {
            binding: binding.clone(),
            cancel: cancel.clone(),
            registry: for_cancel(cancel),
        })
    }
    /// Caller retains task exclusion/slot/environment leases through this await
    /// AND durable terminal commit. Pending/Failed must become Interrupted, never
    /// Cancelled; retained registry receipts continue to account for live workers.
    /// The timeout does not kill a worker or release unknown budget reservations.
    pub async fn wait_requested(
        &self,
        binding: &Binding,
        limit: Duration,
    ) -> Result<Completion, String> {
        binding.validate()?;
        if binding != &self.binding {
            return Err("cancellation completion Attempt binding mismatch".into());
        }
        if !self.cancel.load(Ordering::SeqCst) {
            return Err("cancellation was not requested".into());
        }
        if limit.is_zero() || limit > Duration::from_secs(60) {
            return Err("cancellation drain limit must be positive and at most 60 seconds".into());
        }
        Ok(match self.registry.drain(limit).await {
            DrainOutcome::Complete => Completion::Confirmed,
            DrainOutcome::Pending { active } => Completion::Pending { active },
            DrainOutcome::Failed { active, failures } => Completion::Failed { active, failures },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::task_activity::TaskActivity;
    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let p = std::env::temp_dir()
                .join(format!("agentz-cancel-completion-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn binding(&self) -> Binding {
            Binding {
                project_id: agentz_task_state::canonical_root(&self.0).unwrap(),
                task_id: "task".into(),
                environment_id: "env".into(),
                binding_version: "a".repeat(64),
                run_id: "run".into(),
                attempt_id: "attempt".into(),
                capability_version: "fixture".into(),
            }
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[tokio::test]
    async fn delayed_worker_cleanup_keeps_real_task_gate_and_slot_until_confirmation() {
        let dir = Directory::new();
        let binding = dir.binding();
        let cancel = Arc::new(AtomicBool::new(false));
        let completion = CancellationCompletion::capture(&binding, &cancel).unwrap();
        let receipt = for_cancel(&cancel).register().unwrap();
        let activity = TaskActivity::default();
        let gate = activity.try_enter(&dir.0, "task").unwrap();
        let slots = Arc::new(tokio::sync::Semaphore::new(1));
        let slot = slots.clone().acquire_owned().await.unwrap();
        let (release, ready) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(async move {
            ready.await.unwrap();
            receipt.complete(Ok(()));
        });
        cancel.store(true, Ordering::SeqCst);
        let wait = completion.wait_requested(&binding, Duration::from_secs(1));
        tokio::pin!(wait);
        assert!(tokio::time::timeout(Duration::from_millis(20), &mut wait)
            .await
            .is_err());
        assert!(activity.try_enter(&dir.0, "task").is_err());
        assert!(slots.clone().try_acquire_owned().is_err());
        release.send(()).unwrap();
        assert!(wait.await.unwrap().may_finalize_cancelled());
        worker.await.unwrap();
        // Only caller releases these guards after confirmation/terminal commit.
        assert!(activity.try_enter(&dir.0, "task").is_err());
        drop(slot);
        drop(gate);
        assert!(slots.try_acquire_owned().is_ok());
        assert!(activity.try_enter(&dir.0, "task").is_ok());
    }
    #[tokio::test]
    async fn timeout_and_failed_receipt_never_claim_cancelled() {
        let dir = Directory::new();
        let binding = dir.binding();
        let cancel = Arc::new(AtomicBool::new(false));
        let completion = CancellationCompletion::capture(&binding, &cancel).unwrap();
        let receipt = for_cancel(&cancel).register().unwrap();
        cancel.store(true, Ordering::SeqCst);
        let result = completion
            .wait_requested(&binding, Duration::from_millis(10))
            .await
            .unwrap();
        assert_eq!(result, Completion::Pending { active: 1 });
        assert!(!result.may_finalize_cancelled());
        receipt.complete(Err("cleanup failed".into()));
        let result = completion
            .wait_requested(&binding, Duration::from_millis(10))
            .await
            .unwrap();
        assert!(!result.may_finalize_cancelled());
        assert!(matches!(result, Completion::Failed { .. }));
    }
    #[tokio::test]
    async fn wrong_attempt_unrequested_flag_and_bad_limit_are_rejected() {
        let dir = Directory::new();
        let binding = dir.binding();
        let cancel = Arc::new(AtomicBool::new(false));
        let completion = CancellationCompletion::capture(&binding, &cancel).unwrap();
        assert!(completion
            .wait_requested(&binding, Duration::from_secs(1))
            .await
            .is_err());
        // Unrequested rejection must not seal or prevent legitimate tool starts.
        let receipt = for_cancel(&cancel).register().unwrap();
        receipt.complete(Ok(()));
        cancel.store(true, Ordering::SeqCst);
        let mut other = binding.clone();
        other.attempt_id = "other".into();
        assert!(completion
            .wait_requested(&other, Duration::from_secs(1))
            .await
            .is_err());
        assert!(completion
            .wait_requested(&binding, Duration::ZERO)
            .await
            .is_err());
        assert_eq!(
            completion
                .wait_requested(&binding, Duration::from_secs(1))
                .await
                .unwrap(),
            Completion::Confirmed
        );
    }
}
