//! Durable execution approvals composed with the existing journal hooks.
use agentz_task_state::{
    approval::{ApprovalStore, Binding, Intent, Status},
    approval_broker::ApprovalBroker,
    inbox::{InboxStore, Notice, NoticeKind},
};
use async_trait::async_trait;
use piscis_core::host::EventSink;
use piscis_kernel::agent::{
    hooks::{AgentHooks, ContextHookEvent, HookDecision, ToolHookEvent},
    tool::{Tool, ToolContext, ToolRegistry, ToolResult},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct AutoApprovalPolicy {
    pub policy_mode: String,
    pub confirm_shell_commands: bool,
    pub confirm_file_writes: bool,
    pub always_allow_tools: Vec<String>,
}

impl Default for AutoApprovalPolicy {
    fn default() -> Self {
        Self {
            policy_mode: "balanced".into(),
            confirm_shell_commands: true,
            confirm_file_writes: true,
            always_allow_tools: Vec::new(),
        }
    }
}

pub struct ApprovalHooks {
    project: PathBuf,
    workspace: PathBuf,
    binding: Binding,
    require_binding: bool,
    projectless_app_data: Option<PathBuf>,
    broker: ApprovalBroker,
    sink: Arc<dyn EventSink>,
    next: Arc<dyn AgentHooks>,
    registry: Arc<ToolRegistry>,
    cancel: Arc<AtomicBool>,
    interactive: bool,
    auto_approval: AutoApprovalPolicy,
    claimed: Mutex<HashMap<String, (String, Intent)>>,
    delegated: Mutex<HashMap<String, bool>>,
}
fn now() -> Result<i64, String> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis(),
    )
    .map_err(|e| e.to_string())
}
/// Runners without protected approval IPC cannot inherit the parent's grants.
/// Keep a conservative read-only surface until they use the durable issuer.
pub fn restrict_unapproved_background(registry: &mut ToolRegistry) {
    restrict_background_with_read_roots(registry, Vec::new());
}
pub fn restrict_background_with_read_roots(registry: &mut ToolRegistry, roots: Vec<PathBuf>) {
    let roots = Arc::new(
        roots
            .into_iter()
            .filter_map(|root| std::fs::canonicalize(root).ok())
            .collect::<Vec<_>>(),
    );
    registry.retain(|tool| {
        let name = tool.name();
        tool.is_read_only()
            && !name.starts_with("mcp_")
            && !matches!(
                name,
                "shell"
                    | "bash"
                    | "powershell"
                    | "powershell_query"
                    | "browser"
                    | "uia"
                    | "com_invoke"
                    | "app_control"
                    | "wmi"
                    | "api_connector"
                    | "code_run"
                    | "delegate"
                    | "call_fish"
                    | "pool_chat"
                    | "pool_org"
                    | "read_lints"
                    | "lsp"
            )
            && !tool.needs_confirmation(&json!({}))
    });
    let source = Arc::new(std::mem::replace(registry, ToolRegistry::new()));
    for tool in source.all() {
        registry.register(Box::new(BackgroundReadTool {
            name: tool.name().into(),
            source: source.clone(),
            roots: roots.clone(),
        }));
    }
}
struct BackgroundReadTool {
    name: String,
    source: Arc<ToolRegistry>,
    roots: Arc<Vec<PathBuf>>,
}
impl BackgroundReadTool {
    fn tool(&self) -> &dyn Tool {
        self.source
            .get(&self.name)
            .expect("immutable background registry")
    }
}
#[async_trait]
impl Tool for BackgroundReadTool {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        self.tool().description()
    }
    fn description_minimal(&self) -> std::borrow::Cow<'_, str> {
        self.tool().description_minimal()
    }
    fn input_schema(&self) -> Value {
        self.tool().input_schema()
    }
    fn input_schema_minimal(&self) -> Value {
        self.tool().input_schema_minimal()
    }
    fn is_read_only(&self) -> bool {
        true
    }
    fn needs_confirmation(&self, input: &Value) -> bool {
        self.tool().needs_confirmation(input)
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        if !self.tool().is_read_only() || self.tool().needs_confirmation(&input) {
            return Ok(ToolResult::err(
                "Background invocation requires protected approval; operation was not started",
            ));
        }
        if self.name == "lsp" {
            let path = input
                .get("file")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("LSP file target is missing"))?;
            let root = agentz_task_state::canonical_root(&ctx.workspace_root)
                .map_err(anyhow::Error::msg)?;
            if let Err(error) = validate_source_path(Path::new(&root), path) {
                if !Path::new(path).is_absolute()
                    || !self
                        .roots
                        .iter()
                        .any(|root| validate_source_path(root, path).is_ok())
                {
                    return Ok(ToolResult::err(error));
                }
            }
        }
        if matches!(
            self.name.as_str(),
            "file_read" | "file_search" | "file_list"
        ) {
            let path = input
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or(if self.name == "file_search" { "." } else { "" });
            let root = agentz_task_state::canonical_root(&ctx.workspace_root)
                .map_err(anyhow::Error::msg)?;
            if self.name == "file_read" {
                if let Err(error) = validate_source_path(Path::new(&root), path) {
                    if !Path::new(path).is_absolute()
                        || !self
                            .roots
                            .iter()
                            .any(|root| validate_source_path(root, path).is_ok())
                    {
                        return Ok(ToolResult::err(error));
                    }
                }
            } else {
                let search_root = match resolve_search_root(Path::new(&root), &self.roots, path) {
                    Ok(root) => root,
                    Err(error) => return Ok(ToolResult::err(error)),
                };
                if let Err(error) = check_search_scope(&self.name, &search_root, path, &input).await
                {
                    return Ok(ToolResult::err(error));
                }
            }
        }
        self.tool().call(input, ctx).await
    }
}
impl ApprovalHooks {
    pub async fn new(
        project: PathBuf,
        workspace: PathBuf,
        session: &str,
        broker: ApprovalBroker,
        sink: Arc<dyn EventSink>,
        next: Arc<dyn AgentHooks>,
        registry: Arc<ToolRegistry>,
        cancel: Arc<AtomicBool>,
        interactive: bool,
    ) -> Result<Self, String> {
        let run = uuid::Uuid::new_v4().to_string();
        Self::new_with_identity(
            project,
            workspace,
            session,
            broker,
            sink,
            next,
            registry,
            cancel,
            interactive,
            (&run, &run),
        )
        .await
    }

