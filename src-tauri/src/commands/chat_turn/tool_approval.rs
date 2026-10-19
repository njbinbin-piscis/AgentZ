use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use tauri::Manager;
use tokio::sync::{mpsc, Mutex};

use crate::commands::agents::{safe_id, sync_agents_to_kois, AgentManifest};
use crate::commands::data_scope::resolve_global_config_dir;
use crate::commands::system_prompt::subagent_system_prompt;
use crate::commands::teams::TeamManifest;
use crate::lsp::manager::LspManager;
use piscis_core::host::EventSink;
use piscis_kernel::agent::harness::config::{CompactionSettings, HarnessConfig};
use piscis_kernel::agent::messages::AgentEvent;
use piscis_kernel::agent::plan::PlanStore;
use piscis_kernel::agent::tool::{
    new_tool_registry_handle, ToolContext, ToolRegistry, ToolRegistryHandleExt,
};
use piscis_kernel::llm::{self, LlmMessage, MessageContent};
use piscis_kernel::policy::gate::PolicyGate;
use piscis_kernel::store::settings::{LlmProviderConfig, Settings};
use piscis_kernel::tools::NeutralToolsConfig;

use super::prompt_assembly::dependency_source_roots;

#[derive(Debug, Clone)]
pub(super) struct LlmRuntime {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub options: llm::ClientOptions,
    pub stream: Option<bool>,
}

#[derive(Debug, Clone)]
pub(super) struct SettingsSnapshot {
    provider: String,
    model: String,
    custom_base_url: String,
    max_tokens: u32,
    temperature: Option<f32>,
    top_p: Option<f32>,
    thinking: Option<bool>,
    anthropic_api_key: String,
    openai_api_key: String,
    deepseek_api_key: String,
    qwen_api_key: String,
    minimax_api_key: String,
    zhipu_api_key: String,
    kimi_api_key: String,
}

pub(super) fn snapshot_settings(settings: &Settings) -> SettingsSnapshot {
    SettingsSnapshot {
        provider: settings.provider.clone(),
        model: settings.model.clone(),
        custom_base_url: settings.custom_base_url.clone(),
        max_tokens: settings.max_tokens,
        temperature: settings.temperature,
        top_p: settings.top_p,
        thinking: settings.thinking,
        anthropic_api_key: settings.anthropic_api_key.clone(),
        openai_api_key: settings.openai_api_key.clone(),
        deepseek_api_key: settings.deepseek_api_key.clone(),
        qwen_api_key: settings.qwen_api_key.clone(),
        minimax_api_key: settings.minimax_api_key.clone(),
        zhipu_api_key: settings.zhipu_api_key.clone(),
        kimi_api_key: settings.kimi_api_key.clone(),
    }
}

pub(super) fn restore_settings(settings: &mut Settings, snap: SettingsSnapshot) {
    settings.provider = snap.provider;
    settings.model = snap.model;
    settings.custom_base_url = snap.custom_base_url;
    settings.max_tokens = snap.max_tokens;
    settings.temperature = snap.temperature;
    settings.top_p = snap.top_p;
    settings.thinking = snap.thinking;
    settings.anthropic_api_key = snap.anthropic_api_key;
    settings.openai_api_key = snap.openai_api_key;
    settings.deepseek_api_key = snap.deepseek_api_key;
    settings.qwen_api_key = snap.qwen_api_key;
    settings.minimax_api_key = snap.minimax_api_key;
    settings.zhipu_api_key = snap.zhipu_api_key;
    settings.kimi_api_key = snap.kimi_api_key;
}

fn set_provider_api_key(settings: &mut Settings, provider: &str, key: &str) {
    match provider {
        "anthropic" => settings.anthropic_api_key = key.to_string(),
        "openai" | "custom" => settings.openai_api_key = key.to_string(),
        "deepseek" => settings.deepseek_api_key = key.to_string(),
        "qwen" | "tongyi" => settings.qwen_api_key = key.to_string(),
        "minimax" => settings.minimax_api_key = key.to_string(),
        "zhipu" => settings.zhipu_api_key = key.to_string(),
        "kimi" | "moonshot" => settings.kimi_api_key = key.to_string(),
        _ => settings.openai_api_key = key.to_string(),
    }
}

