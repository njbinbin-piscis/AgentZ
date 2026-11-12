//! Desktop in-process Koi runtime.
//!
//! Implements the kernel [`SubagentRuntime`] contract by running each Koi turn
//! inside the running Tauri process. The pool coordinator calls
//! [`spawn_koi_turn`](SubagentRuntime::spawn_koi_turn) with a fully assembled
//! system prompt; we run it through the shared kernel headless turn
//! ([`run_piscis_turn_cancellable`]) against the Koi's project DB and stream
//! its agent events to the UI over the same `agentz:chat-event` channel the IDE
//! chat uses.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use uuid::Uuid;

use piscis_core::host::{
    EventSink, HeadlessCliMode, HeadlessCliRequest, KoiTurnExit, KoiTurnHandle, KoiTurnOutcome,
    KoiTurnRequest, PoolEvent, PoolEventSink, SubagentRuntime, TodoChangeAction,
};
use piscis_core::koi_prompt::build_koi_task_system_prompt;
use piscis_kernel::agent::tool::ToolRegistry;
use piscis_kernel::headless::{run_piscis_turn_cancellable, HeadlessDeps};
use piscis_kernel::store::settings::Settings;
use piscis_kernel::tools::{register_mcp_tools, register_neutral_into, NeutralToolsConfig};

use crate::commands::chat_turn::{
    materialize_headless_llm_settings, resolve_koi_llm_provider_for_turn,
};
use crate::commands::data_scope::{open_project_kernel_state, resolve_global_config_dir};

/// Same channel the IDE chat streams over, so a Koi turn's tokens/tools can be
/// surfaced in the UI keyed by the Koi's session id.
const POOL_CHAT_EVENT: &str = "agentz:chat-event";

/// Canonical channel every kernel [`PoolEvent`] is fanned out on. The frontend
/// collaboration board subscribes once and dispatches on the serialized
/// `kind` tag.
pub const POOL_EVENT_CHANNEL: &str = "agentz:pool-event";

/// Forwards kernel pool events (todo board, koi status, coordinator lifecycle)
/// to the UI as a single typed stream.
pub struct TauriPoolEventSink {
    app: AppHandle,
}

impl TauriPoolEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl PoolEventSink for TauriPoolEventSink {
    fn emit_pool(&self, event: &PoolEvent) {
        if let PoolEvent::TodoChanged { action, .. } = event {
            if matches!(action, TodoChangeAction::Completed | TodoChangeAction::Cancelled) {
                crate::runtime::patrol::kick_pool_patrols();
            }
        }
        let payload = serde_json::to_value(event).unwrap_or(Value::Null);
        let _ = self.app.emit(POOL_EVENT_CHANNEL, payload);
    }
}

/// Build the desktop pool wiring (runtime + event sink) the kernel coordinator
/// needs to fan team work out to member Koi from the main agent turn.
///
/// The runtime is a single process-wide instance: every dispatcher (chat turn,
/// patrol, workflows, member tools) shares one inflight registry, so a cancel
/// or wait issued from any of them reaches the turn that actually runs.
pub fn pool_wiring(app: &AppHandle) -> (Arc<dyn SubagentRuntime>, Arc<dyn PoolEventSink>) {
    (
        shared_subagent_runtime(app),
        Arc::new(TauriPoolEventSink::new(app.clone())),
    )
}

/// Process-wide shared Koi runtime (see [`pool_wiring`]).
pub fn shared_subagent_runtime(app: &AppHandle) -> Arc<DesktopInProcessSubagentRuntime> {
    use std::sync::OnceLock;
    static RUNTIME: OnceLock<Arc<DesktopInProcessSubagentRuntime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| Arc::new(DesktopInProcessSubagentRuntime::new(app.clone())))
        .clone()
}

struct PoolTurnEventSink {
    app: AppHandle,
    koi_id: String,
}

