//! Owned worker completion for one captured cancellation identity. No OS authorization.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock, Weak,
    },
    time::Duration,
};

#[derive(Debug, Default)]
struct State {
    active: usize,
    failures: usize,
    sealed: bool,
}
#[derive(Debug)]
pub struct OwnedCompletionRegistry {
    cancel: Arc<AtomicBool>,
    state: Mutex<State>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainOutcome {
    Complete,
    Pending { active: usize },
    Failed { active: usize, failures: usize },
}
/// The exact Arc identifies one host attempt; a registry retains it to prevent address reuse.
/// Hosts must allocate fresh cancellation flags per attempt and retain this registry before
/// launching tools. Do not derive ownership from session names or model supplied identifiers.
pub fn for_cancel(cancel: &Arc<AtomicBool>) -> Arc<OwnedCompletionRegistry> {
    static REGISTRIES: OnceLock<Mutex<HashMap<usize, Weak<OwnedCompletionRegistry>>>> =
        OnceLock::new();
    let mut map = REGISTRIES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    map.retain(|_, value| value.strong_count() > 0);
    let key = Arc::as_ptr(cancel) as usize;
    if let Some(existing) = map.get(&key).and_then(Weak::upgrade) {
        return existing;
    }
    let registry = Arc::new(OwnedCompletionRegistry {
        cancel: cancel.clone(),
        state: Mutex::new(State::default()),
    });
    map.insert(key, Arc::downgrade(&registry));
    registry
}
impl OwnedCompletionRegistry {
    /// Register before spawning. A sealed/cancelled attempt must not start new work.
    pub fn register(self: &Arc<Self>) -> Result<OwnedReceipt, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "owned completion state unavailable")?;
        if state.sealed || self.cancel.load(Ordering::SeqCst) {
            return Err("owned execution is stopping".into());
        }
        state.active = state
            .active
            .checked_add(1)
            .ok_or("owned completion count overflow")?;
        Ok(OwnedReceipt {
            registry: self.clone(),
            finished: false,
        })
    }
    /// Seal against further starts, then wait for actual worker receipts. Dropping this
    /// future does not release workers. No automatic cancellation/retry is performed.
    pub async fn drain(&self, timeout: Duration) -> DrainOutcome {
        match self.state.lock() {
            Ok(mut state) => state.sealed = true,
            Err(_) => {
                return DrainOutcome::Failed {
                    active: 0,
                    failures: 1,
                }
            }
        }
        let started = tokio::time::Instant::now();
        loop {
            let status = match self.state.lock() {
                Ok(state) => {
                    if state.active == 0 {
                        if state.failures == 0 {
                            DrainOutcome::Complete
                        } else {
                            DrainOutcome::Failed {
                                active: 0,
                                failures: state.failures,
                            }
                        }
                    } else if state.failures > 0 {
                        DrainOutcome::Failed {
                            active: state.active,
                            failures: state.failures,
                        }
                    } else {
                        DrainOutcome::Pending {
                            active: state.active,
                        }
                    }
                }
                Err(_) => {
                    return DrainOutcome::Failed {
                        active: 0,
                        failures: 1,
                    }
                }
            };
            match status {
                DrainOutcome::Complete | DrainOutcome::Failed { active: 0, .. } => return status,
                _ if started.elapsed() >= timeout => return status,
                _ => {
                    tokio::time::sleep(
                        Duration::from_millis(10).min(timeout.saturating_sub(started.elapsed())),
                    )
                    .await
                }
            }
        }
    }
}
/// Move this into the owned blocking worker, never keep it on the awaited tool future.
pub struct OwnedReceipt {
    registry: Arc<OwnedCompletionRegistry>,
    finished: bool,
}
impl OwnedReceipt {
    /// Only call after process/descendants, pipes and temporary ACL cleanup completed.
    /// An error marks unconfirmed cleanup; error text is deliberately not retained.
    pub fn complete(mut self, result: Result<(), String>) {
        self.finish(result.is_ok());
    }
    fn finish(&mut self, success: bool) {
        if self.finished {
            return;
        }
        if let Ok(mut state) = self.registry.state.lock() {
            if state.active > 0 {
                state.active -= 1;
            } else {
                state.failures = state.failures.saturating_add(1);
            }
            if !success {
                state.failures = state.failures.saturating_add(1);
            }
        }
        self.finished = true;
    }
}
impl Drop for OwnedReceipt {
    fn drop(&mut self) {
        self.finish(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn detached_blocking_worker_remains_pending_until_cleanup() {
        let cancel = Arc::new(AtomicBool::new(false));
        let registry = for_cancel(&cancel);
        let receipt = registry.register().unwrap();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = tokio::task::spawn_blocking(move || {
            release_rx.recv().unwrap();
            receipt.complete(Ok(()));
        });
        drop(worker);
        cancel.store(true, Ordering::SeqCst);
        assert_eq!(
            registry.drain(Duration::from_millis(20)).await,
            DrainOutcome::Pending { active: 1 }
        );
        assert!(registry.register().is_err());
        release_tx.send(()).unwrap();
        assert_eq!(
            registry.drain(Duration::from_secs(1)).await,
            DrainOutcome::Complete
        );
    }
    #[tokio::test]
    async fn dropped_or_failed_receipt_never_confirms_cleanup() {
        for failed in [false, true] {
            let cancel = Arc::new(AtomicBool::new(false));
            let registry = for_cancel(&cancel);
            let receipt = registry.register().unwrap();
            if failed {
                receipt.complete(Err("private cleanup details".into()));
            } else {
                drop(receipt);
            }
            assert_eq!(
                registry.drain(Duration::ZERO).await,
                DrainOutcome::Failed {
                    active: 0,
                    failures: 1
                }
            );
        }
    }
    #[tokio::test]
    async fn exact_cancel_identity_shares_receipts_and_other_attempt_is_independent() {
        let cancel = Arc::new(AtomicBool::new(false));
        let registry = for_cancel(&cancel);
        let same = for_cancel(&cancel);
        assert!(Arc::ptr_eq(&registry, &same));
        let other = for_cancel(&Arc::new(AtomicBool::new(false)));
        assert!(!Arc::ptr_eq(&registry, &other));
        let receipt = registry.register().unwrap();
        assert_eq!(other.drain(Duration::ZERO).await, DrainOutcome::Complete);
        assert_eq!(
            same.drain(Duration::ZERO).await,
            DrainOutcome::Pending { active: 1 }
        );
        receipt.complete(Ok(()));
        assert_eq!(same.drain(Duration::ZERO).await, DrainOutcome::Complete);
    }
    #[tokio::test]
    async fn cancellation_and_sealing_refuse_new_workers() {
        let cancel = Arc::new(AtomicBool::new(true));
        assert!(for_cancel(&cancel).register().is_err());
        let registry = for_cancel(&Arc::new(AtomicBool::new(false)));
        assert_eq!(registry.drain(Duration::ZERO).await, DrainOutcome::Complete);
        assert!(registry.register().is_err());
    }
}
