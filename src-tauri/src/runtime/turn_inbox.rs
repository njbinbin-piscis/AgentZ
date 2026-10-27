//! Per-session inbox for messages that must reach a turn while it runs.
//!
//! A running chat turn registers a channel here; background-task notices are
//! delivered into it and the agent loop injects them as user messages before
//! its next iteration. When no turn is running, callers fall back to starting
//! a new continuation turn.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tokio::sync::mpsc;

const INBOX_CAPACITY: usize = 64;

struct Slot {
    token: u64,
    tx: mpsc::Sender<String>,
}

fn inboxes() -> &'static Mutex<HashMap<String, Slot>> {
    static MAP: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Keeps the inbox registered for the lifetime of a turn.
pub struct InboxGuard {
    session_id: String,
    token: u64,
}

impl InboxGuard {
    /// Unregister now so no further message can be delivered; anything still
    /// queued must then be drained from the receiver by the caller.
    pub fn close(self) {}
}

impl Drop for InboxGuard {
    fn drop(&mut self) {
        let mut map = inboxes().lock().unwrap();
        if map.get(&self.session_id).is_some_and(|s| s.token == self.token) {
            map.remove(&self.session_id);
        }
    }
}

/// Register the running turn of `session_id`. A newer registration for the
/// same session replaces an older one.
pub fn open(session_id: &str) -> (InboxGuard, mpsc::Receiver<String>) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let token = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel(INBOX_CAPACITY);
    inboxes()
        .lock()
        .unwrap()
        .insert(session_id.to_string(), Slot { token, tx });
    (
        InboxGuard {
            session_id: session_id.to_string(),
            token,
        },
        rx,
    )
}

/// Deliver `msg` to the session's running turn. Returns false when no turn is
/// running (or its inbox is full/closed).
pub fn try_deliver(session_id: &str, msg: String) -> bool {
    let map = inboxes().lock().unwrap();
    match map.get(session_id) {
        Some(slot) => slot.tx.try_send(msg).is_ok(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deliver_only_while_open_and_newest_wins() {
        assert!(!try_deliver("inbox-test", "x".into()));
        let (g1, mut rx1) = open("inbox-test");
        assert!(try_deliver("inbox-test", "a".into()));
        assert_eq!(rx1.try_recv().unwrap(), "a");
        let (g2, mut rx2) = open("inbox-test");
        drop(g1); // stale guard must not unregister the newer turn
        assert!(try_deliver("inbox-test", "b".into()));
        assert_eq!(rx2.try_recv().unwrap(), "b");
        g2.close();
        assert!(!try_deliver("inbox-test", "c".into()));
    }
}
