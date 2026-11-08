//! Workbench management commands — installed skills, project rules, and hooks.
//!
//! These power the unified Settings page tabs:
//! - **Skills**: list / uninstall packages under `{config}/skills/{slug}/`.
//! - **Rules**: CRUD on `{project}/.agentz/rules/*.md` (Cursor-style project rules).
//! - **Hooks**: read / write `{project}/.agentz/hooks.json` and test-run a hook.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::commands::data_scope::resolve_global_config_dir;

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct InstalledSkill {
    pub slug: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub description_zh: String,
    pub path: String,
    #[serde(default)]
    pub lifecycle: String,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub quadrant: String,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub version: Option<String>,
    pub registry_version: Option<String>,
    pub content_sha256: Option<String>,
    pub content_hash_matches: Option<bool>,
    pub requested_tools: Vec<String>,
    pub install_history: Vec<crate::skills::provenance::SkillInstallRecord>,
}

fn sanitize_slug(slug: &str) -> Result<String, String> {
    let slug = slug.trim();
    if slug.is_empty()
        || !slug
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        || slug.contains("..")
    {
        return Err(format!("invalid skill slug: '{slug}'"));
    }
    Ok(slug.to_string())
}

/// List skills from quadrant storage (installed, draft, learned).
#[tauri::command]
pub async fn skills_list_installed(app: AppHandle) -> Result<Vec<InstalledSkill>, String> {
    let config_dir = resolve_global_config_dir(&app)?;
    let root = crate::skills::service::skills_root_from_config_dir(&config_dir);
    let mut loader = crate::skills::loader::SkillLoader::new(&root);
    let _ = loader.load_all();
    let ctx = crate::commands::skill_evolution_ctx::SkillEvolutionCtx::open(&app)?;
    let db = ctx.db.lock().await;
    let mut out = Vec::new();
    for skill in loader.list_skills() {
        let meta = db
            .get_skill(&skill.skill_id)
            .ok()
            .flatten()
            .map(|s| crate::skills::provenance::SkillConfigMeta::from_json(&s.config))
            .unwrap_or_else(|| crate::skills::provenance::SkillConfigMeta {
                lifecycle: skill.lifecycle.clone(),
                locked: skill.locked,
                ..Default::default()
            });
        let content_hash_matches = meta.content_sha256.as_ref().map(|expected| {
            std::fs::read_to_string(skill.source_path.join("SKILL.md"))
                .map(|content| crate::skills::provenance::content_sha256(&content) == *expected)
                .unwrap_or(false)
        });
        let quadrant = match skill.lifecycle.as_str() {
            crate::skills::provenance::LIFECYCLE_DRAFT => "draft",
            crate::skills::provenance::LIFECYCLE_LEARNED => "learned",
            crate::skills::provenance::LIFECYCLE_ARCHIVED => "archived",
            _ => "installed",
        };
        out.push(InstalledSkill {
            slug: skill.skill_id.clone(),
            name: skill.name.clone(),
            description: skill.description.clone(),
            description_zh: skill.description_zh.clone(),
            path: skill.source_path.join("SKILL.md").display().to_string(),
            lifecycle: meta.lifecycle,
            locked: meta.locked,
            pinned: meta.pinned,
            quadrant: quadrant.to_string(),
            source: meta.source,
            source_url: meta.source_url,
            version: meta.version,
            registry_version: meta.registry_version,
            content_sha256: meta.content_sha256,
            content_hash_matches,
            requested_tools: meta.requested_tools,
            install_history: meta.install_history,
        });
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

/// Remove a skill directory (any quadrant) and DB record.
#[tauri::command]
pub async fn skills_uninstall(app: AppHandle, slug: String) -> Result<(), String> {
    let slug = sanitize_slug(&slug)?;
    let config_dir = resolve_global_config_dir(&app)?;
    if let Some(fish) = crate::commands::fish::find_user_fish_for_skill(&config_dir, &slug) {
        let agent = if fish.name.trim().is_empty() {
            fish.id.clone()
        } else {
            fish.name.clone()
        };
        return Err(format!(
            "skill '{slug}' is referenced by anonymous agent '{agent}'; delete the anonymous agent first"
        ));
    }
    let ctx = crate::commands::skill_evolution_ctx::SkillEvolutionCtx::open(&app)?;
    let root = ctx.skills_root();
    let db = ctx.db.lock().await;
    crate::skills::service::uninstall_installed(&db, &root, &slug).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Project rules (Cursor-style)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct RuleFile {
    pub name: String,
    pub enabled: bool,
    pub size: u64,
    pub path: String,
}

/// `{project}/.agentz/rules`
fn rules_dir(project_dir: &str) -> Result<PathBuf, String> {
    let trimmed = project_dir.trim();
    if trimmed.is_empty() {
        return Err("project directory is empty".to_string());
    }
    let root = PathBuf::from(trimmed);
    if !root.is_dir() {
        return Err(format!("project directory not found: {trimmed}"));
    }
    Ok(root.join(".agentz").join("rules"))
}

/// Reject names that escape the rules directory.
fn safe_rule_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("invalid rule name: '{name}'"));
    }
    Ok(name.to_string())
}