impl EventSink for PoolTurnEventSink {
    fn emit_session(&self, session_id: &str, event: &str, payload: Value) {
        if event == "agent_event" && payload.get("type").and_then(Value::as_str) == Some("done") {
            let n = |k: &str| payload.get(k).and_then(Value::as_u64).unwrap_or(0);
            crate::commands::botz::record_member_usage(
                &self.app,
                &self.koi_id,
                n("total_input_tokens") + n("total_output_tokens"),
            );
        }
        let _ = self.app.emit(
            POOL_CHAT_EVENT,
            json!({ "sessionId": session_id, "channel": event, "payload": payload }),
        );
    }

    fn emit_broadcast(&self, event: &str, payload: Value) {
        let _ = self.app.emit(
            POOL_CHAT_EVENT,
            json!({ "sessionId": Value::Null, "channel": event, "payload": payload }),
        );
    }
}

#[derive(Clone)]
pub struct DesktopInProcessSubagentRuntime {
    app: AppHandle,
    inflight: Arc<Mutex<HashMap<String, Arc<InflightTurn>>>>,
}

struct InflightTurn {
    cancel: Arc<AtomicBool>,
    /// Cached terminal outcome. `wait_koi_turn` is re-readable: the first
    /// waiter fills the cache, later waiters (retry / recovery paths) clone it
    /// instead of erroring on a consumed oneshot.
    outcome: Mutex<Option<KoiTurnOutcome>>,
    /// todo → turn binding, so the watchdog can cancel the live turn before
    /// resetting the board row (prevents double dispatch of the same todo).
    todo_id: Option<String>,
    project_dir: Option<String>,
}

impl DesktopInProcessSubagentRuntime {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl SubagentRuntime for DesktopInProcessSubagentRuntime {
    async fn spawn_koi_turn(&self, request: KoiTurnRequest) -> anyhow::Result<KoiTurnHandle> {
        let turn_id = Uuid::new_v4().to_string();
        let handle = KoiTurnHandle {
            turn_id: turn_id.clone(),
            pool_id: request.pool_id.clone(),
            koi_id: request.koi_id.clone(),
        };

        let cancel = Arc::new(AtomicBool::new(false));
        let turn = Arc::new(InflightTurn {
            cancel: cancel.clone(),
            outcome: Mutex::new(None),
            todo_id: request.todo_id.clone(),
            project_dir: request.workspace.clone(),
        });
        self.inflight.lock().await.insert(turn_id, turn.clone());

        let app = self.app.clone();
        let inflight = self.inflight.clone();
        let cleanup_turn_id = handle.turn_id.clone();
        let handle_for_task = handle.clone();
        let handle_for_error = handle.clone();
        let turn_for_task = turn.clone();
        tokio::spawn(async move {
            let outcome = run_in_process_koi_turn(app, handle_for_task, request, cancel)
                .await
                .unwrap_or_else(|e| KoiTurnOutcome {
                    handle: handle_for_error,
                    exit_kind: KoiTurnExit::Crashed,
                    response_text: String::new(),
                    error: Some(e.to_string()),
                    exit_code: None,
                });
            *turn_for_task.outcome.lock().await = Some(outcome);
            cleanup_unclaimed_outcome(inflight, cleanup_turn_id).await;
        });

        Ok(handle)
    }