pub(crate) fn apply_llm_provider(settings: &mut Settings, provider: &LlmProviderConfig) {
    let key = {
        let effective = provider.effective_api_key();
        if effective.trim().is_empty() {
            settings.active_api_key().to_string()
        } else {
            effective.to_string()
        }
    };
    settings.provider = provider.provider.clone();
    settings.model = provider.model.clone();
    settings.custom_base_url = provider.base_url.clone();
    if provider.max_tokens > 0 {
        settings.max_tokens = provider.max_tokens;
    }
    settings.temperature = provider.temperature;
    settings.top_p = provider.top_p;
    settings.thinking = provider.thinking;
    set_provider_api_key(settings, &provider.provider, &key);
}

const FAST_MODEL_HINTS: &[&str] = &[
    "haiku", "mini", "flash", "lite", "turbo", "instant", "air", "nano", "small",
];
const SMART_MODEL_HINTS: &[&str] = &[
    "opus", "sonnet", "pro", "o1", "o3", "o4", "r1", "reasoner", "max", "plus", "4o", "4.1", "4.5",
];

pub(super) fn auto_route_model(settings: &Settings, chat_mode: &str) -> Option<String> {
    let flag = std::env::var("CODEZ_AUTO_MODEL_ROUTING").unwrap_or_default();
    if flag != "1" && !flag.eq_ignore_ascii_case("true") {
        return None;
    }
    if settings.llm_providers.len() < 2 {
        return None;
    }
    let hints = if chat_mode == "plan" {
        FAST_MODEL_HINTS
    } else {
        SMART_MODEL_HINTS
    };
    settings
        .llm_providers
        .iter()
        .find(|p| {
            let m = p.model.to_lowercase();
            hints.iter().any(|h| m.contains(h))
        })
        .map(|p| p.id.clone())
}

pub(super) fn resolve_llm_runtime(
    settings: &Settings,
    model_id: Option<&str>,
) -> Result<LlmRuntime> {
    if let Some(id) = model_id.filter(|s| !s.trim().is_empty()) {
        let provider = settings
            .find_llm_provider(id)
            .ok_or_else(|| anyhow!("unknown model id '{id}'"))?;
        let api_key = {
            let key = provider.effective_api_key();
            if key.trim().is_empty() {
                settings.active_api_key().to_string()
            } else {
                key.to_string()
            }
        };
        if api_key.is_empty() {
            return Err(anyhow!("no API key configured for model '{id}'"));
        }
        return Ok(LlmRuntime {
            provider: provider.provider.clone(),
            model: provider.model.clone(),
            api_key,
            base_url: provider.base_url.clone(),
            max_tokens: if provider.max_tokens > 0 {
                provider.max_tokens
            } else {
                settings.max_tokens.max(1024)
            },
            options: llm::ClientOptions {
                temperature: provider.temperature,
                top_p: provider.top_p,
                thinking: provider.thinking,
            },
            stream: provider.stream,
        });
    }

    if settings.model.trim().is_empty() {
        if let Some(first) = settings.llm_providers.first() {
            let api_key = {
                let key = first.effective_api_key();
                if key.trim().is_empty() {
                    settings.active_api_key().to_string()
                } else {
                    key.to_string()
                }
            };
            if api_key.is_empty() {
                return Err(anyhow!(
                    "no API key configured for provider '{}'",
                    first.provider
                ));
            }
            return Ok(LlmRuntime {
                provider: first.provider.clone(),
                model: first.model.clone(),
                api_key,
                base_url: first.base_url.clone(),
                max_tokens: if first.max_tokens > 0 {
                    first.max_tokens
                } else {
                    settings.max_tokens.max(1024)
                },
                options: llm::ClientOptions {
                    temperature: first.temperature,
                    top_p: first.top_p,
                    thinking: first.thinking,
                },
                stream: first.stream,
            });
        }
    }

    let api_key = settings.active_api_key().to_string();
    if api_key.is_empty() {
        return Err(anyhow!(
            "no API key configured for provider '{}'",
            settings.provider
        ));
    }
    Ok(LlmRuntime {
        provider: settings.provider.clone(),
        model: settings.model.clone(),
        api_key,
        base_url: settings.custom_base_url.clone(),
        max_tokens: settings.max_tokens.max(1024),
        options: llm::ClientOptions {
            temperature: settings.temperature,
            top_p: settings.top_p,
            thinking: settings.thinking,
        },
        stream: None,
    })
}

