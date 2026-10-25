//! AgentZ-specific agent turn runner — extends the headless kernel path with
//! runtime model override, vision attachment injection, and plan-mode tooling.

pub mod post_turn;
pub mod prompt_assembly;
pub mod tool_approval;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::sync::{mpsc, Mutex};

use piscis_core::host::{EventSink, HeadlessCliMode, HeadlessCliRequest, HeadlessCliResponse};
use piscis_kernel::agent::harness::config::{CompactionSettings, HarnessConfig};
use piscis_kernel::agent::messages::AgentEvent;
use piscis_kernel::agent::plan::PlanStore;
use piscis_kernel::agent::tool::ToolContext;
use piscis_kernel::headless::KernelState;
use piscis_kernel::llm::{self, LlmMessage, MessageContent};
use piscis_kernel::policy::gate::PolicyGate;
use tauri::Manager;

use crate::browser::BrowserManager;
use crate::lsp::manager::LspManager;

use super::chat::FrontendAttachment;
use super::session::{
    maybe_autotitle_session_from_first_prompt, persist_workz_meta, validate_session_continuation,
};
use super::session_sources::{default_channel_for, SOURCE_CODEZ};
use super::system_prompt::swarm_coordinator_followup_reminder;

pub use post_turn::{model_supports_vision, resolve_attachment};
pub(crate) use prompt_assembly::{load_installed_skills, SkillManifest};
pub(crate) use tool_approval::{
    apply_llm_provider, materialize_headless_llm_settings, resolve_koi_llm_provider_for_turn,
    run_subagent_research, run_subagent_with_options, run_subagent_with_prompt, SubagentOptions,
};

use post_turn::{
    finalize_turn_and_spawn_background, inject_image_block, spawn_turn_event_collector,
    EventCollectorOutput,
};
use prompt_assembly::{
    build_turn_system_prompt, expand_browser_element_refs, expand_file_refs,
    expand_file_refs_remote, expand_terminal_snippets, skill_matches,
};
use tool_approval::{
    auto_route_model, build_tool_registry, register_mcp_and_connector_tools, resolve_llm_runtime,
    resolve_team_coordinator_model_id, snapshot_settings,
};

/// Ensures a turn that wrote `status=running` always clears it — including on
/// early `?` exits — so the WorkZ sidebar cannot keep a stale blinking LED.
pub(super) struct SessionRunningGuard {
    db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    session_id: String,
}

impl SessionRunningGuard {
    fn arm(db: Arc<Mutex<piscis_kernel::store::db::Database>>, session_id: String) -> Self {
        Self { db, session_id }
    }

    pub(super) async fn finish(self, failed: bool) {
        {
            let db = self.db.lock().await;
            let status = if failed { "error" } else { "idle" };
            if let Err(e) = db.update_session_status(&self.session_id, status) {
                tracing::warn!(
                    session_id = %self.session_id,
                    status,
                    "failed to finalize session status: {e}"
                );
            }
        }
        std::mem::forget(self);
    }
}

impl Drop for SessionRunningGuard {
    fn drop(&mut self) {
        let db = self.db.clone();
        let session_id = self.session_id.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let db = db.lock().await;
                if let Err(e) = db.update_session_status(&session_id, "idle") {
                    tracing::warn!(
                        session_id = %session_id,
                        "failed to heal session status after aborted turn: {e}"
                    );
                }
            });
        }
    }
}

