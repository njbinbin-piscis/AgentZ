//! `task` — unified delegation to a registered agent or a builtin Fish.
//!
//! The target's manifest (prompt, model, skills, tool allowlist, permission
//! mode) is reused instead of introducing another config surface. Effective
//! capabilities are resolved by [`crate::runtime::subagent_policy`]: the call
//! can only narrow, never widen. A writable agent (manifest `permission_mode`
//! `ask`/`inherit`, parent not read-only, call not forced read-only) gets the
//! agent-mode tools and runs without per-action approval: enabling the agent is
//! the authorization. Safety is isolation: in a git repo each writer works in
//! its own worktree and is three-way merged back (conflicts reported); outside
//! git, writers are serialized. See [`crate::runtime::subagent_worktree`].

use std::sync::Arc;

use async_trait::async_trait;
use piscis_kernel::agent::plan::PlanStore;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use piscis_kernel::store::settings::Settings;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::commands::agents::{load_all_manifests, AgentManifest};
use crate::commands::chat_turn::{
    load_installed_skills, run_subagent_with_options, SubagentOptions,
};
use crate::commands::fish::{find_fish, load_fish_library};
use crate::commands::system_prompt::{delegated_agent_system_prompt, fish_system_prompt};
use crate::lsp::manager::LspManager;
use crate::runtime::subagent_bridge::BridgeSlot;
use crate::runtime::subagent_worktree as wt;
use crate::runtime::subagent_policy::{
    cap_report, resolve, CallNarrowing, ManifestCaps, ParentPolicy, PermissionMode,
};

const MIN_BRIEF_CHARS: usize = 20;
const MAX_REPORT_CHARS: usize = 12_000;

pub struct TaskTool {
    pub db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    pub settings: Arc<Mutex<Settings>>,
    pub plan_store: PlanStore,
    pub lsp_manager: Arc<LspManager>,
    pub app: tauri::AppHandle,
    pub budget: crate::runtime::task_budget::BudgetScope,
    /// Parent turn hooks/session, filled once the parent harness is built.
    pub bridge: BridgeSlot,
}

/// Validate a delegation brief. Returns an error message when unusable.
pub(crate) fn validate_brief(brief: &str) -> Result<(), String> {
    let n = brief.trim().chars().count();
    if n == 0 {
        return Err("'brief' is required".into());
    }
    if n < MIN_BRIEF_CHARS {
        return Err(format!(
            "'brief' is too short ({n} chars). The sub-agent cannot see your conversation: \
             state the goal, what you already know, and the output format you want."
        ));
    }
    Ok(())
}