    /// Host-issued durable identities. Legacy callers keep their existing single-turn binding.
    pub async fn new_with_identity(
        project: PathBuf,
        workspace: PathBuf,
        session: &str,
        broker: ApprovalBroker,
        sink: Arc<dyn EventSink>,
        next: Arc<dyn AgentHooks>,
        registry: Arc<ToolRegistry>,
        cancel: Arc<AtomicBool>,
        interactive: bool,
        identity: (&str, &str),
    ) -> Result<Self, String> {
        Self::new_with_identity_scope(
            project,
            workspace,
            session,
            broker,
            sink,
            next,
            registry,
            cancel,
            interactive,
            identity,
            None,
            None,
        )
        .await
    }

    pub(crate) async fn new_with_identity_scope(
        project: PathBuf,
        workspace: PathBuf,
        session: &str,
        broker: ApprovalBroker,
        sink: Arc<dyn EventSink>,
        next: Arc<dyn AgentHooks>,
        registry: Arc<ToolRegistry>,
        cancel: Arc<AtomicBool>,
        interactive: bool,
        identity: (&str, &str),
        projectless_app_data: Option<PathBuf>,
        require_binding_override: Option<bool>,
    ) -> Result<Self, String> {
        let project = PathBuf::from(agentz_task_state::canonical_root(&project)?);
        let workspace = PathBuf::from(agentz_task_state::canonical_root(&workspace)?);
        let require_binding = match require_binding_override {
            Some(explicit) => explicit,
            None => workspace != project || agentz_task_state::load(&project, session)?.is_some(),
        };
        let mut binding = resolve_binding_for_mode(
            &project,
            &workspace,
            session,
            identity.0,
            require_binding,
            projectless_app_data.as_deref(),
        )
        .await?;
        binding.attempt_id = identity.1.into();
        binding.validate()?;
        Ok(Self {
            project,
            workspace,
            binding,
            require_binding,
            projectless_app_data,
            broker,
            sink,
            next,
            registry,
            cancel,
            interactive,
            auto_approval: AutoApprovalPolicy::default(),
            claimed: Mutex::new(HashMap::new()),
            delegated: Mutex::new(HashMap::new()),
        })
    }
    pub fn with_auto_approval(mut self, auto_approval: AutoApprovalPolicy) -> Self {
        self.auto_approval = auto_approval;
        self
    }
    fn is_auto_approved(&self, tool_name: &str) -> bool {
        if self.broker.is_always_allowed(tool_name) {
            return true;
        }
        if self.auto_approval.policy_mode.eq_ignore_ascii_case("dev") {
            return true;
        }
        let is_shell = matches!(
            tool_name,
            "shell" | "bash" | "powershell" | "powershell_query" | "code_run"
        );
        if !self.auto_approval.confirm_shell_commands && is_shell {
            return true;
        }
        self.auto_approval.always_allow_tools.iter().any(|allowed| {
            let trimmed = allowed.trim();
            trimmed == "*"
                || trimmed.eq_ignore_ascii_case(tool_name)
                || (is_shell
                    && matches!(
                        trimmed.to_ascii_lowercase().as_str(),
                        "shell" | "bash" | "powershell" | "powershell_query" | "code_run"
                    ))
        })
    }
    fn path(&self) -> PathBuf {
        self.project.join(".agentz").join("approvals.db")
    }
    pub(crate) fn binding(&self) -> &Binding {
        &self.binding
    }
    async fn fresh(&self, ev: &ToolHookEvent<'_>) -> Result<Binding, String> {
        if ev.session_id != self.binding.task_id
            || agentz_task_state::canonical_root(ev.workspace_root)? != self.workspace
        {
            return Err("Tool invocation belongs to a different task/environment".into());
        }
        let mut binding = resolve_binding_for_mode(
            &self.project,
            ev.workspace_root,
            ev.session_id,
            &self.binding.run_id,
            self.require_binding,
            self.projectless_app_data.as_deref(),
        )
        .await?;
        binding.attempt_id = self.binding.attempt_id.clone();
        if binding != self.binding {
            return Err(
                "Task environment binding changed; restart with a newly authorized context".into(),
            );
        }
        Ok(binding)
    }
    async fn authorize(&self, ev: &ToolHookEvent<'_>) -> Result<(), String> {
        let binding = self.fresh(ev).await?;
        if self.cancel.load(Ordering::Relaxed) {
            return Err("Attempt cancelled".into());
        }
        let tool = self
            .registry
            .get(ev.tool_name)
            .ok_or("Tool provider no longer available")?;
        let ordinary_file = matches!(ev.tool_name, "file_read" | "file_write" | "file_edit");
        if ev.tool_name == "lsp" {
            validate_source_path(
                &self.workspace,
                ev.input
                    .get("file")
                    .and_then(Value::as_str)
                    .ok_or("LSP file target is missing")?,
            )?;
        }
        if ev.tool_name == "read_lints" {
            let paths = ev
                .input
                .get("paths")
                .and_then(Value::as_array)
                .filter(|paths| !paths.is_empty())
                .ok_or("Diagnostics require explicit file targets")?;
            for path in paths {
                validate_source_path(
                    &self.workspace,
                    path.as_str()
                        .ok_or("Diagnostic file target must be a string")?,
                )?;
            }
        }
        if matches!(ev.tool_name, "file_search" | "file_list") {
            let path = ev
                .input
                .get("path")
                .map(|v| v.as_str().ok_or("Search target must be a path string"))
                .transpose()?
                .unwrap_or(".");
            check_search_scope(ev.tool_name, &self.workspace, path, ev.input).await?;
        }
        if ordinary_file {
            validate_source_path(
                &self.workspace,
                ev.input
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("File target is missing")?,
            )?;
        }
        let capability_tool = matches!(
            ev.tool_name,
            "shell"
                | "bash"
                | "powershell"
                | "powershell_query"
                | "browser"
                | "uia"
                | "com_invoke"
                | "app_control"
                | "wmi"
                | "read_lints"
                | "lsp"
        ) || ev.tool_name.starts_with("mcp_");
        let internal = matches!(
            ev.tool_name,
            "plan_todo" | "chat_ui" | "chat_ui_listen" | "ask_user"
        );
        if ordinary_file
            || internal
            || (!capability_tool && tool.is_read_only() && !tool.needs_confirmation(ev.input))
            || self.is_auto_approved(ev.tool_name)
        {
            return Ok(());
        }
        if !self.interactive {
            return Err(
                "This background attempt has no exact resource approval; operation was not started"
                    .into(),
            );
        }
        let resource =
            json!({"workspace":self.workspace,"toolUseId":ev.tool_use_id,"provider":ev.tool_name});
        let (intent, rx) = self
            .broker
            .request(
                self.path(),
                format!("approval:{}", uuid::Uuid::new_v4()),
                binding,
                ev.tool_name.into(),
                resource.clone(),
                ev.input.clone(),
                now()?,
                60_000,
            )
            .await?;
        let mut pending = PendingGuard {
            broker: self.broker.clone(),
            path: self.path(),
            intent: Some(intent.clone()),
        };
        let inbox_project = self.project.clone();
        let notice_key = format!("approval:{}", intent.request_id);
        let notice = Notice {
            event_key: notice_key.clone(),
            task_id: intent.binding.task_id.clone(),
            kind: NoticeKind::ActionRequired,
            summary: "Approval required".into(),
            created_ms: intent.created_ms,
            read_ms: None,
        };
        match tokio::task::spawn_blocking(move || InboxStore::open(&inbox_project)?.post(&notice))
            .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => tracing::warn!(%error, "approval inbox notice unavailable"),
            Err(error) => tracing::warn!(%error, "approval inbox worker unavailable"),
        }
        self.sink.emit_session(
            ev.session_id,
            "agent_lifecycle",
            json!({"state":"waiting_permission"}),
        );
        // Event is a wakeup only. The card reads the immutable preview over protected IPC.
        self.sink.emit_session(ev.session_id,"agent_event",json!({"type":"permission_request","request_id":intent.request_id,
            "tool_name":ev.tool_name,"tool_input":Value::Null,"description":"Review the exact host-issued action",
            "approval":{"digest":intent.digest,"taskId":intent.binding.task_id,"environmentId":intent.binding.environment_id,"expiresMs":intent.expires_ms}}));
        let decision = tokio::select! {
            result=tokio::time::timeout(Duration::from_secs(60),rx) => matches!(result,Ok(Ok(true))),
            _=async {while !self.cancel.load(Ordering::Relaxed) {tokio::time::sleep(Duration::from_millis(100)).await;}} => false,
        };
        let inbox_project = self.project.clone();
        if let Ok(read_at) = now() {
            match tokio::task::spawn_blocking(move || {
                InboxStore::open(&inbox_project)?.mark_read(&notice_key, read_at)
            })
            .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => tracing::warn!(%error, "approval inbox resolution unavailable"),
                Err(error) => {
                    tracing::warn!(%error, "approval inbox resolution worker unavailable")
                }
            }
        }
        self.sink.emit_session(
            ev.session_id,
            "agent_lifecycle",
            json!({"state":"executing"}),
        );
        if !decision || self.cancel.load(Ordering::Relaxed) {
            let _ = self
                .broker
                .cancel(self.path(), intent.clone(), now()?)
                .await;
            pending.intent = None;
            return Err(
                "Approval denied, expired or attempt cancelled; operation was not started".into(),
            );
        }
        let actual = self.fresh(ev).await?;
        let path = self.path();
        let boot = self.broker.boot_id().to_string();
        let expected = intent.clone();
        let action = ev.tool_name.to_string();
        let input = ev.input.clone();
        let timestamp = now()?;
        tokio::task::spawn_blocking(move || {
            ApprovalStore::open_existing(&path, &boot, timestamp)?
                .claim(&expected, &actual, &action, &resource, &input, timestamp)
        })
        .await
        .map_err(|e| format!("approval claim worker: {e}"))??;
        pending.intent = None;
        if self.cancel.load(Ordering::Relaxed) {
            self.finish(intent, Status::Failed).await?;
            return Err("Attempt cancelled before tool execution".into());
        }
        let duplicate = {
            let mut claimed = self
                .claimed
                .lock()
                .map_err(|_| "Approval execution map poisoned")?;
            if claimed.contains_key(ev.tool_use_id) {
                true
            } else {
                claimed.insert(ev.tool_use_id.into(), (invocation_key(ev), intent.clone()));
                false
            }
        };
        if duplicate {
            self.finish(intent, Status::Failed).await?;
            return Err("Concurrent duplicate tool-use identity".into());
        }
        Ok(())
    }
    async fn finish(&self, intent: Intent, status: Status) -> Result<(), String> {
        let path = self.path();
        let boot = self.broker.boot_id().to_string();
        let timestamp = now()?;
        tokio::task::spawn_blocking(move || {
            ApprovalStore::open_existing(&path, &boot, timestamp)?
                .finish(&intent, status, timestamp)
        })
        .await
        .map_err(|e| format!("approval result worker: {e}"))?
    }
}
#[async_trait]
impl AgentHooks for ApprovalHooks {
    async fn before_tool(&self, ev: &ToolHookEvent<'_>) -> HookDecision {
        if let Err(reason) = self.authorize(ev).await {
            return HookDecision::Deny(reason);
        }
        let mut decision = self.next.before_tool(ev).await;
        if matches!(decision, HookDecision::Continue)
            && matches!(ev.tool_name, "file_search" | "file_list")
        {
            let path = ev.input.get("path").and_then(Value::as_str).unwrap_or(".");
            if let Err(error) = validate_search_path(&self.workspace, path) {
                decision = HookDecision::Deny(error);
            }
        }
        if let Ok(mut map) = self.delegated.lock() {
            map.insert(
                invocation_key(ev),
                matches!(decision, HookDecision::Continue),
            );
        } else {
            return HookDecision::Deny("Execution lifecycle map poisoned".into());
        }
        decision
    }
    async fn after_tool(&self, ev: &ToolHookEvent<'_>, result: &ToolResult) {
        let delegated = self
            .delegated
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&invocation_key(ev)));
        let Some(continued) = delegated else {
            return;
        };
        self.next.after_tool(ev, result).await;
        let intent = self.claimed.lock().ok().and_then(|mut map| {
            if map
                .get(ev.tool_use_id)
                .is_some_and(|(key, _)| *key == invocation_key(ev))
            {
                map.remove(ev.tool_use_id).map(|(_, intent)| intent)
            } else {
                None
            }
        });
        if let Some(intent) = intent {
            // An external tool can report an error after a side effect: conservatively unknown.
            if let Err(error) = self
                .finish(
                    intent,
                    if !continued {
                        Status::Failed
                    } else if result.is_error {
                        Status::Unknown
                    } else {
                        Status::Executed
                    },
                )
                .await
            {
                tracing::error!(%error,"approval execution result could not be persisted; do not retry the operation");
            }
        }
    }
    async fn on_context_event(&self, ev: &ContextHookEvent<'_>) {
        self.next.on_context_event(ev).await;
    }
}
fn invocation_key(ev: &ToolHookEvent<'_>) -> String {
    let identity = json!([
        ev.session_id,
        ev.tool_use_id,
        ev.tool_name,
        ev.input,
        ev.workspace_root
    ]);
    format!("{:x}", Sha256::digest(identity.to_string().as_bytes()))
}
impl Drop for ApprovalHooks {
    fn drop(&mut self) {
        let intents = self
            .claimed
            .get_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        for (_, (_, intent)) in intents {
            let path = self.path();
            let boot = self.broker.boot_id().to_string();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn_blocking(move || {
                    if let Ok(timestamp) = now() {
                        let _ = ApprovalStore::open_existing(&path, &boot, timestamp).and_then(
                            |mut store| store.finish(&intent, Status::Unknown, timestamp),
                        );
                    }
                });
            }
        }
    }
}
struct PendingGuard {
    broker: ApprovalBroker,
    path: PathBuf,
    intent: Option<Intent>,
}
impl Drop for PendingGuard {
    fn drop(&mut self) {
        if let (Some(intent), Ok(handle)) =
            (self.intent.take(), tokio::runtime::Handle::try_current())
        {
            let broker = self.broker.clone();
            let path = self.path.clone();
            handle.spawn(async move {
                if let Ok(timestamp) = now() {
                    let _ = broker.cancel(path, intent, timestamp).await;
                }
            });
        }
    }
}
#[cfg(test)]
pub(crate) async fn resolve_binding(
    project: &Path,
    workspace: &Path,
    session: &str,
    run: &str,
    require_binding: bool,
) -> Result<Binding, String> {
    resolve_binding_for_mode(project, workspace, session, run, require_binding, None).await
}

