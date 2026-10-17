//! Cancellation registration spans queueing and execution; cleanup owns only its flag.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
type Single = Arc<Mutex<Option<Arc<AtomicBool>>>>;
type Tasks = Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>;
pub struct Registration {
    single: Single,
    tasks: Tasks,
    key: Option<String>,
    flag: Arc<AtomicBool>,
}
impl Registration {
    pub async fn new(
        single: Single,
        tasks: Tasks,
        key: Option<String>,
        flag: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        if let Some(key) = &key {
            let mut slots = tasks.lock().await;
            if slots.contains_key(key) {
                return Err("task already queued or running".into());
            }
            slots.insert(key.clone(), flag.clone());
        } else {
            let mut slot = single.lock().await;
            if slot.is_some() {
                return Err("chat already queued or running".into());
            }
            *slot = Some(flag.clone());
        }
        Ok(Self {
            single,
            tasks,
            key,
            flag,
        })
    }
    pub async fn release(&self) {
        if let Some(key) = &self.key {
            let mut slots = self.tasks.lock().await;
            if slots
                .get(key)
                .is_some_and(|flag| Arc::ptr_eq(flag, &self.flag))
            {
                slots.remove(key);
            }
        } else {
            let mut slot = self.single.lock().await;
            if slot
                .as_ref()
                .is_some_and(|flag| Arc::ptr_eq(flag, &self.flag))
            {
                *slot = None;
            }
        }
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        let single = self.single.clone();
        let tasks = self.tasks.clone();
        let key = self.key.clone();
        let flag = self.flag.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if let Some(key) = key {
                    let mut slots = tasks.lock().await;
                    if slots
                        .get(&key)
                        .is_some_and(|current| Arc::ptr_eq(current, &flag))
                    {
                        slots.remove(&key);
                    }
                } else {
                    let mut slot = single.lock().await;
                    if slot
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &flag))
                    {
                        *slot = None;
                    }
                }
            });
        }
    }
}
pub async fn acquire(
    slots: Arc<Semaphore>,
    cancel: &AtomicBool,
) -> Result<OwnedSemaphorePermit, String> {
    let waiting = slots.acquire_owned();
    tokio::pin!(waiting);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("agent queue cancelled".into());
        }
        tokio::select! {
            permit = &mut waiting => {
                let permit = permit.map_err(|e| format!("agent scheduler closed: {e}"))?;
                if cancel.load(Ordering::SeqCst) { return Err("agent queue cancelled".into()); }
                return Ok(permit);
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(25)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn queued_stop_and_stale_cleanup_preserve_replacement() {
        let single = Arc::new(Mutex::new(None));
        let tasks = Arc::new(Mutex::new(HashMap::new()));
        let flag = Arc::new(AtomicBool::new(false));
        let registration = Registration::new(
            single.clone(),
            tasks.clone(),
            Some("task".into()),
            flag.clone(),
        )
        .await
        .unwrap();
        assert!(Registration::new(
            single.clone(),
            tasks.clone(),
            Some("task".into()),
            Arc::new(AtomicBool::new(false))
        )
        .await
        .is_err());
        let slots = Arc::new(Semaphore::new(0));
        let waiter = tokio::spawn({
            let flag = flag.clone();
            let slots = slots.clone();
            async move { acquire(slots, &flag).await }
        });
        tasks
            .lock()
            .await
            .get("task")
            .unwrap()
            .store(true, Ordering::SeqCst);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(slots.available_permits(), 0);
        registration.release().await;
        let replacement_flag = Arc::new(AtomicBool::new(false));
        let replacement = Registration::new(
            single.clone(),
            tasks.clone(),
            Some("task".into()),
            replacement_flag.clone(),
        )
        .await
        .unwrap();
        drop(registration);
        tokio::task::yield_now().await;
        assert!(Arc::ptr_eq(
            tasks.lock().await.get("task").unwrap(),
            &replacement_flag
        ));
        replacement.release().await;
        assert!(tasks.lock().await.is_empty());
        let direct = Registration::new(single.clone(), tasks.clone(), None, flag)
            .await
            .unwrap();
        drop(direct);
        tokio::task::yield_now().await;
        assert!(single.lock().await.is_none());
    }
}
