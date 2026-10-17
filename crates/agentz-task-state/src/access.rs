//! In-process execution/relocation exclusion, including queued executions.
use std::sync::Arc;
use std::{collections::HashMap, sync::Mutex as MapMutex};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

#[derive(Clone, Default)]
pub struct EnvironmentAccess(Arc<RwLock<()>>);
impl EnvironmentAccess {
    pub async fn execution(&self) -> OwnedRwLockReadGuard<()> {
        self.0.clone().read_owned().await
    }
    pub fn cleanup(&self) -> Result<OwnedRwLockWriteGuard<()>, String> {
        self.0.clone().try_write_owned().map_err(|_| {
            "agent execution or journal operation is active/queued; stop it before repository changes".into()
        })
    }
}

/// Keys must be the canonical Git common-dir returned by validated preflight.
type RepositoryQueues = HashMap<String, Arc<Mutex<()>>>;
#[derive(Clone, Default)]
pub struct RepositoryAccess(Arc<MapMutex<RepositoryQueues>>);
impl RepositoryAccess {
    pub async fn transaction(&self, repository: &str) -> Result<OwnedMutexGuard<()>, String> {
        let queue = {
            let mut queues = self
                .0
                .lock()
                .map_err(|_| "repository transaction map is unavailable")?;
            queues
                .entry(repository.into())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        Ok(queue.lock_owned().await)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn repository_transactions_serialize_one_repository_without_blocking_another() {
        let queues = Arc::new(RepositoryAccess::default());
        let first = queues.transaction("repo-a").await.unwrap();
        let next = queues.as_ref().clone();
        let mut waiting = tokio::spawn(async move { next.transaction("repo-a").await.unwrap() });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut waiting)
                .await
                .is_err()
        );
        let independent = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            queues.transaction("repo-b"),
        )
        .await
        .unwrap()
        .unwrap();
        drop(first);
        let second = tokio::time::timeout(std::time::Duration::from_secs(2), waiting)
            .await
            .unwrap()
            .unwrap();
        drop(second);
        drop(independent);
    }
    #[tokio::test]
    async fn executions_and_cleanup_exclude_each_other_and_release_on_drop() {
        let gate = EnvironmentAccess::default();
        let queued = gate.execution().await;
        let parallel = gate.execution().await;
        assert!(gate.cleanup().is_err());
        drop(queued);
        assert!(gate.cleanup().is_err());
        drop(parallel);
        let cleanup = gate.cleanup().unwrap();
        assert!(gate.cleanup().is_err());
        let another = gate.clone();
        let mut waiting = tokio::spawn(async move { another.execution().await });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut waiting)
                .await
                .is_err()
        );
        drop(cleanup);
        let lease = tokio::time::timeout(std::time::Duration::from_secs(2), waiting)
            .await
            .unwrap()
            .unwrap();
        assert!(gate.cleanup().is_err());
        drop(lease);
        assert!(gate.cleanup().is_ok());
    }
}
