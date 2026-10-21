//! Shared application state for the AgentZ desktop host.
//!
//! Deliberately small: only what the IDE workspace commands need. As later
//! milestones land (agent runtime, indexing, settings) this grows alongside
//! them, but the IDE surface stays self-contained here.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::{oneshot, Mutex, Semaphore};

use piscis_kernel::agent::loop_::ConfirmationResponseMap;
use piscis_kernel::agent::plan::new_plan_store;
use piscis_kernel::agent::plan::PlanStore;

use crate::browser::activity::BrowserActivity;
use crate::browser::BrowserManager;
use crate::commands::dap::DapManager;
use crate::commands::ext_host::ExtHostManager;
use crate::commands::ide::TerminalRegistry;
use crate::gateway::GatewayManager;
use crate::lsp::manager::LspManager;

/// Application state managed by Tauri and injected into commands via
/// `tauri::State<'_, AppState>`.
#[derive(Clone)]
pub struct AppState {
    /// Host-issued opened local project identities, never renderer-assembled paths.
    pub project_scopes: Arc<std::sync::Mutex<crate::runtime::project_scope::ProjectScopeRegistry>>,
    /// Execution/journal read leases exclude whole-checkout relocation.
    pub environment_access: agentz_task_state::access::EnvironmentAccess,
    pub task_activity: crate::runtime::task_activity::TaskActivity,
    pub repository_access: agentz_task_state::access::RepositoryAccess,
    /// Live PTY terminal sessions, keyed by frontend-generated terminal id.
    pub terminals: Arc<Mutex<TerminalRegistry>>,
    /// Active filesystem watchers, keyed by project directory.
    pub file_watchers: Arc<Mutex<HashMap<String, OwnedFileWatcher>>>,
    /// Language-server lifecycle manager (one bridge per project+language).
    pub lsp_manager: Arc<LspManager>,
    /// Cancel flag for the in-flight chat turn in the (sequential) IDE chat
    /// panel. `Some` while a turn is running; the chat panel's Stop flips it.
    pub chat_cancel: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    /// Per-task cancel flags for parallel Agent tasks (M7), keyed by a
    /// frontend-supplied task key so each isolated task can be stopped
    /// independently without affecting its siblings.
    pub task_cancel: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    /// Bounds how many Agent turns run concurrently (M7 task queue). Extra
    /// submissions wait here until a permit frees up, so a flood of parallel
    /// tasks degrades gracefully instead of starving the machine.
    pub agent_slots: Arc<Semaphore>,
    /// Registered background processes (builds, tests, dev servers) shared
    /// into every turn's tool registry; survives individual turns.
    pub background_runs: Arc<piscis_kernel::background::BackgroundRegistry>,
    /// Turns background-task completions into session continuations.
    pub bg_hub: Arc<crate::runtime::background_continuation::BgContinuationHub>,
    /// Per-session plan todo list state for `plan_todo`.
    pub plan_state: PlanStore,
    /// Pending `chat_ui` / `chat_ui_listen` response channels keyed by request id.
    pub interactive_responses: Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>>,
    /// Pending approval channels for shell/file-write policy warnings.
    pub permission_responses: ConfirmationResponseMap,
    pub approval_broker: agentz_task_state::approval_broker::ApprovalBroker,
    /// Cancellation flags for low-latency editor completion requests, keyed
    /// by frontend-generated request id.
    pub completion_cancel: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    /// Embedded Chromium session shared by the Browser panel and the agent
    /// `browser` tool (lazily launched on first use).
    pub browser: BrowserManager,
    /// Tracks agent turns and recent browser tool usage for close-guard UX.
    pub browser_activity: BrowserActivity,
    /// VS Code extension host sidecar lifecycle (Node process + RPC stdin).
    pub ext_host: Arc<ExtHostManager>,
    /// Debug Adapter Protocol broker (one active adapter at a time).
    pub dap: Arc<DapManager>,
    /// Ephemeral terminal text selections keyed by uuid (`@terminal-snippet(id)`).
    pub terminal_snippets: Arc<Mutex<HashMap<String, String>>>,
    /// Per-panel filesystem roots used only by the isolated extension webview protocol.
    pub webview_resource_roots: Arc<std::sync::Mutex<HashMap<String, Vec<PathBuf>>>>,
    /// IM gateway manager (Phase 0A "assistants"): registers Feishu / WeCom /
    /// DingTalk / WeChat etc. channels and routes inbound messages to a headless
    /// agent turn. Shared with the inbound consumer loop spawned at startup.
    pub gateway: Arc<GatewayManager>,
}