pub(crate) async fn resolve_binding_for_mode(
    project: &Path,
    workspace: &Path,
    session: &str,
    run: &str,
    require_binding: bool,
    projectless_app_data: Option<&Path>,
) -> Result<Binding, String> {
    if let Some(app_data) = projectless_app_data {
        if agentz_task_state::load(project, session)?.is_some() {
            return Err("projectless session conflicts with a registered environment".into());
        }
        let bound = agentz_task_state::projectless::AppDataStore::open_existing(app_data)?
            .reopen_session(project, session)?;
        if agentz_task_state::canonical_root(&bound)?
            != agentz_task_state::canonical_root(workspace)?
        {
            return Err("projectless session directory binding changed".into());
        }
    }
    let resolved = crate::commands::agent_task::resolve_task_workspace(
        project,
        Some(session),
        Some(&workspace.to_string_lossy()),
        require_binding && projectless_app_data.is_none(),
    )
    .await?;
    let root = agentz_task_state::canonical_root(Path::new(&resolved))?;
    let record = agentz_task_state::load(project, session)?;
    let direct_git = if record.is_none() {
        direct_git_identity(Path::new(&root)).await?
    } else {
        Value::Null
    };
    let version =
        serde_json::to_string(&json!({"root":root,"record":record,"directGit":direct_git}))
            .map_err(|e| e.to_string())?;
    let fingerprint = format!("{:x}", Sha256::digest(version.as_bytes()));
    Ok(Binding {
        project_id: agentz_task_state::canonical_root(project)?,
        task_id: session.into(),
        environment_id: record
            .map(|record| record.environment_id)
            .unwrap_or_else(|| format!("direct:{fingerprint}")),
        binding_version: fingerprint,
        run_id: run.into(),
        attempt_id: run.into(),
        capability_version: "agentz-hook-v1:piscis-v0.8.68".into(),
    })
}
async fn direct_git_identity(root: &Path) -> Result<Value, String> {
    // Non-Git source folders remain usable without an installed Git executable.
    // A newly created or removed marker changes this identity on the next check.
    if !root
        .ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
    {
        return Ok(Value::Null);
    }
    async fn git(root: &Path, args: &[&str]) -> Result<std::process::Output, String> {
        tokio::time::timeout(
            Duration::from_secs(10),
            piscis_kernel::proc::tokio_command("git")
                .args(crate::commands::ide::GIT_UNQUOTED_PATHS)
                .args(args)
                .env("LC_ALL", "C")
                .current_dir(root)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| "Git approval identity timed out".to_string())?
        .map_err(|e| format!("Git approval identity unavailable: {e}"))
    }
    let repo = git(
        root,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ],
    )
    .await?;
    if !repo.status.success() {
        return Err("Cannot verify direct Git repository identity".into());
    }
    let branch = git(root, &["symbolic-ref", "--quiet", "HEAD"]).await?;
    let head = if branch.status.success() {
        branch
    } else {
        let detached = git(root, &["rev-parse", "--verify", "HEAD"]).await?;
        if !detached.status.success() {
            return Err("Cannot verify direct Git HEAD identity".into());
        }
        detached
    };
    Ok(
        json!({"repository":String::from_utf8_lossy(&repo.stdout).trim(),"head":String::from_utf8_lossy(&head.stdout).trim()}),
    )
}
async fn check_search_links(
    name: &str,
    root: &Path,
    raw: &str,
    input: &Value,
) -> Result<(), String> {
    let depth = if name == "file_list" && !input["recursive"].as_bool().unwrap_or(false) {
        0
    } else {
        input["max_depth"]
            .as_u64()
            .unwrap_or(if name == "file_list" { 3 } else { 10 })
    };
    if depth > 14 {
        return Err("Search depth exceeds the bounded source traversal limit".into());
    }
    let target = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
    let skip_noise = name == "file_search";
    let skip_hidden = name == "file_list" && !input["include_hidden"].as_bool().unwrap_or(false);
    tokio::task::spawn_blocking(move || {
        let mut pending = vec![(target, 0u64)];
        let mut budget = 20_000usize;
        while let Some((dir, level)) = pending.pop() {
            let meta = std::fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() {
                return Err("Linked source traversal requires a separately resolved scope".into());
            }
            if !meta.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                budget = budget
                    .checked_sub(1)
                    .ok_or("Source traversal exceeds the bounded entry limit")?;
                let kind = entry.file_type().map_err(|e| e.to_string())?;
                if kind.is_symlink() {
                    return Err(
                        "Linked source traversal requires a separately resolved scope".into(),
                    );
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if (skip_hidden && name.starts_with('.'))
                    || (skip_noise
                        && kind.is_dir()
                        && piscis_kernel::tools::file_search::is_excluded_search_directory(&name))
                {
                    continue;
                }
                let protected = name.to_lowercase();
                if matches!(protected.as_str(), ".git" | ".agentz" | ".codez" | ".piscis" | ".secret_key" | ".env") || protected.starts_with(".env.") {
                    // Skip, don't fail the whole search: the search tool itself also
                    // never reads these entries (see file_search::is_secret_or_metadata).
                    continue;
                }
                if kind.is_dir() && level < depth {
                    pending.push((entry.path(), level + 1));
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("Search scope worker failed: {e}"))?
}
fn resolve_search_root(
    workspace: &Path,
    additional_roots: &[PathBuf],
    raw: &str,
) -> Result<PathBuf, String> {
    if validate_search_path(workspace, raw).is_ok() {
        return Ok(workspace.to_path_buf());
    }
    if Path::new(raw).is_absolute() {
        if let Some(root) = additional_roots
            .iter()
            .find(|root| validate_search_path(root, raw).is_ok())
        {
            return Ok(root.clone());
        }
    }
    Err("Search target must be within an authorized source root".into())
}
async fn check_search_scope(
    name: &str,
    root: &Path,
    raw: &str,
    input: &Value,
) -> Result<(), String> {
    check_search_scope_with_hook(name, root, raw, input, || {}).await
}
async fn check_search_scope_with_hook(
    name: &str,
    root: &Path,
    raw: &str,
    input: &Value,
    after_preflight: impl FnOnce() + Send,
) -> Result<(), String> {
    validate_search_path(root, raw)?;
    check_search_links(name, root, raw, input).await?;
    after_preflight();
    validate_search_path(root, raw)
}
fn validate_search_path(root: &Path, raw: &str) -> Result<(), String> {
    if raw.is_empty()
        || Path::new(raw)
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("Search target must be within the authorized source root".into());
    }
    let target = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
    if std::fs::canonicalize(&target).map_err(|e| e.to_string())? == root {
        return Ok(());
    }
    validate_source_path(root, raw)
}
fn validate_source_path(root: &Path, raw: &str) -> Result<(), String> {
    let path = Path::new(raw);
    if raw.is_empty()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("File target must be within the authorized source root".into());
    }
    let target = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let mut ancestor = target.as_path();
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        suffix.push(
            ancestor
                .file_name()
                .ok_or("Invalid file target")?
                .to_owned(),
        );
        ancestor = ancestor
            .parent()
            .ok_or("File target has no existing parent")?;
    }
    let mut resolved = std::fs::canonicalize(ancestor).map_err(|e| e.to_string())?;
    for part in suffix.into_iter().rev() {
        resolved.push(part);
    }
    let relative = resolved
        .strip_prefix(root)
        .map_err(|_| "File target escapes the authorized environment")?;
    if relative.as_os_str().is_empty()
        || relative.components().any(|c| {
            let part = c.as_os_str().to_string_lossy().to_lowercase();
            matches!(
                part.as_str(),
                ".git" | ".agentz" | ".codez" | ".piscis" | ".secret_key" | ".env"
            ) || part.starts_with(".env.")
        })
    {
        return Err(
            "Host metadata and credential files are outside ordinary source permissions".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn projectless_binding_isolated_and_lost_catalog_refused() {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                assert!(self.0.starts_with(std::env::temp_dir()));
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "agentz-projectless-approval-{}",
            uuid::Uuid::new_v4()
        )));
        let app_data = fixture.0.join("app-data");
        let project = fixture.0.join("free-sessions");
        std::fs::create_dir_all(&app_data).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        let store = agentz_task_state::projectless::AppDataStore::initialize(&app_data).unwrap();
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        let root_a = store.create_task(&a).unwrap();
        let root_b = store.create_task(&b).unwrap();
        store.bind_session(&project, &a, &a).unwrap();
        store.bind_session(&project, &b, &b).unwrap();
        let bound = resolve_binding_for_mode(&project, &root_a, &a, "run-a", true, Some(&app_data))
            .await
            .unwrap();
        assert_eq!(bound.task_id, a);
        assert!(
            resolve_binding_for_mode(&project, &root_b, &a, "run-a", true, Some(&app_data))
                .await
                .is_err()
        );
        assert!(
            resolve_binding_for_mode(&project, &root_a, &b, "run-b", true, Some(&app_data))
                .await
                .is_err()
        );
        assert!(
            resolve_binding_for_mode(&project, &root_a, &a, "run-a", true, None)
                .await
                .is_err()
        );
        drop(store);
        std::fs::remove_file(app_data.join("projectless/catalog.sqlite")).unwrap();
        assert!(
            resolve_binding_for_mode(&project, &root_a, &a, "run-a", true, Some(&app_data))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn search_preflight_and_kernel_share_excluded_directories() {
        struct TempRoot(PathBuf);
        impl Drop for TempRoot {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let workspace =
            std::env::temp_dir().join(format!("agentz-search-excludes-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&workspace).expect("create search exclusion fixture");
        let _workspace = TempRoot(workspace.clone());
        std::fs::create_dir(workspace.join("src")).expect("create visible source directory");
        std::fs::write(workspace.join("src/visible.rs"), "AZ081_VISIBLE_MARKER")
            .expect("write visible search marker");
        for name in piscis_kernel::tools::file_search::EXCLUDED_SEARCH_DIRECTORY_NAMES {
            let excluded = workspace.join(name);
            std::fs::create_dir(&excluded).expect("create excluded directory");
            std::fs::write(excluded.join("hidden.rs"), format!("AZ081_EXCLUDED_{name}"))
                .expect("write excluded marker");
        }

        let input = json!({"action":"grep","pattern":"AZ081_","path":".","max_depth":5});
        check_search_links("file_search", &workspace, ".", &input)
            .await
            .expect("host preflight should skip every excluded directory");
        let ctx = ToolContext {
            session_id: "az081-exclude-fixture".into(),
            workspace_root: workspace,
            bypass_permissions: false,
            settings: Arc::new(piscis_kernel::agent::tool::ToolSettings::default()),
            max_iterations: None,
            memory_owner_id: "fixture".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        };
        let result = piscis_kernel::tools::file_search::FileSearchTool
            .call(input, &ctx)
            .await
            .expect("run fixed kernel file_search");
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("AZ081_VISIBLE_MARKER"));
        for name in piscis_kernel::tools::file_search::EXCLUDED_SEARCH_DIRECTORY_NAMES {
            assert!(
                !result.content.contains(&format!("AZ081_EXCLUDED_{name}")),
                "kernel search returned content from excluded directory {name}"
            );
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn search_scope_revalidation_rejects_junction_replacement_after_preflight() {
        for name in ["file_search", "file_list"] {
            struct TempRoot(PathBuf);
            impl Drop for TempRoot {
                fn drop(&mut self) {
                    let _ = std::fs::remove_dir_all(&self.0);
                }
            }

            let temp = std::env::temp_dir()
                .join(format!("agentz-search-junction-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&temp).expect("create path replacement test root");
            let _temp_root = TempRoot(temp.clone());
            let workspace = temp.join("workspace");
            let source = workspace.join("src");
            let outside = temp.join("outside");
            std::fs::create_dir_all(&source).expect("create approved source directory");
            std::fs::create_dir(&outside).expect("create out-of-scope target");
            std::fs::write(source.join("safe.rs"), "safe marker")
                .expect("write approved source fixture");
            std::fs::write(outside.join("secret.rs"), "AZ080_OUTSIDE_SECRET")
                .expect("write outside canary");
            let workspace = std::fs::canonicalize(workspace).expect("canonicalize workspace");
            let outside = std::fs::canonicalize(outside).expect("canonicalize outside target");
            let replaced_source = source.with_file_name("src-before-junction");
            let junction = source.clone();
            let input = json!({"recursive":true,"max_depth":4});

            let result = check_search_scope_with_hook(
                name,
                &workspace,
                "src",
                &input,
                move || {
                    std::fs::rename(&junction, &replaced_source)
                        .expect("move checked directory before provider read");
                    let output = std::process::Command::new("powershell.exe")
                        .args([
                            "-NoProfile",
                            "-NonInteractive",
                            "-Command",
                            "New-Item -ItemType Junction -Path $env:AGENTZ_TEST_LINK -Target $env:AGENTZ_TEST_TARGET -ErrorAction Stop | Out-Null",
                        ])
                        .env("AGENTZ_TEST_LINK", &junction)
                        .env("AGENTZ_TEST_TARGET", &outside)
                        .output()
                        .expect("create junction after completed preflight");
                    assert!(
                        output.status.success(),
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                },
            )
            .await;
            assert!(result.is_err(), "{name} must reject the replaced source");
            let provider_started = result.is_ok();
            assert!(
                !provider_started,
                "provider must not receive out-of-scope path"
            );
        }
    }
}