fn apply_model_id_to_settings(settings: &mut Settings, model_id: Option<&str>) {
    if let Some(id) = model_id.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(prov) = settings.find_llm_provider(id).cloned() {
            apply_llm_provider(settings, &prov);
        } else {
            settings.model = id.to_string();
        }
    }
}

fn resolve_koi_model_id(
    settings: &Settings,
    app: &tauri::AppHandle,
    koi_llm_provider_id: Option<&str>,
) -> Option<String> {
    if let Some(pid) = koi_llm_provider_id.filter(|s| !s.trim().is_empty()) {
        if settings.find_llm_provider(pid).is_some() {
            return Some(pid.to_string());
        }
    }
    let flash_id = resolve_global_config_dir(app)
        .ok()
        .and_then(|dir| crate::commands::flash::load_flash_provider_id(&dir));
    flash_id.filter(|id| settings.find_llm_provider(id).is_some())
}

pub(crate) fn materialize_headless_llm_settings(
    settings: &mut Settings,
    app: &tauri::AppHandle,
    koi_llm_provider_id: Option<&str>,
) {
    let model_id = resolve_koi_model_id(settings, app, koi_llm_provider_id);
    apply_model_id_to_settings(settings, model_id.as_deref());
    if settings.model.trim().is_empty() {
        if let Some(first) = settings.llm_providers.first().cloned() {
            apply_llm_provider(settings, &first);
        }
    }
}

pub(crate) fn resolve_koi_llm_provider_for_turn(
    db: &piscis_kernel::store::db::Database,
    config_dir: &Path,
    koi_id: &str,
) -> Option<String> {
    let _ = sync_agents_to_kois(db, config_dir);
    let koi = db.get_koi(koi_id).ok().flatten()?;
    if let Some(pid) = koi.llm_provider_id.filter(|s| !s.trim().is_empty()) {
        return Some(pid);
    }
    let handle = safe_id(&koi.name);
    if handle.is_empty() {
        return None;
    }
    AgentManifest::load(&config_dir.join("agents").join(&handle).join("agent.json"))
        .ok()
        .and_then(|m| m.llm_provider_id)
        .filter(|s| !s.trim().is_empty())
}

pub(super) fn resolve_team_coordinator_model_id(
    app: &tauri::AppHandle,
    db: &piscis_kernel::store::db::Database,
    settings: &Settings,
    team_id: &str,
    config_dir: &Path,
) -> Option<String> {
    if let Some(flash) = resolve_global_config_dir(app)
        .ok()
        .and_then(|dir| crate::commands::flash::load_flash_provider_id(&dir))
    {
        if settings.find_llm_provider(&flash).is_some() {
            return Some(flash);
        }
    }

    let _ = sync_agents_to_kois(db, config_dir);

    if let Ok(team) = TeamManifest::load_by_id(app, team_id) {
        for member in &team.members {
            let slug = safe_id(member);
            if slug.is_empty() {
                continue;
            }
            if let Ok(Some(koi)) = db.find_koi_by_name(&slug) {
                if let Some(pid) = koi.llm_provider_id.filter(|s| !s.trim().is_empty()) {
                    if settings.find_llm_provider(&pid).is_some() {
                        return Some(pid);
                    }
                }
            }
            if let Ok(manifest) =
                AgentManifest::load(&config_dir.join("agents").join(&slug).join("agent.json"))
            {
                if let Some(pid) = manifest.llm_provider_id.filter(|s| !s.trim().is_empty()) {
                    if settings.find_llm_provider(&pid).is_some() {
                        return Some(pid);
                    }
                }
            }
        }
    }

    settings.llm_providers.first().map(|p| p.id.clone())
}

