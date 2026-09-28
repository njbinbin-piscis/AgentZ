//! Bounded, coalescing filesystem work. Dropping the sender stops the worker.
use std::collections::HashSet;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

const MAX_PENDING: usize = 1024;

#[derive(Default)]
pub struct Batch {
    pub paths: HashSet<String>,
    pub rebuild: bool,
}

impl Batch {
    fn push(&mut self, path: String) {
        if self.rebuild {
            return;
        }
        self.paths.insert(path);
        if self.paths.len() > MAX_PENDING {
            self.paths.clear();
            self.rebuild = true;
        }
    }
}

pub struct IndexWorker {
    pending: Arc<Mutex<Batch>>,
    wake: mpsc::SyncSender<()>,
}

impl IndexWorker {
    pub fn start(mut process: impl FnMut(Batch) + Send + 'static) -> std::io::Result<Self> {
        let pending = Arc::new(Mutex::new(Batch::default()));
        let shared = pending.clone();
        let (wake, rx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("workspace-index".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    // Fixed window: continuous writes must not starve processing.
                    std::thread::sleep(Duration::from_millis(200));
                    match rx.try_recv() {
                        Err(mpsc::TryRecvError::Disconnected) => break,
                        _ => {}
                    }
                    let batch =
                        std::mem::take(&mut *shared.lock().unwrap_or_else(|e| e.into_inner()));
                    if batch.rebuild || !batch.paths.is_empty() {
                        process(batch);
                    }
                }
            })?;
        Ok(Self { pending, wake })
    }

    pub fn enqueue(&self, path: String) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(path);
        // A full wake channel already guarantees a pending batch will be read.
        let _ = self.wake.try_send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_events_coalesce() {
        let mut batch = Batch::default();
        for _ in 0..10_000 {
            batch.push("src/main.rs".into());
        }
        assert_eq!(batch.paths.len(), 1);
        assert!(!batch.rebuild);
    }

    #[test]
    fn overflow_keeps_memory_bounded_and_requests_rebuild() {
        let mut batch = Batch::default();
        for i in 0..100_000 {
            batch.push(format!("src/{i}.rs"));
        }
        assert!(batch.rebuild);
        assert!(batch.paths.is_empty());
    }

    #[test]
    fn worker_processes_and_releases_callback_on_drop() {
        let (tx, rx) = mpsc::channel();
        let worker = IndexWorker::start(move |batch| {
            tx.send(batch).unwrap();
        })
        .unwrap();
        worker.enqueue("src/main.rs".into());
        let batch = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(batch.paths.contains("src/main.rs"));
        drop(worker);
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(3)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