    async fn cancel_koi_turn(&self, handle: &KoiTurnHandle) -> anyhow::Result<()> {
        let turn = {
            let inflight = self.inflight.lock().await;
            inflight.get(&handle.turn_id).cloned()
        };
        if let Some(turn) = turn {
            turn.cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    async fn wait_koi_turn(&self, handle: &KoiTurnHandle) -> anyhow::Result<KoiTurnOutcome> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(24 * 60 * 60);
        loop {
            let turn = {
                let inflight = self.inflight.lock().await;
                inflight
                    .get(&handle.turn_id)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("unknown Koi turn: {}", handle.turn_id))?
            };
            if let Some(outcome) = turn.outcome.lock().await.clone() {
                self.inflight.lock().await.remove(&handle.turn_id);
                return Ok(outcome);
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("wait_koi_turn timed out for {}", handle.turn_id);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

impl DesktopInProcessSubagentRuntime {
    /// Cancel every live turn bound to `todo_id` in this project. Called by the
    /// patrol watchdog *before* it resets stale board rows, so a re-dispatch
    /// can never race a still-running turn on the same todo.
    pub async fn cancel_turns_for_todo(&self, project_dir: &str, todo_id: &str) -> usize {
        let turns: Vec<Arc<InflightTurn>> = {
            let inflight = self.inflight.lock().await;
            inflight
                .values()
                .filter(|t| {
                    t.todo_id.as_deref() == Some(todo_id)
                        && t.project_dir.as_deref() == Some(project_dir)
                })
                .cloned()
                .collect()
        };
        for turn in &turns {
            turn.cancel.store(true, Ordering::SeqCst);
        }
        turns.len()
    }
}

/// Reap an outcome that `wait_koi_turn` never claimed (e.g. the coordinator
/// gave up), so the inflight map does not grow unbounded. The drop is logged
/// with the exit kind so a lost deliverable is at least observable.
async fn cleanup_unclaimed_outcome(
    inflight: Arc<Mutex<HashMap<String, Arc<InflightTurn>>>>,
    turn_id: String,
) {
    tokio::time::sleep(Duration::from_secs(60)).await;
    let turn = {
        let inflight = inflight.lock().await;
        inflight.get(&turn_id).cloned()
    };
    let Some(turn) = turn else {
        return;
    };
    let unclaimed = turn.outcome.lock().await.is_some();
    if unclaimed {
        if let Some(outcome) = turn.outcome.lock().await.as_ref() {
            tracing::warn!(
                target: "pool::koi",
                turn_id = %turn_id,
                todo_id = ?turn.todo_id,
                exit_kind = ?outcome.exit_kind,
                "dropping unclaimed Koi turn outcome (no waiter within 60s)"
            );
        }
        inflight.lock().await.remove(&turn_id);
    }
}

/// Build the tool surface a Koi member gets: full neutral tools (read / write /
/// shell / code_run) plus codebase search and web fetch, plus the user's MCP
/// servers and authorized connectors. Intentionally omits `delegate` (no
/// recursive fan-out) and UI-bound tools (chat_ui / browser).
async fn build_koi_registry(
    app: &AppHandle,
    db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    settings: Arc<Mutex<Settings>>,
    event_sink: Arc<dyn EventSink>,
) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    // Members must reach the pool: the kernel koi prompt requires pool_chat /
    // pool_org (deliverable post, complete_todo, @!mention handoff). Both tools
    // only register when a pool event sink is bound.
    let (subagent_runtime, pool_event_sink) = pool_wiring(app);
    let cfg = NeutralToolsConfig {
        db: Some(db),
        settings: Some(settings.clone()),
        event_sink: Some(event_sink),
        pool_event_sink: Some(pool_event_sink),
        subagent_runtime: Some(subagent_runtime),
        ..Default::default()
    };
    register_neutral_into(&mut registry, &cfg);
    registry.register(Box::new(crate::tools::codebase_search::CodebaseSearchTool));
    registry.register(Box::new(crate::tools::graph_search::GraphSearchTool));
    registry.register(Box::new(crate::tools::graph_explore::GraphExploreTool));
    registry.register(Box::new(crate::tools::repo_memory::RepoMemoryTool));
    registry.register(Box::new(crate::tools::symbols::SymbolSearchTool));
    registry.register(Box::new(crate::tools::symbols::ImpactTool));

    let mcp_servers = { settings.lock().await.mcp_servers.clone() };
    if !mcp_servers.is_empty() {
        register_mcp_tools(&mut registry, &mcp_servers).await;
    }
    if let Ok(config_dir) = resolve_global_config_dir(app) {
        registry.register(Box::new(crate::tools::api_connector::ApiConnectorTool {
            config_dir: config_dir.clone(),
            allowed_ids: None,
        }));
        let connector_configs =
            crate::commands::connectors::resolve_connector_mcp_configs(&config_dir);
        if !connector_configs.is_empty() {
            crate::commands::connectors::register_connector_mcp_tools(
                &mut registry,
                &config_dir,
                &connector_configs,
            )
            .await;
        }
    }
    registry
}

async fn run_in_process_koi_turn(
    app: AppHandle,
    handle: KoiTurnHandle,
    request: KoiTurnRequest,
    cancel: Arc<AtomicBool>,
) -> anyhow::Result<KoiTurnOutcome> {
    if cancel.load(Ordering::SeqCst) {
        return Ok(cancelled_outcome(handle, "cancelled before dispatch"));
    }

    // Resolve the project directory: coordinator-provided workspace first.
    // Pool turns must carry the pool's project_dir (or worktree); never
    // fall back to the global settings workspace_root for those.
    let project_dir = match request.workspace.clone().filter(|w| !w.trim().is_empty()) {
        Some(w) => w,
        None if !request.pool_id.trim().is_empty() => {
            return Ok(crashed_outcome(
                handle,
                "pool Koi turn missing workspace (pool project_dir was not passed)",
            ));
        }
        None => resolve_global_config_dir(&app)
            .ok()
            .and_then(|dir| Settings::load(&dir.join("config.json")).ok())
            .map(|s| s.workspace_root)
            .filter(|w| !w.trim().is_empty())
            .unwrap_or_default(),
    };
    if project_dir.trim().is_empty() {
        return Ok(crashed_outcome(
            handle,
            "no workspace configured for Koi turn",
        ));
    }

    app.state::<crate::state::AppState>()
        .browser
        .set_surface(&request.session_id, "botz");
    let (db, settings) = match open_project_kernel_state(&app, &project_dir) {
        Ok(state) => state,
        Err(e) => return Ok(crashed_outcome(handle, &e)),
    };

    // Headless turns only read legacy `settings.provider/model`; map the Koi's
    // bound provider (or flash / first llm_providers entry) before dispatch.
    let koi_llm_provider_id = {
        let guard = db.lock().await;
        resolve_global_config_dir(&app)
            .ok()
            .and_then(|dir| resolve_koi_llm_provider_for_turn(&guard, &dir, &request.koi_id))
            .or_else(|| {
                guard
                    .get_koi(&request.koi_id)
                    .ok()
                    .flatten()
                    .and_then(|k| k.llm_provider_id)
            })
    };
    {
        let mut s = settings.lock().await;
        materialize_headless_llm_settings(&mut s, &app, koi_llm_provider_id.as_deref());
    }

    if let Some(reason) = crate::commands::botz::member_budget_block(&app, &request.koi_id) {
        return Ok(crashed_outcome(handle, &format!("bot paused: {reason}")));
    }

    let sink: Arc<dyn EventSink> = Arc::new(PoolTurnEventSink {
        app: app.clone(),
        koi_id: request.koi_id.clone(),
    });
    let mut registry = build_koi_registry(&app, db.clone(), settings.clone(), sink.clone()).await;

    // Inject org_spec / assemble the member system prompt. Todo turns use the
    // kernel 6-layer contract (Stop Gate last); workflow agent nodes keep a
    // lighter stack.
    let extra_context = {
        let guard = db.lock().await;
        let pool_org = guard
            .get_pool_session(&request.pool_id)
            .ok()
            .flatten()
            .map(|s| s.org_spec)
            .filter(|s| !s.trim().is_empty());
        drop(guard);

        if request.todo_id.is_some() {
            let guard = db.lock().await;
            let koi = guard.get_koi(&request.koi_id).ok().flatten();
            let todo = request
                .todo_id
                .as_ref()
                .and_then(|id| guard.get_koi_todo(id).ok().flatten());
            match (koi, todo) {
                (Some(koi), Some(todo)) => {
                    let org_spec_ctx = pool_org
                        .as_deref()
                        .map(truncate_org_spec)
                        .filter(|s| !s.is_empty())
                        .map(|body| format!("\n\n## Project Organization\n{body}"))
                        .unwrap_or_default();
                    let assignment_ctx = format!(
                        "\n\n## Current Assignment\nTitle: {}\n{}\nTodo id: {}",
                        todo.title,
                        if todo.description.trim().is_empty() {
                            String::new()
                        } else {
                            format!("Description: {}\n", todo.description.trim())
                        },
                        todo.id
                    );
                    build_koi_task_system_prompt(
                        &koi.system_prompt,
                        &koi.name,
                        koi.icon.as_str(),
                        "",
                        "",
                        &org_spec_ctx,
                        "",
                        &assignment_ctx,
                    )
                }
                _ => {
                    let mut sections: Vec<String> = Vec::new();
                    let base = request.system_prompt.trim();
                    if !base.is_empty() {
                        sections.push(base.to_string());
                    }
                    if let Some(org) = pool_org.as_deref() {
                        sections.push(format!(
                            "## Project Organization\n{}",
                            truncate_org_spec(org)
                        ));
                    }
                    if let Some(extra) = request
                        .extra_system_context
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        sections.push(extra.to_string());
                    }
                    sections.join("\n\n")
                }
            }
        } else {
            let mut sections: Vec<String> = Vec::new();
            let base = request.system_prompt.trim();
            if !base.is_empty() {
                sections.push(base.to_string());
            }
            if let Some(org) = pool_org.as_deref() {
                sections.push(format!(
                    "## Project Organization\n{}",
                    truncate_org_spec(org)
                ));
            }
            if let Some(extra) = request
                .extra_system_context
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                sections.push(extra.to_string());
            }
            sections.join("\n\n")
        }
    };

    let cli_request = HeadlessCliRequest {
        prompt: request.user_prompt.clone(),
        workspace: Some(project_dir),
        mode: HeadlessCliMode::Piscis,
        session_id: Some(request.session_id.clone()),
        session_title: Some(format!("Assistant {}", request.koi_id)),
        channel: Some("pool".to_string()),
        pool_id: Some(request.pool_id.clone()),
        task_timeout_secs: request.task_timeout_secs,
        extra_system_context: Some(extra_context),
        ..Default::default()
    };

    // Members are first-class workers: full file/shell/pool tooling in their own
    // worktree (headless turns bypass prompts; the sandbox level still follows
    // Settings). Only delegation stays off so the team depth remains one.
    registry.retain(|t| !matches!(t.name(), "delegate" | "call_fish" | "task"));
    let mut deps = HeadlessDeps::new(db, settings, registry, sink);
    if let Some(secs) = request.task_timeout_secs.filter(|s| *s > 0) {
        deps.default_timeout = Duration::from_secs(u64::from(secs));
    }

    match run_piscis_turn_cancellable(cli_request, deps, cancel.clone()).await {
        Ok(resp) => {
            let was_cancelled = cancel.load(Ordering::SeqCst);
            Ok(KoiTurnOutcome {
                handle,
                exit_kind: if was_cancelled {
                    KoiTurnExit::Cancelled
                } else {
                    KoiTurnExit::Completed
                },
                response_text: resp.response_text,
                error: was_cancelled.then(|| "cancelled".to_string()),
                exit_code: Some(0),
            })
        }
        Err(error) => {
            let was_cancelled = cancel.load(Ordering::SeqCst);
            Ok(KoiTurnOutcome {
                handle,
                exit_kind: if was_cancelled {
                    KoiTurnExit::Cancelled
                } else {
                    KoiTurnExit::Crashed
                },
                response_text: String::new(),
                error: Some(error.to_string()),
                exit_code: Some(1),
            })
        }
    }
}

/// Clip org_spec with a pointer to pool_org(read) when truncated.
fn truncate_org_spec(content: &str) -> String {
    const MAX: usize = 12000;
    if MAX == 0 || content.chars().count() <= MAX {
        return content.to_string();
    }
    format!(
        "{}...\n\n## (org_spec truncated, see pool org_spec action)\n\
         Use pool_org(action=\"read\") to load the full organization contract when needed.",
        content.chars().take(MAX).collect::<String>()
    )
}

fn cancelled_outcome(handle: KoiTurnHandle, reason: &str) -> KoiTurnOutcome {
    KoiTurnOutcome {
        handle,
        exit_kind: KoiTurnExit::Cancelled,
        response_text: String::new(),
        error: Some(reason.to_string()),
        exit_code: Some(0),
    }
}

fn crashed_outcome(handle: KoiTurnHandle, reason: &str) -> KoiTurnOutcome {
    KoiTurnOutcome {
        handle,
        exit_kind: KoiTurnExit::Crashed,
        response_text: String::new(),
        error: Some(reason.to_string()),
        exit_code: Some(1),
    }
}

#[cfg(test)]
mod inflight_tests {
    use super::*;

