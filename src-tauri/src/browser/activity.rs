//! Tracks browser tool usage and in-flight agent turns for close-guard UX.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

const RECENT_BROWSER_WINDOW: Duration = Duration::from_secs(30);

#[derive(Clone, Default)]
pub struct BrowserActivity {
    active_turns: Arc<AtomicUsize>,
    last_browser_tool_at: Arc<Mutex<Option<Instant>>>,
}

impl BrowserActivity {
    pub fn set_chat_turn_active(&self, active: bool) {
        if active {
            self.active_turns.fetch_add(1, Ordering::SeqCst);
        } else {
            let mut cur = self.active_turns.load(Ordering::SeqCst);
            while cur > 0 {
                match self.active_turns.compare_exchange_weak(
                    cur,
                    cur - 1,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(actual) => cur = actual,
                }
            }
        }
    }

    pub fn active_turn_count(&self) -> usize {
        self.active_turns.load(Ordering::SeqCst)
    }

    pub async fn mark_browser_tool(&self) {
        *self.last_browser_tool_at.lock().await = Some(Instant::now());
    }

    pub async fn close_guard(&self) -> BrowserCloseGuard {
        let agent_active = self.active_turns.load(Ordering::SeqCst) > 0;
        let recent_browser = self
            .last_browser_tool_at
            .lock()
            .await
            .map(|t| t.elapsed() < RECENT_BROWSER_WINDOW)
            .unwrap_or(false);

        if agent_active {
            return BrowserCloseGuard {
                can_close: false,
                agent_active: true,
                reason: Some("An agent turn is in progress and may be using the browser.".into()),
            };
        }
        if recent_browser {
            return BrowserCloseGuard {
                can_close: false,
                agent_active: false,
                reason: Some(
                    "The browser was used by the agent recently. Closing will end the session."
                        .into(),
                ),
            };
        }
        BrowserCloseGuard {
            can_close: true,
            agent_active: false,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BrowserCloseGuard {
    pub can_close: bool,
    pub agent_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn concurrent_turns_preserve_agent_active_guard() {
        let activity = BrowserActivity::default();
        assert!(activity.close_guard().await.can_close);

        activity.set_chat_turn_active(true);
        activity.set_chat_turn_active(true);
        assert_eq!(activity.active_turn_count(), 2);

        // Ending one task turn must NOT drop the close guard while the second turn is still active.
        activity.set_chat_turn_active(false);
        let guard = activity.close_guard().await;
        assert!(!guard.can_close);
        assert!(guard.agent_active);

        activity.set_chat_turn_active(false);
        assert_eq!(activity.active_turn_count(), 0);
        let guard_after = activity.close_guard().await;
        assert!(guard_after.can_close);
        assert!(!guard_after.agent_active);
    }
}