#[tracing::instrument(
    name = "agent_turn",
    skip_all,
    fields(
        app_version = crate::diagnostics::APP_VERSION,
        run_id = tracing::field::Empty,
        task_id = tracing::field::Empty,
        environment_id = tracing::field::Empty,
    )
)]
#[allow(clippy::too_many_arguments)]
pub async fn run_agentz_turn(
    app: tauri::AppHandle,
    mut request: HeadlessCliRequest,
    kernel: KernelState,
    event_sink: Arc<dyn EventSink>,
    plan_store: PlanStore,
    confirmation_responses: Option<piscis_kernel::agent::loop_::ConfirmationResponseMap>,
    approval_project_dir: String,
    cancel: Arc<AtomicBool>,
    model_id: Option<String>,
    chat_mode: String,
    attachment: Option<FrontendAttachment>,
    clear_plan: bool,
    display_prompt: Option<String>,
    lsp_manager: Arc<LspManager>,
    browser: BrowserManager,
    journal: Arc<crate::journal::FileJournal>,
    config_dir: PathBuf,
    enabled_skills: Vec<String>,
    enabled_connectors: Option<Vec<String>>,
    agent_id: Option<String>,
    workz_team_id: Option<String>,
    workz_pool_id: Option<String>,
    prefer_zh: bool,
    queued_run: Option<crate::runtime::durable_run::ManagedRun>,
    background_registry: Option<Arc<piscis_kernel::background::BackgroundRegistry>>,
) -> Result<HeadlessCliResponse> {
    if clear_plan {
        return Err(anyhow!(
            "clearPlan=true 已废弃；普通续聊保留计划，新目标须使用显式接口"
        ));
    }
    if !matches!(request.mode, HeadlessCliMode::Piscis) {
        return Err(anyhow!("AgentZ chat only supports mode=piscis"));
    }

    let (db, settings) = kernel;
    let chat_mode = if chat_mode == "plan" { "plan" } else { "agent" };

    let (global_db, _global_settings) =
        super::data_scope::open_global_kernel_state(&app).map_err(|e| anyhow!(e))?;
    let skills_root = crate::skills::service::skills_root_from_config_dir(&config_dir);
    let skill_loader = Arc::new(Mutex::new(crate::skills::loader::SkillLoader::new(
        skills_root.clone(),
    )));
    {
        let mut loader = skill_loader.lock().await;
        let _ = loader.load_all();
    }

    let agent = agent_id
        .as_deref()
        .and_then(|id| crate::commands::agents::resolve_agent(&config_dir, id));
    let mut enabled_skills = enabled_skills;
    if let Some(agent) = agent.as_ref() {
        for slug in &agent.skills {
            if !enabled_skills.iter().any(|s| s.eq_ignore_ascii_case(slug)) {
                enabled_skills.push(slug.clone());
            }
        }
    }

    let model_id = match model_id.filter(|v| !v.trim().is_empty()) {
        Some(id) => Some(id),
        None => match agent
            .as_ref()
            .and_then(|a| a.llm_provider_id.clone())
            .filter(|v| !v.trim().is_empty())
        {
            Some(id) => Some(id),
            None => {
                if let Some(team_id) = workz_team_id.as_deref().filter(|s| !s.is_empty()) {
                    let guard = db.lock().await;
                    let s = settings.lock().await;
                    resolve_team_coordinator_model_id(&app, &guard, &s, team_id, &config_dir)
                } else {
                    let s = settings.lock().await;
                    auto_route_model(&s, chat_mode)
                }
            }
        },
    };

    let settings_snapshot = {
        let mut s = settings.lock().await;
        if let Some(id) = model_id.as_deref().filter(|v| !v.is_empty()) {
            let snap = snapshot_settings(&s);
            if let Some(prov) = s.find_llm_provider(id).cloned() {
                apply_llm_provider(&mut s, &prov);
            }
            Some(snap)
        } else {
            None
        }
    };

    let runtime = {
        let s = settings.lock().await;
        resolve_llm_runtime(&s, model_id.as_deref())?
    };

    let vision_capable = {
        let s = settings.lock().await;
        s.vision_enabled || model_supports_vision(&runtime.provider, &runtime.model)
    };

    let (mut effective_prompt, image_attachment) =
        resolve_attachment(&request.prompt, attachment, vision_capable)?;

    let mut slash_tool_allowlist: Option<Vec<String>> = None;
    let mut slash_display: Option<String> = None;
    if effective_prompt.trim_start().starts_with('/') {
        if let Ok(Some(resolved)) = crate::commands::slash_commands::resolve_command(
            &config_dir,
            &effective_prompt,
            prefer_zh,
        ) {
            effective_prompt = resolved.prompt;
            slash_display = Some(resolved.display);
            if !resolved.tools.is_empty() {
                slash_tool_allowlist = Some(resolved.tools);
            }
        }
    }
    request.prompt = effective_prompt.clone();

    let selected_skills: Vec<SkillManifest> = if enabled_skills.is_empty() {
        Vec::new()
    } else {
        load_installed_skills(&config_dir)
            .into_iter()
            .filter(|s| enabled_skills.iter().any(|e| skill_matches(s, e)))
            .collect()
    };
    let mut skill_enabled_tools: Vec<String> = selected_skills
        .iter()
        .flat_map(|s| s.tools.clone())
        .collect();
    let mut skill_mcp_names: std::collections::HashSet<String> = selected_skills
        .iter()
        .flat_map(|s| s.mcp_servers.clone())
        .collect();
    if let Some(agent) = agent.as_ref() {
        skill_enabled_tools.extend(agent.tools.iter().cloned());
        skill_mcp_names.extend(agent.mcp_servers.iter().cloned());
    }

    let loop_halt = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let budget_scope = crate::runtime::task_budget::BudgetScope::default();
    let delegation_bridge = crate::runtime::subagent_bridge::BridgeSlot::default();
    let api_connector_allowlist = enabled_connectors.clone();
    let mut registry = build_tool_registry(
        app.clone(),
        db.clone(),
        global_db.clone(),
        config_dir.clone(),
        skill_loader.clone(),
        settings.clone(),
        event_sink.clone(),
        plan_store.clone(),
        chat_mode,
        lsp_manager,
        Some(config_dir.join("user-tools")),
        &skill_enabled_tools,
        chat_mode != "plan",
        chat_mode == "agent",
        loop_halt.clone(),
        api_connector_allowlist,
        slash_tool_allowlist.clone(),
        budget_scope.clone(),
        background_registry,
        delegation_bridge.clone(),
    );

    register_mcp_and_connector_tools(
        &mut registry,
        &settings,
        &config_dir,
        &skill_mcp_names,
        &enabled_connectors,
        agent.as_ref(),
    )
    .await;

    let workspace_root = request
        .workspace
        .clone()
        .filter(|w| !w.trim().is_empty())
        .unwrap_or_default();
    let workspace_root = if workspace_root.is_empty() {
        settings.lock().await.workspace_root.clone()
    } else {
        workspace_root
    };
    let mut workspace_root =
        agentz_task_state::canonical_root(std::path::Path::new(&workspace_root))
            .map_err(anyhow::Error::msg)?;
    if let Some(session_id) = request.session_id.as_deref().filter(|id| !id.is_empty()) {
        let project = agentz_task_state::canonical_root(Path::new(&approval_project_dir))
            .map_err(anyhow::Error::msg)?;
        let free =
            agentz_task_state::canonical_root(&super::projects::free_sessions_path(&config_dir))
                .ok();
        if free.as_deref() == Some(project.as_str()) {
            let app_data = app.path().app_data_dir().map_err(|e| anyhow!(e))?;
            if let Some(directory) = super::project_scope::projectless_session_directory(
                &app_data,
                Path::new(&project),
                session_id,
            )
            .map_err(anyhow::Error::msg)?
            {
                workspace_root =
                    agentz_task_state::canonical_root(&directory).map_err(anyhow::Error::msg)?;
            }
        }
    }
    crate::tools::remote_fs::install_if_remote(&mut registry, &workspace_root);

    let display_for_db = display_prompt
        .filter(|s| !s.trim().is_empty())
        .or(slash_display)
        .unwrap_or_else(|| request.prompt.clone());

    let with_browser = expand_browser_element_refs(&effective_prompt, &browser).await;
    let snippets = {
        let snippets_map = app
            .state::<crate::state::AppState>()
            .terminal_snippets
            .clone();
        let guard = snippets_map.lock().await;
        guard.clone()
    };
    let with_terminal = expand_terminal_snippets(&with_browser, &snippets);
    let mut llm_user_content = if crate::remote::is_remote(&workspace_root) {
        expand_file_refs_remote(&with_terminal, &workspace_root).await
    } else {
        expand_file_refs(&with_terminal, &workspace_root)
    };

    let expected_source = request
        .channel
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_channel_for(SOURCE_CODEZ).to_string());

    let session_id = match request.session_id.clone().filter(|s| !s.is_empty()) {
        Some(id) => {
            let exists = {
                let guard = db.lock().await;
                guard
                    .get_session(&id)
                    .map_err(|e| anyhow!("get_session failed: {e}"))?
                    .is_some()
            };
            if exists {
                let guard = db.lock().await;
                validate_session_continuation(
                    &guard,
                    &id,
                    &expected_source,
                    workz_team_id.as_deref(),
                )
                .map_err(|e| anyhow!(e))?;
            }
            let title = request
                .session_title
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("");
            let db = db.lock().await;
            db.ensure_fixed_session(&id, title, &expected_source)
                .context("failed to ensure requested session")?
                .id
        }
        None => {
            let title = request.session_title.as_deref();
            let db = db.lock().await;
            db.create_session_with_source(title, &expected_source)
                .context("failed to create session")?
                .id
        }
    };

    if workz_team_id.is_some() || workz_pool_id.is_some() {
        let guard = db.lock().await;
        persist_workz_meta(
            &guard,
            &session_id,
            workz_team_id.as_deref(),
            workz_pool_id.as_deref(),
        )
        .map_err(|e| anyhow!(e))?;
    }

    let (run_id, attempt_id) = queued_run
        .as_ref()
        .map(|run| {
            let (run, attempt) = run.identity();
            (run.to_owned(), attempt.to_owned())
        })
        .unwrap_or_else(|| {
            (
                uuid::Uuid::new_v4().to_string(),
                uuid::Uuid::new_v4().to_string(),
            )
        });
    tracing::Span::current().record("run_id", run_id.as_str());
    if let Some((task_id, environment_id)) = queued_run.as_ref().and_then(|run| run.task_binding())
    {
        tracing::Span::current().record("task_id", task_id);
        tracing::Span::current().record("environment_id", environment_id);
    }
    let approval_project_dir = agentz_task_state::canonical_root(Path::new(&approval_project_dir))
        .map_err(anyhow::Error::msg)?;
    let is_free_scope =
        agentz_task_state::canonical_root(&super::projects::free_sessions_path(&config_dir))
            .ok()
            .as_deref()
            == Some(approval_project_dir.as_str());
    let (workspace_root, projectless_app_data) = if is_free_scope {
        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|e| anyhow!("projectless app data unavailable: {e}"))?;
        match super::project_scope::projectless_session_directory(
            &app_data,
            Path::new(&approval_project_dir),
            &session_id,
        )
        .map_err(anyhow::Error::msg)?
        {
            Some(directory) => (
                agentz_task_state::canonical_root(&directory).map_err(anyhow::Error::msg)?,
                Some(app_data),
            ),
            None => (workspace_root, None),
        }
    } else {
        (workspace_root, None)
    };
    let has_registered_env =
        agentz_task_state::load(Path::new(&approval_project_dir), &session_id)
            .map_err(anyhow::Error::msg)?
            .is_some();
    let require_binding = if is_free_scope {
        projectless_app_data.is_some() || has_registered_env
    } else {
        workspace_root != approval_project_dir || has_registered_env
    };
    let mut early_binding = crate::runtime::approval::resolve_binding_for_mode(
        Path::new(&approval_project_dir),
        Path::new(&workspace_root),
        &session_id,
        &run_id,
        require_binding,
        projectless_app_data.as_deref(),
    )
    .await
    .map_err(anyhow::Error::msg)?;
    early_binding.attempt_id = attempt_id.clone();
    early_binding.validate().map_err(anyhow::Error::msg)?;
    if projectless_app_data.is_some() {
        let guard = db.lock().await;
        let saved = guard
            .get_session(&session_id)
            .map_err(|e| anyhow!("projectless session metadata unavailable: {e}"))?
            .ok_or_else(|| anyhow!("projectless session disappeared"))?
            .workspace_root;
        if let Some(saved) = saved {
            if agentz_task_state::canonical_root(Path::new(&saved)).map_err(anyhow::Error::msg)?
                != workspace_root
            {
                return Err(anyhow!(
                    "projectless session workspace changed after binding"
                ));
            }
        } else {
            guard
                .set_session_workspace(&session_id, Some(&workspace_root))
                .map_err(|e| anyhow!("persist projectless session workspace failed: {e}"))?;
        }
    }
    let durable_run = if let Some(mut run) = queued_run {
        run.start_bound(&early_binding)
            .map_err(anyhow::Error::msg)?;
        Some(run)
    } else {
        Some(
            crate::runtime::durable_run::ManagedRun::begin_bound(
                Path::new(&approval_project_dir),
                &early_binding,
            )
            .map_err(anyhow::Error::msg)?,
        )
    };
    let mut plan_attempt_guard = None;
    let durable_plan_enabled = registry.get("plan_todo").is_some()
        && confirmation_responses.is_some()
        && !crate::remote::is_remote(&workspace_root);
    if durable_plan_enabled {
        let (guard, capability) =
            crate::tools::durable_plan_todo::ActiveAttemptGuard::acquire(early_binding.clone())
                .map_err(anyhow::Error::msg)?;
        let tool = crate::tools::durable_plan_todo::DurablePlanTodoTool::new(
            PathBuf::from(&approval_project_dir),
            PathBuf::from(&workspace_root),
            early_binding.clone(),
            require_binding,
            display_for_db.clone(),
            agentz_task_state::plan::PlanStore::open(Path::new(&approval_project_dir))
                .map_err(anyhow::Error::msg)?,
            cancel.clone(),
            plan_store.clone(),
            event_sink.clone(),
            capability,
        )
        .map_err(anyhow::Error::msg)?
        .with_projectless_app_data(projectless_app_data.clone());
        tool.publish_recovered_projection()
            .await
            .map_err(anyhow::Error::msg)?;
        registry.unregister("plan_todo");
        registry.register(Box::new(tool));
        plan_attempt_guard = Some(guard);
    } else {
        registry.unregister("plan_todo");
        tracing::debug!(session_id = %session_id, "durable plan tool omitted: scope is restricted, noninteractive or remote");
    }
    let plan_store = if durable_plan_enabled {
        plan_store
    } else {
        piscis_kernel::agent::plan::new_plan_store()
    };

    let status_guard = {
        let db_guard = db.lock().await;
        db_guard
            .append_message(&session_id, "user", &display_for_db)
            .context("failed to append user message")?;
        let _ = db_guard.update_session_status(&session_id, "running");
        let _ = maybe_autotitle_session_from_first_prompt(&db_guard, &session_id, &display_for_db);
        drop(db_guard);
        SessionRunningGuard::arm(db.clone(), session_id.clone())
    };

    if let (Some(_team_id), Some(pool_id)) = (
        workz_team_id.as_deref().filter(|s| !s.is_empty()),
        workz_pool_id.as_deref().filter(|s| !s.is_empty()),
    ) {
        let prior_user_turns = {
            let guard = db.lock().await;
            guard
                .get_messages_latest(&session_id, 2000)
                .unwrap_or_default()
                .into_iter()
                .filter(|m| m.role == "user")
                .count()
        };
        if prior_user_turns > 1 {
            let (open, blocked) = {
                let guard = db.lock().await;
                let todos = guard.list_koi_todos(None).unwrap_or_default();
                let mut open = 0u32;
                let mut blocked = 0u32;
                for t in todos
                    .iter()
                    .filter(|t| t.pool_session_id.as_deref() == Some(pool_id))
                {
                    match t.status.as_str() {
                        "blocked" => blocked += 1,
                        "todo" | "in_progress" | "needs_review" => open += 1,
                        _ => {}
                    }
                }
                (open, blocked)
            };
            let reminder = swarm_coordinator_followup_reminder(pool_id, open, blocked);
            llm_user_content = format!("{reminder}\n\n{llm_user_content}");
        }
    }

    let (
        context_window,
        read_timeout,
        policy_mode,
        tool_rate_limit_per_minute,
        allow_outside_workspace,
        vision_enabled_setting,
        enable_streaming,
        confirm_shell_commands,
        confirm_file_writes,
        always_allow_tools,
        auto_compact_threshold,
        fallback_models,
        compaction,
        tool_settings,
        max_iterations,
    ) = {
        let s = settings.lock().await;
        (
            s.context_window,
            s.llm_read_timeout_secs.max(30),
            s.policy_mode.clone(),
            s.tool_rate_limit_per_minute,
            s.allow_outside_workspace,
            s.vision_enabled,
            runtime.stream.unwrap_or(s.enable_streaming),
            s.confirm_shell_commands,
            s.confirm_file_writes,
            s.always_allow_tools.clone(),
            s.auto_compact_input_tokens_threshold,
            s.fallback_models.clone(),
            CompactionSettings::from_settings(&s),
            Arc::new(piscis_kernel::agent::tool::ToolSettings::from_settings(&s)),
            s.max_iterations,
        )
    };

    let history_budget =
        piscis_kernel::llm::compute_context_budget(context_window, runtime.max_tokens);
    let mut llm_messages: Vec<LlmMessage> = {
        let db = db.lock().await;
        let history = db
            .get_messages_latest(&session_id, 2000)
            .unwrap_or_default();
        let rolling_summary = db
            .get_session_context_state(&session_id)
            .ok()
            .flatten()
            .map(|s| s.rolling_summary)
            .filter(|s| !s.trim().is_empty());
        let state_frame = db
            .get_session_state_frame_json(&session_id)
            .ok()
            .flatten()
            .and_then(|raw| piscis_kernel::agent::state_frame::StateFrame::from_json_opt(&raw));
        crate::context_assembly::build_context_messages(
            &history,
            history_budget,
            rolling_summary.as_deref(),
            state_frame.as_ref(),
        )
    };
    match llm_messages.last_mut() {
        Some(last) if last.role == "user" => {
            last.content = MessageContent::text(&llm_user_content);
        }
        _ => {
            llm_messages.push(LlmMessage {
                role: "user".into(),
                content: MessageContent::text(&llm_user_content),
            });
        }
    }

    if let Some((media_type, data_b64)) = image_attachment.as_ref() {
        inject_image_block(&mut llm_messages, media_type, data_b64);
    }

    let client = llm::build_client_with_options(
        &runtime.provider,
        &runtime.api_key,
        if runtime.base_url.is_empty() {
            None
        } else {
            Some(&runtime.base_url)
        },
        read_timeout,
        runtime.options,
    );

    let system_prompt = build_turn_system_prompt(
        &app,
        &config_dir,
        &workspace_root,
        &session_id,
        chat_mode,
        prefer_zh,
        allow_outside_workspace,
        request.extra_system_context.as_deref(),
        agent.as_ref(),
        &enabled_skills,
        enabled_connectors.as_ref(),
        durable_plan_enabled,
        &plan_store,
        workz_team_id.as_deref(),
        workz_pool_id.as_deref(),
    )
    .await;

    let policy = Arc::new(PolicyGate::with_profile_and_flags(
        &workspace_root,
        &policy_mode,
        tool_rate_limit_per_minute,
        allow_outside_workspace,
    ));

    let vision_override = vision_capable || vision_enabled_setting;
    // Vision models get a compressed screenshot after each visual browser action.
    app.state::<crate::state::AppState>()
        .browser
        .set_auto_screenshot(vision_override);
    let registry = Arc::new(registry);
    let hooks: Arc<dyn piscis_kernel::agent::hooks::AgentHooks> =
        Arc::new(crate::runtime::journal_hooks::JournalWithIdeNotify::new(
            journal.clone(),
            app.clone(),
            Some(db.clone()),
        ));
    let approval_broker = app
        .state::<crate::state::AppState>()
        .approval_broker
        .clone();
    let mut synced_always_allow = always_allow_tools.clone();
    if !confirm_shell_commands
        && !synced_always_allow
            .iter()
            .any(|t| t.eq_ignore_ascii_case("shell"))
    {
        synced_always_allow.push("shell".into());
    }
    approval_broker.set_always_allowed(synced_always_allow);
    let approval_hooks = Arc::new(
        crate::runtime::approval::ApprovalHooks::new_with_identity_scope(
            PathBuf::from(&approval_project_dir),
            PathBuf::from(&workspace_root),
            &session_id,
            approval_broker,
            event_sink.clone(),
            hooks,
            registry.clone(),
            cancel.clone(),
            confirmation_responses.is_some(),
            (&run_id, &attempt_id),
            projectless_app_data,
            Some(require_binding),
        )
        .await
        .map_err(|e| anyhow!(e))?
        .with_auto_approval(crate::runtime::approval::AutoApprovalPolicy {
            policy_mode: policy_mode.clone(),
            confirm_shell_commands,
            confirm_file_writes,
            always_allow_tools: always_allow_tools.clone(),
        }),
    );
    if approval_hooks.binding() != &early_binding {
        return Err(anyhow!(
            "task environment binding changed after plan recovery; restart turn"
        ));
    }
    let hooks: Arc<dyn piscis_kernel::agent::hooks::AgentHooks> = approval_hooks;
    let _ = delegation_bridge.set(crate::runtime::subagent_bridge::ParentBridge {
        read_only: chat_mode == "plan",
    });
    let confirm_flags = piscis_kernel::agent::loop_::confirm_flags_handle(
        confirm_shell_commands,
        confirm_file_writes,
    );
    let harness = HarnessConfig::for_main_chat(
        runtime.model.clone(),
        fallback_models,
        registry,
        policy,
        system_prompt,
        runtime.max_tokens,
        context_window,
        confirm_flags,
        Some(vision_override),
        None,
        String::new(),
        auto_compact_threshold,
        compaction,
        db.clone(),
        plan_store.clone(),
    )
    .with_hooks(hooks)
    .with_streaming(enable_streaming);

    let budget_limits =
        crate::runtime::task_budget::load_limits_for_app(&app).map_err(|e| anyhow!(e))?;
    budget_scope
        .bind(
            Path::new(&approval_project_dir),
            &early_binding,
            &budget_limits,
        )
        .map_err(|e| anyhow!(e))?;
    let client = budget_scope.guard(client);
    let agent = harness.into_agent_loop(client, None, None);

    journal.begin_turn(&session_id);

    let workspace_buf = PathBuf::from(&workspace_root);
    let ctx = ToolContext {
        session_id: session_id.clone(),
        workspace_root: workspace_buf,
        bypass_permissions: false,
        settings: tool_settings,
        max_iterations: Some(max_iterations),
        memory_owner_id: "piscis".to_string(),
        pool_session_id: workz_pool_id.clone().filter(|s| !s.trim().is_empty()),
        tool_use_id: None,
        cancel: cancel.clone(),
        loop_halt: Some(loop_halt),
    };

    let cancellation_completion =
        crate::runtime::cancel_completion::CancellationCompletion::capture(&early_binding, &cancel)
            .map_err(anyhow::Error::msg)?;
    let durable_stream = if !crate::remote::is_remote(&workspace_root)
        && workz_team_id.is_none()
        && workz_pool_id.is_none()
    {
        Some(
            crate::runtime::durable_stream::DurableStreamWriter::open(
                Path::new(&approval_project_dir),
                &early_binding,
            )
            .map_err(anyhow::Error::msg)?,
        )
    } else {
        None
    };
    let (tx, rx) = mpsc::channel::<AgentEvent>(1024);
    event_sink.emit_session(
        &session_id,
        "agent_lifecycle",
        serde_json::json!({"state": "executing", "max_iterations": max_iterations}),
    );
    let collector = spawn_turn_event_collector(
        rx,
        durable_stream,
        cancel.clone(),
        event_sink.clone(),
        session_id.clone(),
        app.clone(),
        workspace_root.clone(),
    );

    let timeout = match request.task_timeout_secs {
        Some(s) if s > 0 => Duration::from_secs(u64::from(s)),
        _ => Duration::from_secs(60 * 60 * 24 * 365),
    };
    let run_fut = agent.run(llm_messages, tx, cancel.clone(), ctx);
    let mut turn_timeout_secs: Option<u64> = None;
    let run_res = tokio::time::timeout(timeout, run_fut).await;

    let (ok, new_messages, error_msg): (bool, Vec<LlmMessage>, Option<String>) = match run_res {
        Ok(Ok((msgs, _total_in, _total_out))) => (true, msgs, None),
        Ok(Err(e)) => (false, Vec::new(), Some(format!("agent error: {e}"))),
        Err(_) => {
            cancel.store(true, Ordering::SeqCst);
            turn_timeout_secs = Some(timeout.as_secs());
            (
                false,
                Vec::new(),
                Some(format!("timed out after {}s", timeout.as_secs())),
            )
        }
    };

    let collector_out = match collector.await {
        Ok(output) => output,
        Err(error) => {
            cancel.store(true, Ordering::SeqCst);
            EventCollectorOutput {
                streamed_text: String::new(),
                stream_error: Some(format!("Execution event collector failed: {error}")),
                tool_calls: 0,
                tool_errors: 0,
                iterations: 0,
                unsaved_tail: String::new(),
                durability_failed: true,
            }
        }
    };

    finalize_turn_and_spawn_background(
        app,
        db,
        settings,
        settings_snapshot,
        event_sink,
        plan_store,
        session_id,
        run_id,
        attempt_id,
        workspace_root,
        runtime,
        budget_scope,
        max_iterations,
        cancel,
        early_binding,
        cancellation_completion,
        durable_run,
        plan_attempt_guard,
        status_guard,
        ok,
        new_messages,
        error_msg,
        turn_timeout_secs,
        collector_out,
    )
    .await
}