/// A rule file is "enabled" when it ends in `.md`/`.mdc`; disabled rules carry a
/// trailing `.disabled` so the agent's rule loader skips them.
fn rule_is_enabled(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".mdc")
}

/// List rule files under `{project}/.agentz/rules/`.
#[tauri::command]
pub fn rules_list(project_dir: String) -> Result<Vec<RuleFile>, String> {
    let dir = rules_dir(&project_dir)?;
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        let is_rule = lower.ends_with(".md")
            || lower.ends_with(".mdc")
            || lower.ends_with(".md.disabled")
            || lower.ends_with(".mdc.disabled");
        if !is_rule {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(RuleFile {
            enabled: rule_is_enabled(&name),
            name,
            size,
            path: path.display().to_string(),
        });
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

/// Read the contents of a single rule file.
#[tauri::command]
pub fn rules_read(project_dir: String, name: String) -> Result<String, String> {
    let dir = rules_dir(&project_dir)?;
    let name = safe_rule_name(&name)?;
    std::fs::read_to_string(dir.join(&name)).map_err(|e| e.to_string())
}

/// Create or overwrite a rule file. A `.md` extension is appended when missing.
#[tauri::command]
pub fn rules_write(project_dir: String, name: String, content: String) -> Result<String, String> {
    let dir = rules_dir(&project_dir)?;
    let mut name = safe_rule_name(&name)?;
    let lower = name.to_lowercase();
    if !(lower.ends_with(".md") || lower.ends_with(".mdc")) {
        name.push_str(".md");
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(&name), content).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Delete a rule file.
#[tauri::command]
pub fn rules_delete(project_dir: String, name: String) -> Result<(), String> {
    let dir = rules_dir(&project_dir)?;
    let name = safe_rule_name(&name)?;
    let path = dir.join(&name);
    if !path.is_file() {
        return Err(format!("rule not found: {name}"));
    }
    std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Toggle a rule on/off by adding/removing a trailing `.disabled` suffix.
#[tauri::command]
pub fn rules_set_enabled(
    project_dir: String,
    name: String,
    enabled: bool,
) -> Result<String, String> {
    let dir = rules_dir(&project_dir)?;
    let name = safe_rule_name(&name)?;
    let src = dir.join(&name);
    if !src.is_file() {
        return Err(format!("rule not found: {name}"));
    }
    let currently_enabled = rule_is_enabled(&name);
    if currently_enabled == enabled {
        return Ok(name);
    }
    let new_name = if enabled {
        name.trim_end_matches(".disabled").to_string()
    } else {
        format!("{name}.disabled")
    };
    let dst = dir.join(&new_name);
    std::fs::rename(&src, &dst).map_err(|e| e.to_string())?;
    Ok(new_name)
}

// ---------------------------------------------------------------------------
// Hooks (Cursor-style hooks.json)
// ---------------------------------------------------------------------------

/// A single user-defined hook: a shell command run on a lifecycle event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// One of: `beforeAgentTurn`, `afterAgentTurn`, `beforeFileEdit`, `afterFileEdit`.
    pub event: String,
    pub command: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub hooks: Vec<HookDef>,
}

fn default_version() -> u32 {
    1
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            version: 1,
            hooks: Vec::new(),
        }
    }
}

/// `{project}/.agentz/hooks.json`
fn hooks_path(project_dir: &str) -> Result<PathBuf, String> {
    let trimmed = project_dir.trim();
    if trimmed.is_empty() {
        return Err("project directory is empty".to_string());
    }
    let root = PathBuf::from(trimmed);
    if !root.is_dir() {
        return Err(format!("project directory not found: {trimmed}"));
    }
    Ok(root.join(".agentz").join("hooks.json"))
}

const HOOK_EVENTS: &[&str] = &[
    "beforeAgentTurn",
    "afterAgentTurn",
    "beforeFileEdit",
    "afterFileEdit",
];

/// Read the project hooks config (returns an empty config when absent).
#[tauri::command]
pub fn hooks_get(project_dir: String) -> Result<HooksConfig, String> {
    let path = hooks_path(&project_dir)?;
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Ok(HooksConfig::default());
    };
    if raw.trim().is_empty() {
        return Ok(HooksConfig::default());
    }
    serde_json::from_str(&raw).map_err(|e| format!("invalid hooks.json: {e}"))
}

