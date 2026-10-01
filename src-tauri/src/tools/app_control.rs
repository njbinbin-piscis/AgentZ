//! `app_control` — let the agent manage the app on the user's behalf: read and
//! change system settings (through a tiered, audited catalog), manage
//! assistants and teams, and undo its own changes.
//!
//! Safety model: secrets and authorization switches are locked; privileged
//! fields, assistant/team edits and rollbacks prompt the user on every call;
//! every write is validated, rate limited, diffed, audited and reversible.

use async_trait::async_trait;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use piscis_kernel::store::settings::Settings;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

use crate::commands::agents::{agents_get, agents_list, agents_save, agents_uninstall, AgentManifest};
use crate::commands::app_changes::{self, ChangeRecord};
use crate::commands::data_scope::resolve_global_config_dir;
use crate::commands::teams::{teams_get, teams_list, teams_save, teams_uninstall, TeamManifest};
use crate::tools::settings_catalog::{self as catalog, Tier};

pub use crate::commands::app_changes::APP_CONTROL_UPDATED_EVENT;

const INSTRUCTIONS_KEY: &str = "piscis_personal_prompt";
const MAX_APPEND_CHARS: usize = 1000;
const MAX_INSTRUCTIONS_CHARS: usize = 8000;

pub struct AppControlTool {
    pub app: AppHandle,
}

fn bad(msg: impl Into<String>) -> anyhow::Result<ToolResult> {
    Ok(ToolResult::err(msg.into()))
}

impl AppControlTool {
    fn config_dir(&self) -> anyhow::Result<PathBuf> {
        resolve_global_config_dir(&self.app).map_err(|e| anyhow::anyhow!(e))
    }

    fn notify(&self, kind: &str) {
        let _ = self.app.emit(APP_CONTROL_UPDATED_EVENT, json!({ "kind": kind }));
    }

