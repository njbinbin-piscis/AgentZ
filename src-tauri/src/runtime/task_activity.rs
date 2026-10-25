//! Single-host task exclusion; environment read leases do not serialize a task.
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex as MapMutex, Weak},
};
use tokio::sync::{Mutex, OwnedMutexGuard};
type Key = (String, String);
type Entries = HashMap<Key, Weak<Mutex<()>>>;
fn holders() -> &'static MapMutex<HashMap<Key, (String, std::time::Instant)>> {
    static H: std::sync::OnceLock<MapMutex<HashMap<Key, (String, std::time::Instant)>>> =
        std::sync::OnceLock::new();
    H.get_or_init(Default::default)
}
#[derive(Clone, Default)]
pub struct TaskActivity(Arc<MapMutex<Entries>>);
impl TaskActivity {
    #[track_caller]
    pub fn try_enter(&self, project: &Path, task: &str) -> Result<OwnedMutexGuard<()>, String> {
        let loc = std::panic::Location::caller();
        let caller = format!("{}:{}", loc.file(), loc.line());
        if task.trim().is_empty() {
            return Err("task activity requires a task identity".into());
        }
        let key = (agentz_task_state::canonical_root(project)?, task.to_owned());
        let lock = {
            let mut entries = self.0.lock().map_err(|_| "task activity map unavailable")?;
            entries.retain(|_, weak| weak.strong_count() > 0);
            if let Some(lock) = entries.get(&key).and_then(Weak::upgrade) {
                lock
            } else {
                let lock = Arc::new(Mutex::new(()));
                entries.insert(key.clone(), Arc::downgrade(&lock));
                lock
            }
        };
        match lock.try_lock_owned() {
            Ok(guard) => {
                if let Ok(mut h) = holders().lock() {
                    h.insert(key, (caller, std::time::Instant::now()));
                }
                Ok(guard)
            }
            Err(_) => {
                // Diagnostic: say who holds the gate and for how long, so a
                // leaked gate (UI idle, nothing running) is identifiable.
                let who = holders()
                    .lock()
                    .ok()
                    .and_then(|h| h.get(&key).map(|(c, t)| format!("{c}, {}s", t.elapsed().as_secs())))
                    .unwrap_or_else(|| "unknown".into());
                tracing::warn!(task = %task, holder = %who, "task gate busy");
                Err(format!("task queued/running or goal mutation active (held by {who})"))
            }
        }
    }

    /// True while a host turn (or goal mutation) still holds the task gate.
    /// Used to heal persisted `running` session rows that outlived their turn.
    pub fn is_active(&self, project: &Path, task: &str) -> bool {
        if task.trim().is_empty() {
            return false;
        }
        let Ok(key) = agentz_task_state::canonical_root(project).map(|root| (root, task.to_owned()))
        else {
            return false;
        };
        let Ok(mut entries) = self.0.lock() else {
            return false;
        };
        entries.retain(|_, weak| weak.strong_count() > 0);
        // The Arc backing the gate lives only while a turn holds OwnedMutexGuard.
        entries.get(&key).and_then(Weak::upgrade).is_some()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn same_task_excludes_queue_and_mutation_different_keys_run_and_drop_releases() {
        let root = std::env::temp_dir().join(format!("agentz-task-gate-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("other")).unwrap();
        let gate = TaskActivity::default();
        let queued = gate.try_enter(&root, "task").unwrap();
        assert!(gate.try_enter(&root.join("."), "task").is_err());
        let other_task = gate.try_enter(&root, "other").unwrap();
        let other_project = gate.try_enter(&root.join("other"), "task").unwrap();
        let slots = Arc::new(tokio::sync::Semaphore::new(0));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), slots.acquire())
                .await
                .is_err()
        );
        assert!(gate.try_enter(&root, "task").is_err());
        assert!(gate.is_active(&root, "other"));
        drop(queued);
        let replacement = gate.try_enter(&root, "task").unwrap();
        drop(replacement);
        drop(other_task);
        assert!(!gate.is_active(&root, "other"));
        assert!(!gate.is_active(&root, "missing"));
        drop(other_project);
        for id in 0..50 {
            drop(gate.try_enter(&root, &format!("temporary-{id}")).unwrap());
        }
        assert_eq!(gate.0.lock().unwrap().len(), 1);
        assert!(gate.try_enter(&root, " ").is_err());
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