/// Persist the project hooks config to `{project}/.agentz/hooks.json`.
#[tauri::command]
pub fn hooks_save(project_dir: String, config: HooksConfig) -> Result<(), String> {
    for hook in &config.hooks {
        if !HOOK_EVENTS.contains(&hook.event.as_str()) {
            return Err(format!("unknown hook event: {}", hook.event));
        }
    }
    let path = hooks_path(&project_dir)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct HookRunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Test-run a hook command in the project directory (10s timeout).
#[tauri::command]
pub async fn hooks_run(project_dir: String, command: String) -> Result<HookRunResult, String> {
    let trimmed_dir = project_dir.trim();
    if trimmed_dir.is_empty() || !Path::new(trimmed_dir).is_dir() {
        return Err(format!("project directory not found: {trimmed_dir}"));
    }
    run_hook_command(trimmed_dir, &command)
        .await
        .map_err(|e| e.to_string())
}

/// Spawn a shell command rooted at `cwd`, capturing stdout/stderr with a timeout.
pub async fn run_hook_command(cwd: &str, command: &str) -> Result<HookRunResult, std::io::Error> {
    run_hook_command_with_env(cwd, command, &[]).await
}

/// Like [`run_hook_command`] but also sets environment variables (event
/// context such as `AGENTZ_FILE_PATH`).
pub async fn run_hook_command_with_env(
    cwd: &str,
    command: &str,
    env: &[(&str, String)],
) -> Result<HookRunResult, std::io::Error> {
    use tokio::process::Command;

    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };

    cmd.current_dir(cwd);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.kill_on_drop(true);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let child = cmd.spawn()?;
    let out =
        match tokio::time::timeout(std::time::Duration::from_secs(10), child.wait_with_output())
            .await
        {
            Ok(res) => res?,
            Err(_) => {
                return Ok(HookRunResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: "hook timed out after 10s".to_string(),
                });
            }
        };

    Ok(HookRunResult {
        exit_code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Run all enabled hooks for `event` and return their combined stdout as a
/// system-prompt section, or `None` when there's nothing to inject. Failures
/// are swallowed so a misconfigured hook never breaks an agent turn.
pub async fn run_event_hooks(workspace_root: &str, event: &str) -> Option<String> {
    let trimmed = workspace_root.trim();
    if trimmed.is_empty() || !Path::new(trimmed).is_dir() {
        return None;
    }
    let config = hooks_get(trimmed.to_string()).ok()?;
    let mut blocks: Vec<String> = Vec::new();
    for hook in config
        .hooks
        .iter()
        .filter(|h| h.enabled && h.event == event)
    {
        if hook.command.trim().is_empty() {
            continue;
        }
        if let Ok(res) = run_hook_command(trimmed, &hook.command).await {
            let label = if hook.name.trim().is_empty() {
                hook.id.clone()
            } else {
                hook.name.clone()
            };
            let body = res.stdout.trim();
            if !body.is_empty() {
                blocks.push(format!("### {label}\n{body}"));
            }
        }
    }
    if blocks.is_empty() {
        return None;
    }
    Some(format!(
        "## Hook output ({event})\nThe following context was produced by project hooks:\n{}",
        blocks.join("\n\n")
    ))
}

/// Enabled, non-empty hooks of `event` for a valid project directory.
fn enabled_hooks(workspace_root: &str, event: &str) -> Vec<HookDef> {
    let trimmed = workspace_root.trim();
    if trimmed.is_empty() || !Path::new(trimmed).is_dir() {
        return Vec::new();
    }
    hooks_get(trimmed.to_string())
        .map(|c| {
            c.hooks
                .into_iter()
                .filter(|h| h.enabled && h.event == event && !h.command.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn hook_label(hook: &HookDef) -> &str {
    if hook.name.trim().is_empty() {
        &hook.id
    } else {
        &hook.name
    }
}

/// Environment passed to every hook command so scripts know what happened.
fn hook_env(
    workspace_root: &str,
    event: &str,
    session_id: Option<&str>,
    tool_name: Option<&str>,
    file_path: Option<&str>,
) -> Vec<(&'static str, String)> {
    let mut env = vec![
        ("AGENTZ_HOOK_EVENT", event.to_string()),
        ("AGENTZ_PROJECT_DIR", workspace_root.trim().to_string()),
    ];
    if let Some(v) = session_id {
        env.push(("AGENTZ_SESSION_ID", v.to_string()));
    }
    if let Some(v) = tool_name {
        env.push(("AGENTZ_TOOL_NAME", v.to_string()));
    }
    if let Some(v) = file_path {
        env.push(("AGENTZ_FILE_PATH", v.to_string()));
    }
    env
}

fn clip_hook_text(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let mut out: String = t.chars().take(max).collect();
        out.push('…');
        out
    }
}

/// Run the enabled `beforeFileEdit` hooks. A hook that exits with a positive
/// status blocks the edit: the returned message goes back to the model.
/// Spawn failures, timeouts and signal exits never block (fail open), so a
/// broken hook cannot wedge the agent.
pub async fn run_before_file_edit_hooks(
    workspace_root: &str,
    session_id: &str,
    tool_name: &str,
    file_path: Option<&str>,
) -> Option<String> {
    let root = workspace_root.trim();
    for hook in enabled_hooks(root, "beforeFileEdit") {
        let env = hook_env(root, "beforeFileEdit", Some(session_id), Some(tool_name), file_path);
        let Ok(res) = run_hook_command_with_env(root, &hook.command, &env).await else {
            continue;
        };
        if res.exit_code > 0 {
            let detail = if res.stderr.trim().is_empty() {
                res.stdout.as_str()
            } else {
                res.stderr.as_str()
            };
            return Some(format!(
                "Edit blocked by project hook '{}' (beforeFileEdit, exit code {}): {}",
                hook_label(&hook),
                res.exit_code,
                clip_hook_text(detail, 1000)
            ));
        }
    }
    None
}

/// Run the enabled hooks of an "after" event (`afterFileEdit`,
/// `afterAgentTurn`) for their side effects (formatters, linters, notifiers).
/// Output and exit codes are only logged: the model never sees them.
pub async fn run_after_hooks(
    workspace_root: &str,
    event: &str,
    session_id: Option<&str>,
    tool_name: Option<&str>,
    file_path: Option<&str>,
) {
    let root = workspace_root.trim();
    for hook in enabled_hooks(root, event) {
        let env = hook_env(root, event, session_id, tool_name, file_path);
        match run_hook_command_with_env(root, &hook.command, &env).await {
            Ok(res) if res.exit_code != 0 => tracing::debug!(
                "hook '{}' ({event}) exited with {}: {}",
                hook_label(&hook),
                res.exit_code,
                clip_hook_text(&res.stderr, 300)
            ),
            Ok(_) => {}
            Err(e) => tracing::debug!("hook '{}' ({event}) failed to start: {e}", hook_label(&hook)),
        }
    }
}

#[cfg(test)]
mod hook_tests {
    use super::*;

    fn project(hooks: serde_json::Value) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "agentz-hooks-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join(".agentz")).unwrap();
        std::fs::write(
            dir.join(".agentz/hooks.json"),
            serde_json::json!({"version": 1, "hooks": hooks}).to_string(),
        )
        .unwrap();
        dir
    }

    fn hook(id: &str, event: &str, command: &str, enabled: bool) -> serde_json::Value {
        serde_json::json!({"id": id, "name": id, "event": event, "command": command, "enabled": enabled})
    }

    #[tokio::test]
    async fn before_file_edit_blocks_only_on_positive_exit() {
        let dir = project(serde_json::json!([hook("ok", "beforeFileEdit", "exit 0", true)]));
        let root = dir.to_string_lossy().to_string();
        assert!(run_before_file_edit_hooks(&root, "s", "file_write", Some("a.txt")).await.is_none());

        let dir = project(serde_json::json!([
            hook("ok", "beforeFileEdit", "exit 0", true),
            hook("guard", "beforeFileEdit", "echo nope 1>&2 && exit 3", true),
        ]));
        let root = dir.to_string_lossy().to_string();
        let msg = run_before_file_edit_hooks(&root, "s", "file_write", Some("a.txt"))
            .await
            .expect("blocked");
        assert!(msg.contains("guard") && msg.contains("exit code 3") && msg.contains("nope"), "{msg}");
    }

    #[tokio::test]
    async fn disabled_and_other_event_hooks_never_block() {
        let dir = project(serde_json::json!([
            hook("off", "beforeFileEdit", "exit 9", false),
            hook("other", "afterFileEdit", "exit 9", true),
        ]));
        let root = dir.to_string_lossy().to_string();
        assert!(run_before_file_edit_hooks(&root, "s", "file_edit", None).await.is_none());
    }

    #[tokio::test]
    async fn missing_hooks_file_is_a_no_op() {
        let dir = std::env::temp_dir();
        let root = dir.to_string_lossy().to_string();
        assert!(enabled_hooks(&root, "afterAgentTurn").is_empty());
    }

    #[tokio::test]
    async fn after_hooks_run_with_event_context_in_env() {
        #[cfg(windows)]
        let cmd = "echo %AGENTZ_HOOK_EVENT% %AGENTZ_FILE_PATH%> hook-out.txt";
        #[cfg(not(windows))]
        let cmd = "echo \"$AGENTZ_HOOK_EVENT $AGENTZ_FILE_PATH\" > hook-out.txt";
        let dir = project(serde_json::json!([hook("fmt", "afterFileEdit", cmd, true)]));
        let root = dir.to_string_lossy().to_string();
        run_after_hooks(&root, "afterFileEdit", Some("s"), Some("file_edit"), Some("src/a.rs")).await;
        let out = std::fs::read_to_string(dir.join("hook-out.txt")).unwrap();
        assert!(out.contains("afterFileEdit") && out.contains("src/a.rs"), "{out}");
    }
}