/// Invalidates callbacks before a local watcher and its index worker are released.
pub struct OwnedFileWatcher {
    pub watcher: notify::RecommendedWatcher,
    pub active: Arc<AtomicBool>,
    owners: usize,
}

impl OwnedFileWatcher {
    pub fn new(watcher: notify::RecommendedWatcher, active: Arc<AtomicBool>) -> Self {
        Self {
            watcher,
            active,
            owners: 1,
        }
    }

    pub fn retain(&mut self) -> Result<(), String> {
        self.owners = self
            .owners
            .checked_add(1)
            .ok_or("too many local watcher owners")?;
        Ok(())
    }

    fn release(&mut self) -> bool {
        self.owners -= 1;
        self.owners == 0
    }
}

/// Returns true only when the final owner releases the watcher.
pub fn release_file_watcher(
    watchers: &mut HashMap<String, OwnedFileWatcher>,
    project_dir: &str,
) -> bool {
    let Some(watcher) = watchers.get_mut(project_dir) else {
        return false;
    };
    if !watcher.release() {
        return false;
    }
    watchers.remove(project_dir);
    true
}

#[cfg(test)]
mod watcher_ownership_tests {
    use super::*;

    #[test]
    fn shared_local_watcher_stays_active_until_final_release() {
        let watcher = notify::recommended_watcher(|_: Result<notify::Event, notify::Error>| {})
            .expect("create watcher");
        let active = Arc::new(AtomicBool::new(true));
        let mut watchers = HashMap::new();
        watchers.insert(
            "project".to_owned(),
            OwnedFileWatcher::new(watcher, Arc::clone(&active)),
        );
        watchers.get_mut("project").unwrap().retain().unwrap();
        assert!(!release_file_watcher(&mut watchers, "project"));
        assert!(active.load(Ordering::Acquire));
        assert_eq!(watchers.len(), 1);
        assert!(release_file_watcher(&mut watchers, "project"));
        assert!(!active.load(Ordering::Acquire));
        assert!(watchers.is_empty());
        assert!(!release_file_watcher(&mut watchers, "project"));
    }
}

impl Drop for OwnedFileWatcher {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

/// Default cap on concurrently running Agent turns. Overridable via the
/// `CODEZ_AGENT_CONCURRENCY` environment variable.
const DEFAULT_AGENT_CONCURRENCY: usize = 3;

fn agent_concurrency() -> usize {
    std::env::var("CODEZ_AGENT_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or(DEFAULT_AGENT_CONCURRENCY)
}

impl AppState {
    pub fn new() -> Self {
        Self {
            project_scopes: Arc::new(std::sync::Mutex::new(
                crate::runtime::project_scope::ProjectScopeRegistry::default(),
            )),
            environment_access: agentz_task_state::access::EnvironmentAccess::default(),
            task_activity: crate::runtime::task_activity::TaskActivity::default(),
            repository_access: agentz_task_state::access::RepositoryAccess::default(),
            terminals: Arc::new(Mutex::new(TerminalRegistry::new())),
            file_watchers: Arc::new(Mutex::new(HashMap::new())),
            lsp_manager: Arc::new(LspManager::with_bridge_name("AgentZ")),
            chat_cancel: Arc::new(Mutex::new(None)),
            task_cancel: Arc::new(Mutex::new(HashMap::new())),
            agent_slots: Arc::new(Semaphore::new(agent_concurrency())),
            background_runs: Arc::new(piscis_kernel::background::BackgroundRegistry::new()),
            bg_hub: Arc::new(
                crate::runtime::background_continuation::BgContinuationHub::new(),
            ),
            plan_state: new_plan_store(),
            interactive_responses: Arc::new(Mutex::new(HashMap::new())),
            permission_responses: Arc::new(Mutex::new(HashMap::new())),
            approval_broker: agentz_task_state::approval_broker::ApprovalBroker::new(
                uuid::Uuid::new_v4().to_string(),
            ),
            completion_cancel: Arc::new(Mutex::new(HashMap::new())),
            browser: BrowserManager::new(),
            browser_activity: BrowserActivity::default(),
            ext_host: crate::remote::register_manager(Arc::new(ExtHostManager::new())),
            dap: Arc::new(DapManager::new()),
            terminal_snippets: Arc::new(Mutex::new(HashMap::new())),
            webview_resource_roots: Arc::new(std::sync::Mutex::new(HashMap::new())),
            gateway: Arc::new(GatewayManager::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
