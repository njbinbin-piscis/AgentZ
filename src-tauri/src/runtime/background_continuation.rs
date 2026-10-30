//! Host-side continuation for background tasks.
//!
//! When a registered background task becomes ready or reaches a terminal
//! state, the kernel registry fires its completion listener. This hub turns
//! that event into (a) a `background_task` UI event on the shared chat
//! channel and (b) — when the starter asked for notification — a synthetic
//! continuation turn dispatched into the originating session through the
//! exact same stack as a user `chat_send` (`dispatch_host_turn`).
//!
//! Guardrails: one dispatch per (task, event kind), a per-session cap on
//! consecutive automatic continuations (reset by any user turn), parking
//! while the session is busy, and kill-discards-pending semantics.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use piscis_kernel::background::{BgEvent, BgKind, BgSnapshot, NotifyOn, ReadySpec};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::commands::chat::{dispatch_host_turn, HostTurnParams, CHAT_EVENT};
use crate::commands::data_scope::open_project_kernel_state;
use crate::state::AppState;

/// Consecutive automatic continuations allowed per session before a human
/// turn is required again (mirrors the frontend turn-timeout budget).
const MAX_AUTO_CONTINUATIONS: u32 = 6;
/// How long a parked continuation waits for its session to become idle.
const PARK_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
struct PendingContinuation {
    bg_id: String,
    project_dir: String,
    session_id: String,
    prompt: String,
    display_prompt: String,
    session_source: Option<String>,
    prefer_zh: bool,
}

/// What the user picked in the composer for their latest turn in a session;
/// continuations reuse it instead of falling back to the default model.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct TurnProfile {
    pub model_id: Option<String>,
    pub agent_id: Option<String>,
    pub enabled_skills: Option<Vec<String>>,
}

#[derive(Default)]
pub struct BgContinuationHub {
    profiles: Mutex<HashMap<String, TurnProfile>>,
    dispatched: Mutex<HashSet<String>>,
    counts: Mutex<HashMap<String, u32>>,
    pending: Mutex<HashMap<String, PendingContinuation>>,
    /// Notices handed to a running turn, keyed by prompt text, until that
    /// turn closes (so unconsumed ones can be re-routed).
    injected: Mutex<HashMap<String, PendingContinuation>>,
}

/// Whether a registry event should be reported back to the agent.
fn should_notify(
    kind: &str,
    task_kind: BgKind,
    notify_on: NotifyOn,
    has_ready: bool,
    ready_notified: bool,
    snap: &BgSnapshot,
) -> bool {
    // An explicit kill (agent, user, session cleanup) was intended.
    if snap.killed {
        return false;
    }
    let clean_exit = snap.status == "exited" && snap.exit_code == Some(0);
    match (kind, task_kind) {
        ("ready", BgKind::Service) => has_ready,
        ("ready", BgKind::Job) => notify_on == NotifyOn::Ready,
        // Services are expected to run until stopped: any exit is unexpected.
        (_, BgKind::Service) => true,
        // Already reported as ready: only a failure is news.
        (_, BgKind::Job) if ready_notified => !clean_exit,
        _ => true,
    }
}