const PLAN_MODE_DISABLED: &[&str] = &[
    "file_write",
    "file_edit",
    "shell",
    "code_run",
    "process_control",
    "elevate",
    "email",
    "ssh",
    "web_search",
    "memory_store",
    "plan_todo",
];

fn turn_devenv_tool() -> crate::tools::devenv::DevEnvTool {
    crate::tools::devenv::DevEnvTool::environment_only()
}

#[cfg(test)]
mod durable_devenv_wiring_tests {
    #[test]
    fn turn_constructor_has_no_legacy_plan_mutation_capability() {
        assert!(!super::turn_devenv_tool().updates_legacy_plan());
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_tool_registry(
    app: tauri::AppHandle,
    db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    global_db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    config_dir: PathBuf,
    skill_loader: Arc<Mutex<crate::skills::loader::SkillLoader>>,
    settings: Arc<Mutex<Settings>>,
    event_sink: Arc<dyn EventSink>,
    plan_store: PlanStore,
    chat_mode: &str,
    lsp_manager: Arc<LspManager>,
    user_tools_dir: Option<PathBuf>,
    extra_enabled_tools: &[String],
    enable_pool: bool,
    enable_skill_manage: bool,
    loop_halt: Arc<std::sync::atomic::AtomicBool>,
    api_connector_allowlist: Option<Vec<String>>,
    slash_tool_allowlist: Option<Vec<String>>,
    budget_scope: crate::runtime::task_budget::BudgetScope,
) -> ToolRegistry {
    let mut builtin_tool_enabled = None;
    if chat_mode == "plan" {
        let mut map = HashMap::new();
        for name in PLAN_MODE_DISABLED {
            map.insert((*name).to_string(), false);
        }
        builtin_tool_enabled = Some(map);
    }
    if let Some(allow) = slash_tool_allowlist.filter(|a| !a.is_empty()) {
        if let Some(map) = crate::commands::slash_commands::tool_allowlist_map(&allow) {
            builtin_tool_enabled = Some(map);
        }
    }
    if !extra_enabled_tools.is_empty() {
        let map = builtin_tool_enabled.get_or_insert_with(HashMap::new);
        const PLAN_MODE_WRITE_TOOLS: &[&str] = &[
            "file_write",
            "file_edit",
            "shell",
            "code_run",
            "process_control",
            "elevate",
            "email",
            "ssh",
            "memory_store",
            "plan_todo",
        ];
        for name in extra_enabled_tools {
            if chat_mode == "plan" && PLAN_MODE_WRITE_TOOLS.contains(&name.as_str()) {
                continue;
            }
            map.insert(name.clone(), true);
        }
    }

    let (subagent_runtime, pool_event_sink) = if enable_pool {
        let (rt, sink) = crate::runtime::koi::pool_wiring(&app);
        (Some(rt), Some(sink))
    } else {
        (None, None)
    };

    let mut handle = new_tool_registry_handle();
    let cfg = NeutralToolsConfig {
        db: Some(db),
        settings: Some(settings),
        builtin_tool_enabled,
        user_tools_dir,
        event_sink: Some(event_sink),
        plan_store: Some(plan_store),
        pool_event_sink,
        subagent_runtime,
        coordinator_config: Default::default(),
    };
    let db_for_delegate = cfg.db.clone();
    let settings_for_delegate = cfg.settings.clone();
    let plan_for_delegate = cfg.plan_store.clone();
    piscis_kernel::tools::register_neutral_tools(&mut handle, &cfg);

    if let Some(registry) = handle.as_registry_mut() {
        registry.register(Box::new(crate::tools::LspTool {
            lsp_manager: lsp_manager.clone(),
        }));
        registry.register(Box::new(crate::tools::ReadLintsTool {
            lsp_manager: lsp_manager.clone(),
        }));
        registry.register(Box::new(crate::tools::codebase_search::CodebaseSearchTool));
        registry.register(Box::new(crate::tools::graph_search::GraphSearchTool));
        registry.register(Box::new(crate::tools::graph_explore::GraphExploreTool));
        registry.register(Box::new(crate::tools::repo_memory::RepoMemoryTool));
        registry.register(Box::new(crate::tools::symbols::SymbolSearchTool));
        registry.register(Box::new(crate::tools::symbols::ImpactTool));
        if let Ok(config_dir) = crate::commands::data_scope::resolve_global_config_dir(&app) {
            registry.register(Box::new(crate::tools::api_connector::ApiConnectorTool {
                config_dir,
                allowed_ids: api_connector_allowlist,
            }));
        }
        registry.register(Box::new(crate::tools::chat_ui::ChatUiTool {
            app: app.clone(),
        }));
        registry.register(Box::new(crate::tools::chat_ui_patch::ChatUiPatchTool {
            app: app.clone(),
        }));
        registry.register(Box::new(crate::tools::chat_ui_listen::ChatUiListenTool {
            app: app.clone(),
        }));
        registry.register(Box::new(crate::tools::plan_mode_ui::PlanModeUiTool {
            app: app.clone(),
            loop_halt,
        }));
        if chat_mode == "plan" {
            if let (Some(db), Some(settings), Some(plan)) = (
                db_for_delegate.clone(),
                settings_for_delegate.clone(),
                plan_for_delegate.clone(),
            ) {
                registry.register(Box::new(crate::tools::delegate::DelegateTool {
                    budget: budget_scope.clone(),
                    db,
                    settings,
                    plan_store: plan,
                    lsp_manager: lsp_manager.clone(),
                    app: app.clone(),
                }));
            }
        }
        if chat_mode != "plan" {
            if let (Some(db), Some(settings), Some(plan)) =
                (db_for_delegate, settings_for_delegate, plan_for_delegate)
            {
                registry.register(Box::new(crate::tools::delegate::DelegateTool {
                    budget: budget_scope.clone(),
                    db: db.clone(),
                    settings: settings.clone(),
                    plan_store: plan.clone(),
                    lsp_manager: lsp_manager.clone(),
                    app: app.clone(),
                }));
                registry.register(Box::new(crate::tools::call_fish::CallFishTool {
                    budget: budget_scope.clone(),
                    db,
                    settings,
                    plan_store: plan,
                    lsp_manager,
                    app: app.clone(),
                }));
            }
            {
                let guarded_browser = app.state::<crate::state::AppState>().browser.guarded_tool();
                registry.register(Box::new(guarded_browser));
                registry.register(Box::new(crate::tools::terminal_read::TerminalReadTool {
                    terminals: app.state::<crate::state::AppState>().terminals.clone(),
                }));
            }
            registry.register(Box::new(turn_devenv_tool()));
            registry.register(Box::new(crate::tools::app_control::AppControlTool { app }));
            if enable_skill_manage {
                registry.register(Box::new(crate::tools::skill_manage::SkillManageTool {
                    db: global_db,
                    config_dir: config_dir.clone(),
                    loader: skill_loader,
                }));
            }
        }
    }

    handle
        .into_registry()
        .map_err(|_| "internal: tool registry handle type mismatch".to_string())
        .expect("tool registry")
}

pub(super) async fn register_mcp_and_connector_tools(
    registry: &mut ToolRegistry,
    settings: &Arc<Mutex<Settings>>,
    config_dir: &Path,
    skill_mcp_names: &HashSet<String>,
    enabled_connectors: &Option<Vec<String>>,
    agent: Option<&AgentManifest>,
) {
    let mcp_servers = { settings.lock().await.mcp_servers.clone() };
    if !mcp_servers.is_empty() {
        piscis_kernel::tools::register_mcp_tools(registry, &mcp_servers).await;
    }
    if !skill_mcp_names.is_empty() {
        let bound: Vec<piscis_kernel::store::settings::McpServerConfig> = mcp_servers
            .iter()
            .filter(|m| !m.enabled && skill_mcp_names.contains(&m.name))
            .map(|m| {
                let mut c = m.clone();
                c.enabled = true;
                c
            })
            .collect();
        if !bound.is_empty() {
            piscis_kernel::tools::register_mcp_tools(registry, &bound).await;
        }
    }
    match enabled_connectors {
        Some(ids) => {
            let selected =
                crate::commands::connectors::resolve_named_connector_mcp_configs(config_dir, ids);
            if !selected.is_empty() {
                crate::commands::connectors::register_connector_mcp_tools(
                    registry,
                    config_dir,
                    &selected,
                )
                .await;
            }
        }
        None => {
            let connector_configs =
                crate::commands::connectors::resolve_connector_mcp_configs(config_dir);
            if !connector_configs.is_empty() {
                crate::commands::connectors::register_connector_mcp_tools(
                    registry,
                    config_dir,
                    &connector_configs,
                )
                .await;
            }
            if let Some(agent) = agent {
                if !agent.connectors.is_empty() {
                    let already: HashSet<String> =
                        connector_configs.iter().map(|c| c.name.clone()).collect();
                    let agent_connectors: Vec<piscis_kernel::store::settings::McpServerConfig> =
                        crate::commands::connectors::resolve_named_connector_mcp_configs(
                            config_dir,
                            &agent.connectors,
                        )
                        .into_iter()
                        .filter(|c| !already.contains(&c.name))
                        .collect();
                    if !agent_connectors.is_empty() {
                        crate::commands::connectors::register_connector_mcp_tools(
                            registry,
                            config_dir,
                            &agent_connectors,
                        )
                        .await;
                    }
                }
            }
        }
    }
}

fn build_subagent_registry(
    app: tauri::AppHandle,
    db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    settings: Arc<Mutex<Settings>>,
    event_sink: Arc<dyn EventSink>,
    plan_store: PlanStore,
    lsp_manager: Arc<LspManager>,
) -> ToolRegistry {
    let config_dir = resolve_global_config_dir(&app).unwrap_or_else(|_| PathBuf::from("."));
    let skills_root = crate::skills::service::skills_root_from_config_dir(&config_dir);
    let skill_loader = Arc::new(Mutex::new(crate::skills::loader::SkillLoader::new(
        skills_root,
    )));
    build_tool_registry(
        app,
        db.clone(),
        db,
        config_dir,
        skill_loader,
        settings,
        event_sink,
        plan_store,
        "plan",
        lsp_manager,
        None,
        &[],
        false,
        false,
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
        None,
        None,
        crate::runtime::task_budget::BudgetScope::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_subagent_research(
    app: tauri::AppHandle,
    db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    settings: Arc<Mutex<Settings>>,
    plan_store: PlanStore,
    lsp_manager: Arc<LspManager>,
    workspace_root: String,
    task: String,
    cancel: Arc<AtomicBool>,
    budget: crate::runtime::task_budget::BudgetScope,
) -> Result<String> {
    let system_prompt = subagent_system_prompt(&workspace_root);
    run_subagent_with_prompt(
        app,
        db,
        settings,
        plan_store,
        lsp_manager,
        workspace_root,
        system_prompt,
        task,
        cancel,
        budget,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_subagent_with_prompt(
    app: tauri::AppHandle,
    db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    settings: Arc<Mutex<Settings>>,
    plan_store: PlanStore,
    lsp_manager: Arc<LspManager>,
    workspace_root: String,
    system_prompt: String,
    task: String,
    cancel: Arc<AtomicBool>,
    budget: crate::runtime::task_budget::BudgetScope,
) -> Result<String> {
    let flash_id = crate::commands::data_scope::resolve_global_config_dir(&app)
        .ok()
        .and_then(|dir| crate::commands::flash::load_flash_provider_id(&dir));
    let runtime = {
        let s = settings.lock().await;
        let flash = flash_id.filter(|id| s.find_llm_provider(id).is_some());
        resolve_llm_runtime(&s, flash.as_deref())?
    };

    let mut registry = build_subagent_registry(
        app,
        db.clone(),
        settings.clone(),
        Arc::new(NullEventSink),
        plan_store,
        lsp_manager,
    );
    crate::tools::remote_fs::install_if_remote(&mut registry, &workspace_root);

    crate::runtime::approval::restrict_background_with_read_roots(
        &mut registry,
        dependency_source_roots(),
    );
    let (
        context_window,
        read_timeout,
        policy_mode,
        rate,
        allow_outside,
        threshold,
        fallback,
        compaction,
        tool_settings,
    ) = {
        let s = settings.lock().await;
        (
            s.context_window,
            s.llm_read_timeout_secs.max(30),
            s.policy_mode.clone(),
            s.tool_rate_limit_per_minute,
            s.allow_outside_workspace,
            s.auto_compact_input_tokens_threshold,
            s.fallback_models.clone(),
            CompactionSettings::from_settings(&s),
            Arc::new(piscis_kernel::agent::tool::ToolSettings::from_settings(&s)),
        )
    };

    let client = llm::build_client_with_timeout(
        &runtime.provider,
        &runtime.api_key,
        if runtime.base_url.is_empty() {
            None
        } else {
            Some(&runtime.base_url)
        },
        read_timeout,
    );
    let client = budget.guard(client);

    let policy = Arc::new(
        PolicyGate::with_profile_and_flags(&workspace_root, &policy_mode, rate, allow_outside)
            .with_extra_read_roots(dependency_source_roots()),
    );

    let harness = HarnessConfig::for_scheduler(
        runtime.model.clone(),
        fallback,
        Arc::new(registry),
        policy,
        system_prompt,
        runtime.max_tokens,
        context_window,
        Some(false),
        threshold,
        compaction,
        db.clone(),
    );
    let agent = harness.into_agent_loop(client, None, None);

    let session_id = format!(
        "subagent-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let ctx = ToolContext {
        session_id: session_id.clone(),
        workspace_root: PathBuf::from(&workspace_root),
        bypass_permissions: false,
        settings: tool_settings,
        max_iterations: Some(10),
        memory_owner_id: "piscis".to_string(),
        pool_session_id: None,
        tool_use_id: None,
        cancel: cancel.clone(),
        loop_halt: None,
    };

    let messages = vec![LlmMessage {
        role: "user".into(),
        content: MessageContent::text(&task),
    }];

    let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);
    let collector = tokio::spawn(async move {
        let mut text = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                AgentEvent::TextDelta { delta } => text.push_str(&delta),
                AgentEvent::Done { .. } => break,
                _ => {}
            }
        }
        text
    });

    let run_fut = agent.run(messages, tx, cancel.clone(), ctx);
    let run_res = tokio::time::timeout(Duration::from_secs(240), run_fut).await;
    let text = collector.await.unwrap_or_default();

    match run_res {
        Ok(Ok(_)) => Ok(text),
        Ok(Err(e)) => Err(anyhow!("sub-agent failed: {e}")),
        Err(_) => Ok(format!(
            "{text}\n\n[sub-agent timed out after 240s — partial findings above]"
        )),
    }
}

struct NullEventSink;

impl EventSink for NullEventSink {
    fn emit_session(&self, _session_id: &str, _event: &str, _payload: serde_json::Value) {}
    fn emit_broadcast(&self, _event: &str, _payload: serde_json::Value) {}
}