fn persona_of(m: &AgentManifest) -> String {
    let mut p = if m.system_prompt.trim().is_empty() {
        m.system_prompt_zh.clone()
    } else {
        m.system_prompt.clone()
    };
    if p.trim().is_empty() {
        p = m.description.clone();
    }
    p
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Delegate a self-contained job to a registered sub-agent (or builtin Fish) so \
         its intermediate steps never enter your context. The sub-agent has NO access \
         to your conversation: the brief must state the goal, what you already know, \
         and the output format you want. Builtin presets are read-only; a registered \
         agent marked writable can edit files and run commands without asking but cannot \
         delegate further. In a git repo each writer works in an isolated copy that is \
         merged back when it finishes (unmergeable files are reported), so emit several \
         independent `task` calls in one turn to run them in parallel. Outside git, \
         writers run one at a time.\n\
         \n\
         Parameters:\n\
         - 'action': 'list' to see delegable agents, or 'call' (default).\n\
         - 'agent_id': id of a delegable agent or builtin Fish (action=call).\n\
         - 'brief': self-contained task description (action=call).\n\
         - 'read_only': optional; true forces a writable agent to run read-only.\n\
         - 'isolation': optional; 'direct' makes a writable agent edit the project \
           in place (exclusive: waits for other writers; a git checkpoint is taken first). \
           Default is the isolated copy."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["list", "call"]},
                "agent_id": {"type": "string", "description": "Delegable agent id or builtin Fish id."},
                "brief": {"type": "string", "description": "Self-contained brief."},
                "read_only": {"type": "boolean", "description": "true forces read-only; never widens."},
                "isolation": {
                    "type": "string",
                    "enum": ["worktree", "direct"],
                    "description": "Writable agents only; default worktree."
                }
            },
            "required": ["action"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("call")
            .trim()
            .to_lowercase();
        let config_dir = crate::commands::data_scope::resolve_global_config_dir(&self.app)
            .map_err(|e| anyhow::anyhow!(e))?;
        let manifests = load_all_manifests(&config_dir.join("agents"));

        if action == "list" {
            let mut lines: Vec<String> = manifests
                .iter()
                .filter(|m| m.delegable)
                .map(|m| {
                    let mode = PermissionMode::parse(&m.permission_mode);
                    format!(
                        "- {} ({}) [{}]: {}",
                        m.id,
                        m.name,
                        if mode == PermissionMode::ReadOnly { "read-only" } else { "can write, isolated + merged" },
                        m.description
                    )
                })
                .collect();
            lines.extend(
                load_fish_library(&config_dir)
                    .iter()
                    .filter(|f| !manifests.iter().any(|m| m.id == f.id))
                    .map(|f| format!("- {} ({}) [read-only]: {}", f.id, f.name, f.description)),
            );
            return Ok(ToolResult::ok(format!(
                "Available sub-agents:\n{}",
                lines.join("\n")
            )));
        }

        let agent_id = input
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let brief = input
            .get("brief")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if agent_id.is_empty() {
            return Ok(ToolResult::err(
                "'agent_id' is required (use action=list to discover ids)",
            ));
        }
        if let Err(e) = validate_brief(&brief) {
            return Ok(ToolResult::err(e));
        }
        // The call may only restrict: `read_only: true` forces read-only, while
        // omitting it or passing false never widens beyond manifest and parent.
        let force_read_only = input
            .get("read_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let direct_requested = input
            .get("isolation")
            .and_then(|v| v.as_str())
            .map_or(false, |s| s.eq_ignore_ascii_case("direct"));

        let mut workspace = ctx.workspace_root.to_string_lossy().to_string();
        // Without a parent bridge we cannot tell whether the parent is read-only
        // (plan mode), so stay read-only.
        let bridge = self.bridge.get();
        let parent = ParentPolicy {
            read_only: bridge.map_or(true, |b| b.read_only),
        };
        let narrow = CallNarrowing { force_read_only };

        let original_workspace = workspace.clone();
        let (mut system_prompt, display, options) =
            if let Some(m) = manifests.iter().find(|m| m.id == agent_id) {
                if !m.delegable {
                    return Ok(ToolResult::err(format!(
                        "agent '{agent_id}' is not delegable (enable `delegable` in its manifest)"
                    )));
                }
                let caps = resolve(
                    &ManifestCaps {
                        tool_allowlist: m.tool_allowlist.clone(),
                        mode: PermissionMode::parse(&m.permission_mode),
                    },
                    &narrow,
                    &parent,
                );
                let mut persona = persona_of(m);
                // Only the manifest's own skills; never the parent's selection.
                if !m.skills.is_empty() {
                    for s in load_installed_skills(&config_dir)
                        .into_iter()
                        .filter(|s| m.skills.iter().any(|w| w.eq_ignore_ascii_case(&s.slug)))
                    {
                        persona.push_str(&format!("\n\n## Skill: {}\n{}", s.name, s.body));
                    }
                }
                let writable = !caps.read_only && bridge.is_some();
                // Writing and testing takes longer than scouting.
                let (default_iters, default_timeout) = if writable { (40, 900) } else { (0, 0) };
                (
                    delegated_agent_system_prompt(&workspace, &m.name, &persona, writable),
                    m.name.clone(),
                    SubagentOptions {
                        provider_id: m.llm_provider_id.clone(),
                        caps: Some(caps),
                        max_iterations: Some(if m.max_iterations > 0 { m.max_iterations } else { default_iters }),
                        timeout_secs: Some(if m.task_timeout_secs > 0 {
                            m.task_timeout_secs as u64
                        } else {
                            default_timeout
                        }),
                        writable,
                        hooks: None,
                    },
                )
            } else if let Some(f) = find_fish(&config_dir, &agent_id) {
                (
                    fish_system_prompt(&workspace, &f.name, &f.system_prompt),
                    f.name.clone(),
                    SubagentOptions::default(),
                )
            } else {
                return Ok(ToolResult::err(format!(
                    "unknown agent '{agent_id}' — use action=list"
                )));
            };

        // Writers: isolated worktree (parallel) in a git repo, otherwise exclusive
        // and serialized. Guards live until the run (and merge) finishes.
        let run_id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        let mut read_guard: Option<tokio::sync::RwLockReadGuard<'static, ()>> = None;
        let mut write_guard: Option<tokio::sync::RwLockWriteGuard<'static, ()>> = None;
        let mut isolation: Option<wt::Isolation> = None;
        let mut isolation_note = String::new();
        let mut in_place_ck: Option<String> = None;
        let was_writable = options.writable;
        let wt_root = config_dir.join("subagent-worktrees");
        let conflict_dir = ctx.workspace_root.join(".agentz").join("subagent-conflicts");
        if options.writable {
            let ws_path = ctx.workspace_root.clone();
            let repo = if direct_requested {
                None
            } else {
                let p = ws_path.clone();
                tokio::task::spawn_blocking(move || wt::repo_root(&p)).await.ok().flatten()
            };
            if let Some(root) = repo {
                read_guard = Some(wt::WRITERS.read().await);
                let (r, id, w) = (root, run_id.clone(), wt_root.clone());
                match tokio::task::spawn_blocking(move || wt::create(&r, &id, &w)).await {
                    Ok(Ok(iso)) => {
                        workspace = wt::workspace_in(&iso, &ws_path).to_string_lossy().to_string();
                        isolation = Some(iso);
                    }
                    Ok(Err(e)) => {
                        isolation_note = format!("worktree creation failed ({e}), so it fell back to in-place");
                    }
                    Err(e) => {
                        isolation_note = format!("worktree creation failed ({e}), so it fell back to in-place")
                    }
                }
            } else if direct_requested {
                isolation_note = "in-place was requested by the caller (isolation: direct)".into();
            } else {
                isolation_note = "the project is not a git repository, so writers are serialized".into();
            }
            if isolation.is_none() {
                read_guard = None;
                write_guard = Some(wt::WRITERS.write().await);
                // Git checkpoint so an in-place run can be rolled back.
                let p = ws_path.clone();
                let id = run_id.clone();
                let ck = tokio::task::spawn_blocking(move || {
                    wt::repo_root(&p).and_then(|r| wt::checkpoint_in_place(&r, &id))
                })
                .await
                .ok()
                .flatten();
                in_place_ck = ck;
            }
        }
        let _guards = (read_guard, write_guard);
        if workspace != original_workspace {
            system_prompt = system_prompt.replace(&original_workspace, &workspace);
        }

        // Result cache: same agent + brief + workspace, valid only while the
        // workspace (git HEAD, dirty files and their mtimes) is unchanged.
        // Writing runs are never cached (they change the workspace).
        let cache_key = crate::commands::delegation::cache_key(&agent_id, &brief, &workspace);
        let stamp = if options.writable {
            None
        } else {
            let ws = workspace.clone();
            tokio::task::spawn_blocking(move || crate::commands::delegation::workspace_stamp(&ws))
                .await
                .ok()
                .flatten()
        };
        if let Some(stamp) = stamp.as_deref() {
            let hit = crate::commands::delegation::global_cache()
                .lock()
                .ok()
                .and_then(|mut c| c.get(&cache_key, stamp, std::time::Instant::now()));
            if let Some(cached) = hit {
                return Ok(ToolResult::ok(format!(
                    "{display} report (cached, workspace unchanged):\n\n{cached}"
                )));
            }
        }

        let outcome = run_subagent_with_options(
            self.app.clone(),
            self.db.clone(),
            self.settings.clone(),
            self.plan_store.clone(),
            self.lsp_manager.clone(),
            workspace,
            system_prompt,
            brief,
            ctx.cancel.clone(),
            self.budget.clone(),
            options,
        )
        .await;

        // Merge the isolated run back (also when it timed out: partial work is
        // still work). A failed/cancelled run only keeps recovery refs.
        let merge_note = match isolation {
            Some(iso) => {
                let merge = matches!(&outcome, Ok(_));
                let cd = conflict_dir.clone();
                match tokio::task::spawn_blocking(move || wt::finish(iso, merge, &cd)).await {
                    Ok(rep) => rep.render(),
                    Err(e) => format!("Merge step failed: {e}"),
                }
            }
            None if was_writable => {
                wt::render_in_place(&run_id, in_place_ck.as_deref(), &isolation_note)
            }
            None => String::new(),
        };

        match outcome {
            Ok(o) if o.text.trim().is_empty() && o.edited_paths.is_empty() && merge_note.is_empty() => {
                Ok(ToolResult::ok(format!("[{display} returned no findings]")))
            }
            Ok(o) => {
                let mut body = cap_report(&o.text, MAX_REPORT_CHARS);
                if !merge_note.is_empty() {
                    body.push_str(&format!("\n\n{merge_note}"));
                }
                if !o.edited_paths.is_empty() {
                    body.push_str(&format!(
                        "\n\nFiles written or edited ({}):\n{}",
                        o.edited_paths.len(),
                        o.edited_paths
                            .iter()
                            .map(|p| format!("- {p}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ));
                }
                if !o.timed_out {
                    if let Some(stamp) = stamp {
                        if let Ok(mut c) = crate::commands::delegation::global_cache().lock() {
                            c.put(cache_key, stamp, body.clone(), std::time::Instant::now());
                        }
                    }
                }
                Ok(ToolResult::ok(format!(
                    "{display} report ({} tool calls, {}+{} tokens{}{}):\n\n{}",
                    o.tool_calls,
                    o.input_tokens,
                    o.output_tokens,
                    if o.timed_out { ", timed out" } else { "" },
                    if o.model_fallback { ", main-model fallback" } else { "" },
                    body
                )))
            }
            Err(e) => Ok(ToolResult::err(format!(
                "{}{}",
                crate::commands::delegation::explain_failure(&e.to_string()),
                if merge_note.is_empty() { String::new() } else { format!("\n{merge_note}") }
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_brief_rejected() {
        assert!(validate_brief("   ").unwrap_err().contains("required"));
    }

    #[test]
    fn short_brief_rejected() {
        assert!(validate_brief("find foo").unwrap_err().contains("too short"));
    }

    #[test]
    fn adequate_brief_accepted() {
        assert!(validate_brief("Find every call site of load_config and list file:line.").is_ok());
    }
}