impl BgContinuationHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// A user-initiated turn (anything not dispatched by this hub) resets
    /// the session's automatic-continuation budget.
    pub fn note_user_turn(&self, session_id: &str, profile: TurnProfile) {
        self.counts.lock().unwrap().remove(session_id);
        self.set_profile(session_id, profile);
    }

    /// The user changed the session's composer selection (without sending).
    pub fn set_profile(&self, session_id: &str, profile: TurnProfile) {
        self.profiles
            .lock()
            .unwrap()
            .insert(session_id.to_string(), profile);
    }

    fn profile_for(&self, session_id: &str) -> TurnProfile {
        self.profiles
            .lock()
            .unwrap()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Drop a parked (not yet dispatched) continuation for a task, e.g.
    /// because the user killed it or deleted its session.
    pub fn cancel_pending_for_bg(&self, bg_id: &str) {
        self.pending.lock().unwrap().remove(bg_id);
    }

    /// Session deleted: drop every parked continuation targeting it.
    pub fn cancel_pending_for_session(&self, session_id: &str) {
        self.profiles.lock().unwrap().remove(session_id);
        self.pending
            .lock()
            .unwrap()
            .retain(|_, item| item.session_id != session_id);
    }

    pub fn pending_count(&self) -> usize {
        self.pending.lock().unwrap().len()
    }

    fn try_claim(&self, bg_id: &str, kind: &str) -> bool {
        self.dispatched
            .lock()
            .unwrap()
            .insert(format!("{bg_id}:{kind}"))
    }

    fn was_claimed(&self, bg_id: &str, kind: &str) -> bool {
        self.dispatched
            .lock()
            .unwrap()
            .contains(&format!("{bg_id}:{kind}"))
    }

    fn claim_budget(&self, session_id: &str) -> bool {
        let mut counts = self.counts.lock().unwrap();
        let n = counts.entry(session_id.to_string()).or_insert(0);
        if *n >= MAX_AUTO_CONTINUATIONS {
            return false;
        }
        *n += 1;
        true
    }

    /// Registry listener entry point. Called from kernel worker threads;
    /// never blocks them on turn dispatch.
    pub fn on_event(&self, app: AppHandle, state: AppState, event: BgEvent) {
        let (kind, snap) = match &event {
            BgEvent::Ready(snap) => ("ready", snap.clone()),
            BgEvent::Terminal(snap) => ("terminal", snap.clone()),
        };
        // (a) UI event, regardless of continuation policy.
        let _ = app.emit(
            CHAT_EVENT,
            json!({
                "sessionId": snap.session_id,
                "channel": "background_task",
                "payload": {
                    "kind": kind,
                    "task": snap,
                },
            }),
        );
        // (b) Persist + maybe continue the originating session.
        let hub_state = state.clone();
        let hub_app = app.clone();
        // Registry callbacks arrive on plain kernel worker threads without a
        // tokio context, so hop onto the Tauri runtime explicitly.
        tauri::async_runtime::spawn(async move {
            hub_state
                .bg_hub
                .handle_completion(hub_app, hub_state.clone(), kind, snap)
                .await;
        });
    }

    async fn handle_completion(
        &self,
        app: AppHandle,
        state: AppState,
        kind: &str,
        snap: BgSnapshot,
    ) {
        // Persist the terminal/ready state (best-effort; P2 table).
        crate::runtime::background_store::record_snapshot(&app, &snap, kind).await;

        let Some(entry) = state.background_runs.get(&snap.id) else {
            return;
        };
        let ready_notified = self.was_claimed(&snap.id, "ready");
        if !should_notify(
            kind,
            entry.kind,
            entry.notify_on,
            entry.ready != ReadySpec::None,
            ready_notified,
            &snap,
        ) {
            return;
        }
        if !self.try_claim(&snap.id, kind) {
            return;
        }
        let project_dir = snap.workspace_root.clone();
        if project_dir.is_empty() || snap.session_id.is_empty() {
            return;
        }

        // Resolve the session's source + UI language from its project store.
        let (session_source, prefer_zh) = match open_project_kernel_state(&app, &project_dir) {
            Ok((db, settings)) => {
                let source = {
                    let guard = db.lock().await;
                    guard
                        .get_session(&snap.session_id)
                        .ok()
                        .flatten()
                        .map(|s| s.source)
                };
                let zh = {
                    let guard = settings.lock().await;
                    guard.language.to_lowercase().starts_with("zh")
                };
                (source, zh)
            }
            Err(_) => (None, false),
        };

        let tail = entry.output_tail(4096);
        let status_line = match kind {
            "ready" => "is ready (readiness condition met; still running)".to_string(),
            _ => match snap.exit_code {
                Some(code) => format!("finished with status {} (exit code {})", snap.status, code),
                None => format!("finished with status {}", snap.status),
            },
        };
        let prompt = format!(
            "<background_task_completed>\n\
             task_id: {}\n\
             command: {}\n\
             result: {}\n\
             elapsed_ms: {}\n\
             output_tail:\n{}\n\
             </background_task_completed>\n\
             The background task you started earlier has {status_for_user}. \
             Check its result against the original goal: if it failed, diagnose and fix; \
             if it succeeded, continue with the next step. Do not restart the task unless it is actually needed.",
            snap.id,
            snap.label,
            status_line,
            snap.elapsed_ms,
            if tail.trim().is_empty() { "(no output)" } else { tail.trim() },
            status_for_user = if kind == "ready" {
                "become ready"
            } else {
                "finished"
            },
        );
        let display_prompt = if prefer_zh {
            format!("后台任务「{}」已{}，自动继续", truncate_label(&snap.label), if kind == "ready" { "就绪" } else { "完成" })
        } else {
            format!(
                "Background task \"{}\" {}, resuming automatically",
                truncate_label(&snap.label),
                if kind == "ready" { "is ready" } else { "finished" }
            )
        };

        let pending = PendingContinuation {
            bg_id: snap.id.clone(),
            project_dir: project_dir.clone(),
            session_id: snap.session_id.clone(),
            prompt,
            display_prompt,
            session_source,
            prefer_zh,
        };

        // A turn is running in this session: hand the notice to it instead of
        // queueing a separate turn behind it.
        if crate::runtime::turn_inbox::try_deliver(&snap.session_id, pending.prompt.clone()) {
            self.injected
                .lock()
                .unwrap()
                .insert(pending.prompt.clone(), pending);
            let _ = app.emit(
                CHAT_EVENT,
                json!({
                    "sessionId": snap.session_id,
                    "channel": "background_task_injected",
                    "payload": { "bgId": snap.id, "kind": kind },
                }),
            );
            return;
        }
        self.park_and_dispatch(app, state, pending).await;
    }

    /// The session's running turn ended. Notices it never consumed are
    /// re-routed to a continuation turn; consumed ones are forgotten.
    pub fn on_turn_closed(
        &self,
        app: AppHandle,
        state: AppState,
        session_id: &str,
        undelivered: Vec<String>,
    ) {
        let mut retry = Vec::new();
        {
            let mut injected = self.injected.lock().unwrap();
            for msg in undelivered {
                if let Some(p) = injected.remove(&msg) {
                    retry.push(p);
                }
            }
            injected.retain(|_, p| p.session_id != session_id);
        }
        for pending in retry {
            let hub = state.bg_hub.clone();
            let app = app.clone();
            let state = state.clone();
            tauri::async_runtime::spawn(async move {
                hub.park_and_dispatch(app, state, pending).await;
            });
        }
    }

    async fn park_and_dispatch(
        &self,
        app: AppHandle,
        state: AppState,
        pending: PendingContinuation,
    ) {
        if !self.claim_budget(&pending.session_id) {
            tracing::info!(
                session_id = %pending.session_id,
                bg_id = %pending.bg_id,
                "background continuation suppressed: auto-continuation budget exhausted"
            );
            return;
        }
        self.pending
            .lock()
            .unwrap()
            .insert(pending.bg_id.clone(), pending.clone());
        crate::runtime::background_store::save_pending(
            &app,
            &pending.project_dir,
            &pending.bg_id,
            &pending.session_id,
            &pending.prompt,
            &pending.display_prompt,
            pending.session_source.as_deref(),
            pending.prefer_zh,
        )
        .await;
        self.dispatch_when_idle(app, state, pending).await;
    }

    /// Dispatch immediately when the session is idle; otherwise poll the
    /// activity gate (the pending record can be cancelled meanwhile).
    async fn dispatch_when_idle(
        &self,
        app: AppHandle,
        state: AppState,
        pending: PendingContinuation,
    ) {
        let bg_id = pending.bg_id.clone();
        let deadline = std::time::Instant::now() + PARK_TIMEOUT;
        loop {
            if !self.pending.lock().unwrap().contains_key(&bg_id) {
                return; // cancelled while parked
            }
            let busy = state
                .task_activity
                .is_active(std::path::Path::new(&pending.project_dir), &pending.session_id);
            if !busy {
                break;
            }
            if std::time::Instant::now() >= deadline {
                tracing::info!(bg_id = %bg_id, "background continuation expired while session busy");
                self.pending.lock().unwrap().remove(&bg_id);
                return;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        // A user turn can grab the task gate between the idle check and dispatch;
        // re-park on that error instead of failing the continuation.
        loop {
            let profile = self.profile_for(&pending.session_id);
            let result = dispatch_host_turn(
                app.clone(),
                None,
                &state,
                HostTurnParams {
                    prompt: pending.prompt.clone(),
                    display_prompt: Some(pending.display_prompt.clone()),
                    session_id: Some(pending.session_id.clone()),
                    project_dir: Some(pending.project_dir.clone()),
                    chat_mode: Some("agent".to_string()),
                    task_key: Some(format!("host-bg-{bg_id}")),
                    session_source: pending.session_source.clone(),
                    prefer_zh: Some(pending.prefer_zh),
                    model_id: profile.model_id,
                    agent_id: profile.agent_id,
                    enabled_skills: profile.enabled_skills,
                    ..Default::default()
                },
            )
            .await;
            match result {
                Err(e)
                    if e.contains("task queued/running")
                        && std::time::Instant::now() < deadline
                        && self.pending.lock().unwrap().contains_key(&bg_id) =>
                {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                other => {
                    self.pending.lock().unwrap().remove(&bg_id);
                    crate::runtime::background_store::delete_pending(
                        &app,
                        &pending.project_dir,
                        &bg_id,
                    )
                    .await;
                    if let Err(error) = other {
                        tracing::warn!(bg_id = %bg_id, "background continuation turn failed: {error}");
                    }
                    return;
                }
            }
        }
    }

    /// Restore parked continuations persisted before an app restart
    /// (P2). Called once per project when its store is first reconciled.
    pub async fn restore_pending(
        &self,
        app: AppHandle,
        state: AppState,
        project_dir: &str,
    ) {
        let stored = crate::runtime::background_store::take_pending_continuations(
            &app, project_dir,
        )
        .await;
        for item in stored {
            if !self.try_claim(&item.bg_id, "restored") {
                continue;
            }
            if !self.claim_budget(&item.session_id) {
                continue;
            }
            let pending = PendingContinuation {
                bg_id: item.bg_id.clone(),
                project_dir: project_dir.to_string(),
                session_id: item.session_id,
                prompt: item.prompt,
                display_prompt: item.display_prompt,
                session_source: item.session_source,
                prefer_zh: item.prefer_zh,
            };
            self.pending
                .lock()
                .unwrap()
                .insert(item_bg_id(&pending), pending.clone());
            let app2 = app.clone();
            let state2 = state.clone();
            // The hub lives inside AppState; recover it through the state.
            let hub = state2.bg_hub.clone();
            tokio::spawn(async move {
                hub.dispatch_when_idle(app2, state2, pending).await;
            });
        }
    }
}

fn item_bg_id(p: &PendingContinuation) -> String {
    p.bg_id.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_dedupes_per_task_and_kind() {
        let hub = BgContinuationHub::new();
        assert!(hub.try_claim("bg-1", "terminal"));
        assert!(!hub.try_claim("bg-1", "terminal"));
        // Ready and terminal are distinct events for the same task.
        assert!(hub.try_claim("bg-1", "ready"));
        assert!(hub.try_claim("bg-2", "terminal"));
    }

    fn snap(status: &'static str, exit: Option<i32>, killed: bool) -> BgSnapshot {
        BgSnapshot {
            id: "bg".into(),
            session_id: "s".into(),
            workspace_root: String::new(),
            label: String::new(),
            cwd: String::new(),
            pid: None,
            status,
            kind: "job",
            killed,
            exit_code: exit,
            error: None,
            ready: false,
            started_unix_ms: 0,
            finished_unix_ms: None,
            elapsed_ms: 0,
            stdout_total: 0,
            stderr_total: 0,
        }
    }

    #[test]
    fn notify_policy() {
        let ok = snap("exited", Some(0), false);
        let bad = snap("exited", Some(1), false);
        let killed = snap("killed", Some(130), true);
        // Kills are never reported.
        assert!(!should_notify("terminal", BgKind::Job, NotifyOn::Exit, false, false, &killed));
        assert!(!should_notify("terminal", BgKind::Service, NotifyOn::Exit, false, false, &killed));
        // Jobs report completion; after a ready notice only failures.
        assert!(should_notify("terminal", BgKind::Job, NotifyOn::Exit, false, false, &ok));
        assert!(!should_notify("terminal", BgKind::Job, NotifyOn::Ready, true, true, &ok));
        assert!(should_notify("terminal", BgKind::Job, NotifyOn::Ready, true, true, &bad));
        // Services: ready when a condition exists; any unrequested exit.
        let running = snap("running", None, false);
        assert!(should_notify("ready", BgKind::Service, NotifyOn::Exit, true, false, &running));
        assert!(!should_notify("ready", BgKind::Job, NotifyOn::Exit, true, false, &running));
        assert!(should_notify("terminal", BgKind::Service, NotifyOn::Exit, false, false, &ok));
    }

    #[test]
    fn budget_caps_and_user_turn_resets() {
        let hub = BgContinuationHub::new();
        for _ in 0..MAX_AUTO_CONTINUATIONS {
            assert!(hub.claim_budget("sess"));
        }
        assert!(!hub.claim_budget("sess"));
        // Other sessions have their own budget.
        assert!(hub.claim_budget("other"));
        hub.note_user_turn("sess", TurnProfile::default());
        assert!(hub.claim_budget("sess"));
    }

    #[test]
    fn continuation_reuses_latest_user_turn_profile() {
        let hub = BgContinuationHub::new();
        assert_eq!(hub.profile_for("sess"), TurnProfile::default());
        let picked = TurnProfile {
            model_id: Some("mimopro".into()),
            agent_id: Some("reviewer".into()),
            enabled_skills: Some(vec!["tdd".into()]),
        };
        hub.note_user_turn("sess", picked.clone());
        assert_eq!(hub.profile_for("sess"), picked);
        assert_eq!(hub.profile_for("other"), TurnProfile::default());
    }

    #[test]
    fn cancel_pending_discards_parked_continuation() {
        let hub = BgContinuationHub::new();
        hub.pending.lock().unwrap().insert(
            "bg-9".into(),
            PendingContinuation {
                bg_id: "bg-9".into(),
                project_dir: "C:/p".into(),
                session_id: "s".into(),
                prompt: "p".into(),
                display_prompt: "d".into(),
                session_source: None,
                prefer_zh: false,
            },
        );
        assert_eq!(hub.pending_count(), 1);
        assert!(!hub.was_claimed("bg-9", "ready"));
        hub.cancel_pending_for_bg("bg-9");
        assert_eq!(hub.pending_count(), 0);
    }
}

fn truncate_label(label: &str) -> String {
    let mut out: String = label.chars().take(48).collect();
    if label.chars().count() > 48 {
        out.push('…');
    }
    out
}

/// Install the registry listener that routes completions into the hub.
/// Called once from app setup.
pub fn install_background_listener(app: &AppHandle, state: &AppState) {
    let app = app.clone();
    let state = state.clone();
    let registry = state.background_runs.clone();
    registry.set_listener(Arc::new(move |event| {
        state.bg_hub.on_event(app.clone(), state.clone(), event);
    }));
}