    fn turn(todo_id: Option<&str>, project: Option<&str>) -> Arc<InflightTurn> {
        Arc::new(InflightTurn {
            cancel: Arc::new(AtomicBool::new(false)),
            outcome: Mutex::new(None),
            todo_id: todo_id.map(String::from),
            project_dir: project.map(String::from),
        })
    }

    fn outcome(handle: &KoiTurnHandle) -> KoiTurnOutcome {
        KoiTurnOutcome {
            handle: handle.clone(),
            exit_kind: KoiTurnExit::Completed,
            response_text: "done".into(),
            error: None,
            exit_code: Some(0),
        }
    }

    #[tokio::test]
    async fn wait_is_rereadable_and_removes_entry() {
        let inflight: Arc<Mutex<HashMap<String, Arc<InflightTurn>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let handle = KoiTurnHandle {
            turn_id: "t1".into(),
            pool_id: "p".into(),
            koi_id: "k".into(),
        };
        let t = turn(Some("todo-1"), Some("D:/proj"));
        *t.outcome.lock().await = Some(outcome(&handle));
        inflight.lock().await.insert("t1".into(), t);

        // First wait claims the outcome…
        let first = {
            let t = { inflight.lock().await.get("t1").cloned().unwrap() };
            let guard = t.outcome.lock().await;
            guard.clone().unwrap()
        };
        assert_eq!(first.exit_kind, KoiTurnExit::Completed);
        // …and a second read still sees it (cache, not consumed oneshot).
        let second = {
            let t = { inflight.lock().await.get("t1").cloned().unwrap() };
            let guard = t.outcome.lock().await;
            guard.clone().unwrap()
        };
        assert_eq!(second.response_text, "done");
    }

    #[tokio::test]
    async fn cancel_turns_for_todo_matches_todo_and_project() {
        let inflight: Arc<Mutex<HashMap<String, Arc<InflightTurn>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let a = turn(Some("todo-1"), Some("D:/proj"));
        let b = turn(Some("todo-1"), Some("D:/other"));
        let c = turn(Some("todo-2"), Some("D:/proj"));
        inflight.lock().await.insert("a".into(), a.clone());
        inflight.lock().await.insert("b".into(), b.clone());
        inflight.lock().await.insert("c".into(), c.clone());

        let turns: Vec<Arc<InflightTurn>> = {
            let guard = inflight.lock().await;
            guard
                .values()
                .filter(|t| {
                    t.todo_id.as_deref() == Some("todo-1")
                        && t.project_dir.as_deref() == Some("D:/proj")
                })
                .cloned()
                .collect()
        };
        for t in &turns {
            t.cancel.store(true, Ordering::SeqCst);
        }
        assert_eq!(turns.len(), 1);
        assert!(a.cancel.load(Ordering::SeqCst));
        assert!(!b.cancel.load(Ordering::SeqCst));
        assert!(!c.cancel.load(Ordering::SeqCst));
    }
}