    async fn get_settings(&self) -> anyhow::Result<ToolResult> {
        let path = self.config_dir()?.join("config.json");
        let settings = Settings::load(&path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let mut value = serde_json::to_value(&settings)?;
        redact_secrets(&mut value);
        Ok(ToolResult::ok(serde_json::to_string_pretty(&value)?))
    }

    /// Validated, audited settings write shared by `update_settings` and `append_instructions`.
    fn apply_settings(
        &self,
        patch: &Value,
        session: &str,
        allow_instructions: bool,
    ) -> anyhow::Result<ToolResult> {
        let Some(obj) = patch.as_object().filter(|o| !o.is_empty()) else {
            return bad("`settings` must be a non-empty JSON object of fields to change.");
        };
        let mut problems = vec![];
        for (key, value) in obj {
            if key == INSTRUCTIONS_KEY && !allow_instructions {
                problems.push(format!("`{key}`: use the append_instructions action"));
                continue;
            }
            match catalog::tier_of(key) {
                Tier::Locked => problems.push(format!(
                    "`{key}` is locked or not exposed; ask the user to change it in Settings"
                )),
                _ => {
                    if let Err(e) = catalog::validate(key, value) {
                        problems.push(e);
                    }
                }
            }
        }
        if let Some(hit) = catalog::find_secret_write(patch, "") {
            problems.push(format!(
                "`{hit}` is a secret; secrets can only be changed by the user in Settings"
            ));
        }
        if !problems.is_empty() {
            return bad(format!("Rejected:\n- {}", problems.join("\n- ")));
        }

        let dir = self.config_dir()?;
        let path = dir.join("config.json");
        let settings = Settings::load(&path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let old = serde_json::to_value(&settings)?;
        let mut clean = patch.clone();
        catalog::strip_secrets(&mut clean);
        let mut new = old.clone();
        catalog::merge(&mut new, &clean);

        let mut diff = vec![];
        let mut before = serde_json::Map::new();
        for key in obj.keys() {
            let o = old.get(key).cloned().unwrap_or(Value::Null);
            let n = new.get(key).cloned().unwrap_or(Value::Null);
            let mut d = vec![];
            catalog::diff(key, &o, &n, &mut d);
            if !d.is_empty() {
                before.insert(key.clone(), o);
                diff.extend(d);
            }
        }
        if diff.is_empty() {
            return Ok(ToolResult::ok("No changes: the values already match."));
        }
        if let Err(e) = app_changes::check_rate_limit(session) {
            return bad(e);
        }

        let mut merged: Settings = serde_json::from_value(new)
            .map_err(|e| anyhow::anyhow!("invalid settings after merge: {e}"))?;
        merged.config_path = path;
        merged.save().map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let fields: Vec<String> = before.keys().cloned().collect();
        let id = app_changes::record(
            &dir,
            ChangeRecord {
                session,
                kind: "settings",
                target: &fields.join(","),
                summary: format!("settings: {}", fields.join(", ")),
                diff: diff.clone(),
                before: Value::Object(before),
            },
        )
        .map_err(|e| anyhow::anyhow!(e))?;
        self.notify("settings");
        Ok(ToolResult::ok(serde_json::to_string_pretty(&json!({
            "change_id": id,
            "applied": diff,
            "hint": "Use rollback with this change_id to undo."
        }))?))
    }

    async fn append_instructions(&self, text: &str, session: &str) -> anyhow::Result<ToolResult> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > MAX_APPEND_CHARS {
            return bad(format!("`text` must be 1..={MAX_APPEND_CHARS} characters."));
        }
        let path = self.config_dir()?.join("config.json");
        let settings = Settings::load(&path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let current = settings.piscis_personal_prompt.clone();
        let combined = if current.trim().is_empty() {
            text.to_string()
        } else {
            format!("{}\n{}", current.trim_end(), text)
        };
        if combined.chars().count() > MAX_INSTRUCTIONS_CHARS {
            return bad("Custom instructions would exceed the size limit; ask the user to prune them.");
        }
        self.apply_settings(&json!({ INSTRUCTIONS_KEY: combined }), session, true)
    }

    async fn create_assistant(&self, spec: &Value, session: &str) -> anyhow::Result<ToolResult> {
        let manifest: AgentManifest = serde_json::from_value(spec.clone())
            .map_err(|e| anyhow::anyhow!("invalid assistant spec: {e}"))?;
        if manifest.id.trim().is_empty() || manifest.name.trim().is_empty() {
            return bad("assistant requires non-empty `id` and `name`.");
        }
        if agents_get(self.app.clone(), manifest.id.clone()).await.is_ok() {
            return bad("An assistant with this id exists; use update_assistant.");
        }
        if let Err(e) = app_changes::check_rate_limit(session) {
            return bad(e);
        }
        let info = agents_save(self.app.clone(), manifest)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let id = self.audit_manifest("assistant", &info.id, Value::Null, "created", session)?;
        self.notify("assistant");
        Ok(ToolResult::ok(format!(
            "Assistant '{}' ({}) created. change_id={id}",
            info.name, info.id
        )))
    }

    async fn update_assistant(&self, id: &str, patch: &Value, session: &str) -> anyhow::Result<ToolResult> {
        let old = agents_get(self.app.clone(), id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let old_v = serde_json::to_value(&old)?;
        let mut new_v = old_v.clone();
        catalog::merge(&mut new_v, patch);
        new_v["id"] = json!(id);
        let manifest: AgentManifest = serde_json::from_value(new_v.clone())
            .map_err(|e| anyhow::anyhow!("invalid assistant after update: {e}"))?;
        if let Err(e) = app_changes::check_rate_limit(session) {
            return bad(e);
        }
        agents_save(self.app.clone(), manifest)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        self.finish_manifest("assistant", id, old_v, new_v, session)
    }

    async fn delete_assistant(&self, id: &str, session: &str) -> anyhow::Result<ToolResult> {
        let old = agents_get(self.app.clone(), id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        if let Err(e) = app_changes::check_rate_limit(session) {
            return bad(e);
        }
        let old_v = serde_json::to_value(&old)?;
        agents_uninstall(self.app.clone(), id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let cid = self.audit_manifest("assistant", id, old_v, "deleted", session)?;
        self.notify("assistant");
        Ok(ToolResult::ok(format!("Assistant '{id}' deleted. change_id={cid} (rollback restores it)")))
    }

    async fn create_team(&self, spec: &Value, session: &str) -> anyhow::Result<ToolResult> {
        let manifest: TeamManifest = serde_json::from_value(spec.clone())
            .map_err(|e| anyhow::anyhow!("invalid team spec: {e}"))?;
        if manifest.id.trim().is_empty() || manifest.name.trim().is_empty() {
            return bad("team requires non-empty `id` and `name`.");
        }
        if teams_get(self.app.clone(), manifest.id.clone()).await.is_ok() {
            return bad("A team with this id exists; use update_team.");
        }
        if let Err(e) = app_changes::check_rate_limit(session) {
            return bad(e);
        }
        let info = teams_save(self.app.clone(), manifest)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let cid = self.audit_manifest("team", &info.id, Value::Null, "created", session)?;
        self.notify("team");
        Ok(ToolResult::ok(format!(
            "Team '{}' ({}) created with {} member(s). change_id={cid}",
            info.name,
            info.id,
            info.members.len()
        )))
    }

    async fn update_team(&self, id: &str, patch: &Value, session: &str) -> anyhow::Result<ToolResult> {
        let old = teams_get(self.app.clone(), id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let old_v = serde_json::to_value(&old)?;
        let mut new_v = old_v.clone();
        catalog::merge(&mut new_v, patch);
        new_v["id"] = json!(id);
        let manifest: TeamManifest = serde_json::from_value(new_v.clone())
            .map_err(|e| anyhow::anyhow!("invalid team after update: {e}"))?;
        if let Err(e) = app_changes::check_rate_limit(session) {
            return bad(e);
        }
        teams_save(self.app.clone(), manifest)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        self.finish_manifest("team", id, old_v, new_v, session)
    }

    async fn delete_team(&self, id: &str, session: &str) -> anyhow::Result<ToolResult> {
        let old = teams_get(self.app.clone(), id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        if let Err(e) = app_changes::check_rate_limit(session) {
            return bad(e);
        }
        let old_v = serde_json::to_value(&old)?;
        teams_uninstall(self.app.clone(), id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let cid = self.audit_manifest("team", id, old_v, "deleted", session)?;
        self.notify("team");
        Ok(ToolResult::ok(format!("Team '{id}' deleted. change_id={cid} (rollback restores it)")))
    }

    fn audit_manifest(
        &self,
        kind: &str,
        id: &str,
        before: Value,
        what: &str,
        session: &str,
    ) -> anyhow::Result<String> {
        let dir = self.config_dir()?;
        app_changes::record(
            &dir,
            ChangeRecord {
                session,
                kind,
                target: id,
                summary: format!("{kind} {id} {what}"),
                diff: vec![],
                before,
            },
        )
        .map_err(|e| anyhow::anyhow!(e))
    }

    fn finish_manifest(
        &self,
        kind: &str,
        id: &str,
        old: Value,
        new: Value,
        session: &str,
    ) -> anyhow::Result<ToolResult> {
        let mut diff = vec![];
        catalog::diff("", &old, &new, &mut diff);
        let dir = self.config_dir()?;
        let cid = app_changes::record(
            &dir,
            ChangeRecord {
                session,
                kind,
                target: id,
                summary: format!("{kind} {id} updated"),
                diff: diff.clone(),
                before: old,
            },
        )
        .map_err(|e| anyhow::anyhow!(e))?;
        self.notify(kind);
        Ok(ToolResult::ok(serde_json::to_string_pretty(&json!({
            "change_id": cid, "applied": diff
        }))?))
    }

    async fn list_assistants(&self) -> anyhow::Result<ToolResult> {
        let list = agents_list(self.app.clone()).await.map_err(|e| anyhow::anyhow!(e))?;
        Ok(ToolResult::ok(serde_json::to_string_pretty(&list)?))
    }

    async fn list_teams(&self) -> anyhow::Result<ToolResult> {
        let list = teams_list(self.app.clone()).await.map_err(|e| anyhow::anyhow!(e))?;
        Ok(ToolResult::ok(serde_json::to_string_pretty(&list)?))
    }
}

/// Blank out obvious secret fields before returning settings to the model.
fn redact_secrets(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if catalog::is_secret_key(k) {
                    if let Value::String(s) = v {
                        if !s.is_empty() {
                            *s = "***".to_string();
                        }
                    }
                } else {
                    redact_secrets(v);
                }
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(redact_secrets),
        _ => {}
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> &'a str {
    input.get(key).and_then(|v| v.as_str()).unwrap_or("").trim()
}

#[async_trait]
impl Tool for AppControlTool {
    fn name(&self) -> &str {
        "app_control"
    }

    fn description(&self) -> &str {
        "Manage the desktop app on the user's behalf: change settings, harness parameters, \
         assistants and teams, with every change audited and reversible.\n\
         \n\
         Actions:\n\
         - `get_settings`: current settings (secrets redacted).\n\
         - `describe_settings`: the catalog of fields you may change and their tier. Call this \
           before `update_settings`.\n\
         - `update_settings`: apply the partial `settings` object. Free-tier fields apply \
           directly; privileged fields (policy_mode, confirm_*, allow_outside_workspace, \
           providers, MCP servers, iteration/compaction limits, ...) ask the user every time. \
           Secrets and unlisted fields are locked. Returns a field-level diff and a change_id.\n\
         - `append_instructions`: append `text` to the user's custom instructions (append-only; \
           cannot override built-in safety rules).\n\
         - `list_assistants` / `list_teams`; `get_assistant` / `get_team` (by `id`).\n\
         - `create_assistant` / `create_team`: new definitions (fails if the id exists).\n\
         - `update_assistant` / `update_team`: deep-merge the `assistant` / `team` patch into \
           the definition `id`.\n\
         - `delete_assistant` / `delete_team`: remove definition `id`.\n\
         - `list_changes`: recent audited changes. `rollback`: undo a change by `change_id`.\n\
         \n\
         Writes are rate limited per session. Explain what you intend to change and why before \
         you call a privileged action."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "get_settings", "describe_settings", "update_settings",
                        "append_instructions",
                        "list_assistants", "get_assistant", "create_assistant",
                        "update_assistant", "delete_assistant",
                        "list_teams", "get_team", "create_team", "update_team", "delete_team",
                        "list_changes", "rollback"
                    ]
                },
                "settings": { "type": "object", "description": "For update_settings: partial settings." },
                "text": { "type": "string", "description": "For append_instructions." },
                "id": { "type": "string", "description": "Assistant/team id for get/update/delete." },
                "assistant": { "type": "object", "description": "Assistant manifest or patch." },
                "team": { "type": "object", "description": "Team manifest or patch." },
                "change_id": { "type": "string", "description": "For rollback." },
                "limit": { "type": "integer", "description": "For list_changes (default 20)." }
            },
            "required": ["action"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn needs_confirmation(&self, input: &Value) -> bool {
        match str_arg(input, "action") {
            "update_settings" => input
                .get("settings")
                .and_then(|s| s.as_object())
                .map(|o| o.keys().any(|k| catalog::tier_of(k) != Tier::Free))
                .unwrap_or(true),
            "append_instructions" | "update_assistant" | "delete_assistant" | "update_team"
            | "delete_team" | "rollback" => true,
            _ => false,
        }
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let session = ctx.session_id.as_str();
        let id = str_arg(&input, "id");
        let need_id = |what: &str| -> Option<ToolResult> {
            id.is_empty()
                .then(|| ToolResult::err(format!("`id` of the {what} is required.")))
        };
        match str_arg(&input, "action") {
            "get_settings" => self.get_settings().await,
            "describe_settings" => Ok(ToolResult::ok(serde_json::to_string_pretty(&catalog::describe())?)),
            "update_settings" => {
                let patch = input.get("settings").cloned().unwrap_or(Value::Null);
                self.apply_settings(&patch, session, false)
            }
            "append_instructions" => self.append_instructions(str_arg(&input, "text"), session).await,
            "list_assistants" => self.list_assistants().await,
            "list_teams" => self.list_teams().await,
            "get_assistant" => {
                if let Some(e) = need_id("assistant") {
                    return Ok(e);
                }
                let m = agents_get(self.app.clone(), id.to_string())
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                Ok(ToolResult::ok(serde_json::to_string_pretty(&m)?))
            }
            "get_team" => {
                if let Some(e) = need_id("team") {
                    return Ok(e);
                }
                let m = teams_get(self.app.clone(), id.to_string())
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                Ok(ToolResult::ok(serde_json::to_string_pretty(&m)?))
            }
            "create_assistant" => match input.get("assistant").filter(|v| v.is_object()) {
                Some(spec) => self.create_assistant(spec, session).await,
                None => bad("`assistant` object is required."),
            },
            "update_assistant" => match (need_id("assistant"), input.get("assistant")) {
                (Some(e), _) => Ok(e),
                (None, Some(p)) if p.is_object() => self.update_assistant(id, p, session).await,
                _ => bad("`assistant` patch object is required."),
            },
            "delete_assistant" => match need_id("assistant") {
                Some(e) => Ok(e),
                None => self.delete_assistant(id, session).await,
            },
            "create_team" => match input.get("team").filter(|v| v.is_object()) {
                Some(spec) => self.create_team(spec, session).await,
                None => bad("`team` object is required."),
            },
            "update_team" => match (need_id("team"), input.get("team")) {
                (Some(e), _) => Ok(e),
                (None, Some(p)) if p.is_object() => self.update_team(id, p, session).await,
                _ => bad("`team` patch object is required."),
            },
            "delete_team" => match need_id("team") {
                Some(e) => Ok(e),
                None => self.delete_team(id, session).await,
            },
            "list_changes" => {
                let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                let dir = self.config_dir()?;
                Ok(ToolResult::ok(serde_json::to_string_pretty(&app_changes::list(&dir, limit))?))
            }
            "rollback" => {
                let cid = str_arg(&input, "change_id");
                if cid.is_empty() {
                    return bad("`change_id` is required.");
                }
                if let Err(e) = app_changes::check_rate_limit(session) {
                    return bad(e);
                }
                match app_changes::rollback(&self.app, cid, session).await {
                    Ok(msg) => Ok(ToolResult::ok(msg)),
                    Err(e) => bad(e),
                }
            }
            other => bad(format!("Unknown action '{other}'. See the tool description for valid actions.")),
        }
    }
}
