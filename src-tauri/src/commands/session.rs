//! Chat session management — list / load / fork / delete / checkpoints.

use std::path::Path;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use piscis_kernel::store::db::{ChatMessage, Database, Session};

use crate::commands::data_scope::{open_project_kernel_state, ProjectDirParam};
use crate::runtime::task_activity::TaskActivity;
use crate::state::AppState;
use crate::commands::session_sources::{
    excluded_from_workz_task_list, is_codez_source, is_coordinator_boilerplate_title,
    is_generic_codez_title, is_generic_workz_title, is_workz_task_source, normalize_source,
    prompt_text_for_title, source_matches_filter, sources_compatible, workz_goal_text_for_title,
    SOURCE_LEGACY, SOURCE_WORKZ_TEAM,
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WorkzSessionMeta {
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub pool_id: Option<String>,
    /// Working directory chosen for this session (plain sessions).
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StateFrameEnvelope {
    #[serde(default)]
    agentz_workz: Option<WorkzSessionMeta>,
}

pub fn load_workz_meta(db: &Database, session_id: &str) -> Result<WorkzSessionMeta, String> {
    let raw = db
        .get_session_state_frame_json(session_id)
        .map_err(|e| format!("read state_frame failed: {e}"))?;
    let Some(text) = raw.filter(|s| !s.trim().is_empty()) else {
        return Ok(WorkzSessionMeta::default());
    };
    let frame: StateFrameEnvelope =
        serde_json::from_str(&text).map_err(|e| format!("parse state_frame failed: {e}"))?;
    Ok(frame.agentz_workz.unwrap_or_default())
}

pub fn persist_workz_meta(
    db: &Database,
    session_id: &str,
    team_id: Option<&str>,
    pool_id: Option<&str>,
) -> Result<(), String> {
    let mut meta = load_workz_meta(db, session_id)?;
    if let Some(t) = team_id.filter(|s| !s.trim().is_empty()) {
        meta.team_id = Some(t.to_string());
    }
    if let Some(p) = pool_id.filter(|s| !s.trim().is_empty()) {
        meta.pool_id = Some(p.to_string());
    }
    save_workz_meta(db, session_id, meta)
}

fn save_workz_meta(db: &Database, session_id: &str, meta: WorkzSessionMeta) -> Result<(), String> {
    let frame = StateFrameEnvelope {
        agentz_workz: Some(meta),
    };
    let json = serde_json::to_string(&frame).map_err(|e| format!("serialize state_frame: {e}"))?;
    db.update_session_state_frame_json(session_id, Some(&json))
        .map_err(|e| format!("write state_frame failed: {e}"))
}

/// Reject continuing a session under the wrong mode namespace (or team).
pub fn validate_session_continuation(
    db: &Database,
    session_id: &str,
    expected_source: &str,
    team_id: Option<&str>,
) -> Result<(), String> {
    let existing = db
        .get_session(session_id)
        .map_err(|e| format!("get_session failed: {e}"))?
        .ok_or_else(|| format!("session '{session_id}' not found"))?;
    if !sources_compatible(expected_source, &existing.source) {
        return Err(format!(
            "session '{session_id}' belongs to '{}' but this turn expects '{}'",
            existing.source, expected_source
        ));
    }
    if normalize_source(expected_source) == SOURCE_WORKZ_TEAM {
        let meta = load_workz_meta(db, session_id)?;
        if let Some(bound) = meta.team_id.as_deref().filter(|s| !s.is_empty()) {
            let want = team_id.filter(|s| !s.is_empty()).unwrap_or("");
            if !want.is_empty() && bound != want {
                return Err(format!(
                    "session '{session_id}' is bound to team '{bound}', not '{want}'"
                ));
            }
        }
    }
    Ok(())
}

/// Lightweight session row for the UI sidebar.
#[derive(Debug, Serialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: Option<String>,
    pub status: String,
    pub message_count: i64,
    pub updated_at: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

fn truncate_title_line(text: &str, max_chars: usize) -> String {
    let line = text.trim().lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return String::new();
    }
    let n = line.chars().count();
    if n <= max_chars {
        line.to_string()
    } else {
        format!("{}…", line.chars().take(max_chars).collect::<String>())
    }
}

/// After the first user message, replace generic titles with the user's question.
pub fn maybe_autotitle_session_from_first_prompt(
    db: &Database,
    session_id: &str,
    user_text: &str,
) -> Result<(), String> {
    let session = db
        .get_session(session_id)
        .map_err(|e| format!("get_session failed: {e}"))?
        .ok_or_else(|| format!("session '{session_id}' not found"))?;
    if session.message_count > 1 {
        return Ok(());
    }

    let goal = if is_workz_task_source(&session.source) {
        if !is_generic_workz_title(session.title.as_deref()) {
            return Ok(());
        }
        workz_goal_text_for_title(user_text)
    } else if is_codez_source(&session.source) {
        if !is_generic_codez_title(session.title.as_deref()) {
            return Ok(());
        }
        prompt_text_for_title(user_text)
    } else {
        return Ok(());
    };
    let title = truncate_title_line(&goal, 80);
    if title.is_empty() {
        return Ok(());
    }
    db.rename_session(session_id, &title)
        .map_err(|e| format!("rename_session failed: {e}"))
}

/// Back-compat alias for WorkZ call sites.
pub fn maybe_autotitle_workz_session(
    db: &Database,
    session_id: &str,
    user_text: &str,
) -> Result<(), String> {
    maybe_autotitle_session_from_first_prompt(db, session_id, user_text)
}

/// User + assistant text for the first chat round (CodeZ / WorkZ task sessions).
pub fn first_chat_round_for_title(db: &Database, session_id: &str) -> Option<(String, String)> {
    let session = db.get_session(session_id).ok()??;
    if !is_codez_source(&session.source) && !is_workz_task_source(&session.source) {
        return None;
    }
    let msgs = db.get_messages_latest(session_id, 32).ok()?;
    let mut users = msgs
        .iter()
        .filter(|m| m.role == "user" && !m.content.trim().is_empty())
        .collect::<Vec<_>>();
    let assistants = msgs
        .iter()
        .filter(|m| m.role == "assistant" && !m.content.trim().is_empty())
        .collect::<Vec<_>>();
    if users.len() != 1 || assistants.is_empty() {
        return None;
    }
    let raw_user = users.remove(0).content.clone();
    let user = if is_workz_task_source(&session.source) {
        workz_goal_text_for_title(&raw_user)
    } else {
        prompt_text_for_title(&raw_user)
    };
    if user.is_empty() {
        return None;
    }
    let assistant = assistants.last()?.content.clone();
    Some((user, assistant))
}

/// Normalize a one-line title from the flash model.
pub fn sanitize_llm_session_title(raw: &str) -> String {
    let mut line = raw
        .trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '「' || c == '」' || c == '『' || c == '』')
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if line.to_ascii_lowercase().starts_with("title:") {
        line = line[6..].trim().to_string();
    }
    truncate_title_line(&line, 48)
}

/// Sidebar title for CodeZ rows when the stored title is still generic.
fn resolve_codez_list_title(db: &Database, session: &Session) -> Option<String> {
    if !is_generic_codez_title(session.title.as_deref()) {
        return session.title.clone();
    }
    let msgs = db.get_messages(&session.id, 8, 0).ok().unwrap_or_default();
    let first_user = msgs.into_iter().find(|m| m.role == "user")?;
    let title = truncate_title_line(&prompt_text_for_title(&first_user.content), 80);
    if title.is_empty() {
        return session.title.clone();
    }
    Some(title)
}

/// Sidebar title for a WorkZ task row — never the coordinator system preamble.
fn resolve_workz_task_list_title(db: &Database, session: &Session) -> Option<String> {
    if !is_generic_workz_title(session.title.as_deref())
        && !is_coordinator_boilerplate_title(session.title.as_deref())
    {
        return session.title.clone();
    }
    let msgs = db.get_messages(&session.id, 8, 0).ok().unwrap_or_default();
    let first_user = msgs.into_iter().find(|m| m.role == "user")?;
    let goal = workz_goal_text_for_title(&first_user.content);
    let title = truncate_title_line(&goal, 80);
    if title.is_empty() {
        return session.title.clone();
    }
    Some(title)
}

/// A single persisted chat message for history reload.
#[derive(Debug, Serialize)]
pub struct MessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
    /// Tool calls made while producing this (assistant) bubble, in call order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDto>,
}

/// A persisted tool call + its result, reconstructed from the message rows so a
/// reloaded / error-interrupted session still shows what the agent did.
#[derive(Debug, Serialize)]
pub struct ToolDto {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub result: Option<String>,
    pub is_error: bool,
    /// Char count of the bubble text preceding this call (interleave position).
    pub text_offset: usize,
}

/// Upper bound for a persisted tool result sent to the UI (chars).
const UI_TOOL_RESULT_MAX_CHARS: usize = 20_000;

/// Build UI bubbles (with tool calls) from raw rows: tool-result rows are folded
/// into the matching call, consecutive assistant rows of one turn are merged.
/// Prompts the agent kernel writes into history as `user` rows to steer the
/// model (plan-todo reminders, tool-args correction). They are for the LLM
/// only and must not render as user bubbles.
fn is_kernel_injected_prompt(content: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "⚠️ 你的计划中还有未完成的步骤",
        "系统提示：你上一次的工具调用参数",
    ];
    let t = content.trim_start();
    PREFIXES.iter().any(|p| t.starts_with(p))
}

fn messages_rich(msgs: Vec<ChatMessage>) -> Vec<MessageDto> {
    let mut out: Vec<MessageDto> = Vec::new();
    for m in msgs {
        if m.role != "user" && m.role != "assistant" {
            continue;
        }
        if m.role == "user" && is_kernel_injected_prompt(&m.content) {
            continue;
        }
        if m.role == "user" && m.content.trim().is_empty() {
            // Tool-result carrier row: attach results to the calls they answer.
            if let Some(raw) = m.tool_results_json.as_deref() {
                if let Ok(serde_json::Value::Array(items)) = serde_json::from_str(raw) {
                    for item in items {
                        let Some(id) = item.get("tool_use_id").and_then(|v| v.as_str()) else {
                            continue;
                        };
                        let content = item
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        let is_error = item
                            .get("is_error")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        for bubble in out.iter_mut().rev() {
                            if let Some(t) = bubble.tools.iter_mut().find(|t| t.id == id) {
                                let truncated: String =
                                    content.chars().take(UI_TOOL_RESULT_MAX_CHARS).collect();
                                t.result = Some(truncated);
                                t.is_error = is_error;
                                break;
                            }
                        }
                    }
                }
            }
            continue;
        }
        let calls: Vec<serde_json::Value> = m
            .tool_calls_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_default();
        if m.role == "assistant" && m.content.trim().is_empty() && calls.is_empty() {
            continue;
        }
        let merge =
            m.role == "assistant" && out.last().map(|l| l.role == "assistant").unwrap_or(false);
        if !merge {
            out.push(MessageDto {
                id: m.id.clone(),
                role: m.role.clone(),
                content: String::new(),
                tools: Vec::new(),
            });
        }
        let bubble = out.last_mut().expect("bubble pushed above");
        if !m.content.trim().is_empty() {
            if !bubble.content.is_empty() {
                bubble.content.push_str("\n\n");
            }
            bubble.content.push_str(&m.content);
        }
        if merge {
            // Anchor checkpoint/fork to the last chunk in the merged turn.
            bubble.id = m.id.clone();
        }
        let offset = bubble.content.chars().count();
        for call in calls {
            let (Some(id), Some(name)) = (
                call.get("id").and_then(|v| v.as_str()),
                call.get("name").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            bubble.tools.push(ToolDto {
                id: id.to_string(),
                name: name.to_string(),
                input: call
                    .get("input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                result: None,
                is_error: false,
                text_offset: offset,
            });
        }
    }
    out
}

/// Open the project-local session DB.
async fn with_db<T>(
    app: &AppHandle,
    project: ProjectDirParam,
    f: impl FnOnce(&Database) -> Result<T, String>,
) -> Result<T, String> {
    let project_dir = project.required()?;
    let (db, _settings) = open_project_kernel_state(app, &project_dir)?;
    let guard = db.lock().await;
    f(&guard)
}

/// Persist and return a consistent sidebar status. A `running` row with no live
/// task gate is treated as aborted (process kill, early turn exit) and healed.
fn reconcile_listed_status(
    db: &Database,
    activity: &TaskActivity,
    project: &Path,
    session_id: &str,
    status: &str,
) -> String {
    if status != "running" {
        return status.to_string();
    }
    if activity.is_active(project, session_id) {
        return "running".into();
    }
    match db.update_session_status(session_id, "idle") {
        Ok(()) => {
            tracing::info!(
                session_id,
                "healed stale session status running -> idle (no live turn)"
            );
        }
        Err(e) => {
            tracing::warn!(
                session_id,
                "failed to heal stale running session status: {e}"
            );
        }
    }
    "idle".into()
}

#[cfg(test)]
mod reconcile_tests {
    use super::reconcile_listed_status;
    use crate::runtime::task_activity::TaskActivity;
    use piscis_kernel::store::db::Database;

    #[tokio::test]
    async fn heals_running_without_live_gate_and_keeps_live_running() {
        let root = std::env::temp_dir().join(format!(
            "agentz-session-reconcile-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let db = Database::open_in_memory().unwrap();
        let live = db
            .create_session_with_source(Some("live"), "workz")
            .unwrap();
        let stale = db
            .create_session_with_source(Some("stale"), "workz")
            .unwrap();
        db.update_session_status(&live.id, "running").unwrap();
        db.update_session_status(&stale.id, "running").unwrap();

        let activity = TaskActivity::default();
        let _gate = activity.try_enter(&root, &live.id).unwrap();

        assert_eq!(
            reconcile_listed_status(&db, &activity, &root, &live.id, "running"),
            "running"
        );
        assert_eq!(
            reconcile_listed_status(&db, &activity, &root, &stale.id, "running"),
            "idle"
        );
        assert_eq!(
            db.list_sessions(10, 0)
                .unwrap()
                .into_iter()
                .find(|s| s.id == stale.id)
                .unwrap()
                .status,
            "idle"
        );
        assert_eq!(
            reconcile_listed_status(&db, &activity, &root, "idle-row", "idle"),
            "idle"
        );
        drop(_gate);
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn messages_raw_chronological(db: &Database, session_id: &str) -> Result<Vec<ChatMessage>, String> {
    db.get_messages_latest(session_id, 1000)
        .map_err(|e| format!("get_messages failed: {e}"))
}

fn copy_messages_to_session(
    db: &Database,
    source_id: &str,
    dest_id: &str,
    up_to_message_id: Option<&str>,
) -> Result<(), String> {
    let msgs = messages_raw_chronological(db, source_id)?;
    if msgs.is_empty() {
        return Ok(());
    }
    if let Some(target) = up_to_message_id {
        let mut found = false;
        for m in msgs {
            db.append_message(dest_id, &m.role, &m.content)
                .map_err(|e| format!("copy message failed: {e}"))?;
            if m.id == target {
                found = true;
                break;
            }
        }
        if !found {
            return Err(format!(
                "checkpoint message '{target}' not found in session"
            ));
        }
    } else {
        for m in msgs {
            db.append_message(dest_id, &m.role, &m.content)
                .map_err(|e| format!("copy message failed: {e}"))?;
        }
    }
    Ok(())
}

/// List recent chat sessions, newest first. When `sources` is non-empty, only
/// sessions whose `source` column matches one of the tags are returned (legacy
/// `agentz` rows count as CodeZ). Koi `pool` sessions are always excluded from
/// WorkZ task lists. Optional `team_id` narrows `workz-team` rows to one team.
#[tauri::command]
pub async fn chat_list_sessions(
    app: AppHandle,
    project_dir: Option<String>,
    sources: Option<Vec<String>>,
    team_id: Option<String>,
) -> Result<Vec<SessionMeta>, String> {
    let filter: Vec<String> = sources
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let team_filter = team_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let workz_listing = !filter.is_empty() && filter.iter().any(|s| is_workz_task_source(s));
    let project = ProjectDirParam { project_dir };
    let project_path = project.required()?;
    let activity = app.state::<AppState>().task_activity.clone();
    with_db(
        &app,
        ProjectDirParam {
            project_dir: Some(project_path.clone()),
        },
        move |db| {
            let sessions = db
                .list_sessions(200, 0)
                .map_err(|e| format!("list_sessions failed: {e}"))?;
            let project = Path::new(&project_path);
            Ok(sessions
                .into_iter()
                .filter(|s| {
                    if filter.is_empty() {
                        return true;
                    }
                    if !source_matches_filter(&s.source, &filter) {
                        return false;
                    }
                    if workz_listing && excluded_from_workz_task_list(&s.source) {
                        return false;
                    }
                    if let Some(ref want_team) = team_filter {
                        if normalize_source(&s.source) == SOURCE_WORKZ_TEAM {
                            let meta = load_workz_meta(db, &s.id).unwrap_or_default();
                            match meta.team_id.as_deref().filter(|t| !t.is_empty()) {
                                Some(bound) if bound != want_team.as_str() => return false,
                                None => {}
                                _ => {}
                            }
                        }
                    }
                    true
                })
                .map(|s| {
                    let meta = load_workz_meta(db, &s.id).unwrap_or_default();
                    let title = if is_workz_task_source(&s.source) {
                        resolve_workz_task_list_title(db, &s)
                    } else if is_codez_source(&s.source) {
                        resolve_codez_list_title(db, &s)
                    } else {
                        s.title.clone()
                    };
                    let status =
                        reconcile_listed_status(db, &activity, project, &s.id, &s.status);
                    SessionMeta {
                        id: s.id,
                        title,
                        status,
                        message_count: s.message_count,
                        updated_at: s.updated_at.to_rfc3339(),
                        source: s.source,
                        team_id: meta.team_id,
                        pool_id: meta.pool_id,
                        cwd: meta.cwd,
                    }
                })
                .collect())
        },
    )
    .await
}

/// Load a session's messages in chronological order.
#[tauri::command]
pub async fn chat_get_messages(
    app: AppHandle,
    session_id: String,
    project_dir: Option<String>,
) -> Result<Vec<MessageDto>, String> {
    with_db(&app, ProjectDirParam { project_dir }, |db| {
        Ok(messages_rich(messages_raw_chronological(db, &session_id)?))
    })
    .await
}

/// One page of history, newest-first paging (see [`chat_get_messages_page`]).
#[derive(Debug, Serialize)]
pub struct MessagePageDto {
    /// Chronological (oldest → newest) within the page.
    pub messages: Vec<MessageDto>,
    /// Pass back as `offset` to fetch the next-older page.
    pub next_offset: i64,
    pub has_more: bool,
}

/// Default page size in raw DB rows (tool-result rows count too).
const HISTORY_PAGE_DEFAULT: i64 = 120;

/// Load history newest-first in pages: `offset` = raw rows already consumed from
/// the newest end. Each page is returned oldest → newest so the UI can prepend it.
/// A page is trimmed at its old edge to start on a user message, so an assistant
/// turn is never split across two pages (it would render as two bubbles).
#[tauri::command]
pub async fn chat_get_messages_page(
    app: AppHandle,
    session_id: String,
    project_dir: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<MessagePageDto, String> {
    with_db(&app, ProjectDirParam { project_dir }, |db| {
        let limit = limit.unwrap_or(HISTORY_PAGE_DEFAULT).clamp(10, 500);
        let offset = offset.unwrap_or(0).max(0);
        let mut raw = db
            .get_messages_older(&session_id, limit, offset)
            .map_err(|e| format!("get_messages failed: {e}"))?;
        let fetched = raw.len() as i64;
        let mut has_more = fetched == limit;
        if has_more {
            if let Ok(Some(sess)) = db.get_session(&session_id) {
                has_more = offset + fetched < sess.message_count;
            }
        }
        if has_more {
            // Start the page on a real user message; the trimmed older rows are
            // picked up (whole) by the next page because `next_offset` excludes them.
            if let Some(k) = raw.iter().position(|m| {
                m.role == "user"
                    && !m.content.trim().is_empty()
                    && !is_kernel_injected_prompt(&m.content)
            }) {
                if k > 0 {
                    raw.drain(..k);
                }
            }
        }
        let consumed = raw.len() as i64;
        let messages = messages_rich(raw);
        Ok(MessagePageDto {
            messages,
            next_offset: offset + consumed,
            has_more,
        })
    })
    .await
}

/// Strip leading `(N) ` prefixes and trailing ` (fork)` / ` (fork @ checkpoint)` suffixes
/// from a session title, returning `(root_title, first_leading_number, had_fork_suffix)`.
fn parse_fork_title(raw: &str) -> (&str, Option<u32>, bool) {
    let mut s = raw.trim();
    let mut had_suffix = false;
    loop {
        if let Some(rest) = s.strip_suffix("(fork @ checkpoint)") {
            s = rest.trim_end();
            had_suffix = true;
        } else if let Some(rest) = s.strip_suffix("(fork)") {
            s = rest.trim_end();
            had_suffix = true;
        } else {
            break;
        }
    }
    let mut first_num: Option<u32> = None;
    while let Some(after_open) = s.strip_prefix('(') {
        let Some((digits, after_close)) = after_open.split_once(')') else {
            break;
        };
        let Ok(n) = digits.parse::<u32>() else {
            break;
        };
        if n == 0 {
            break;
        }
        let rest = after_close.trim_start();
        if rest.is_empty() {
            break;
        }
        if first_num.is_none() {
            first_num = Some(n);
        }
        s = rest;
    }
    let root = if s.is_empty() { "Chat" } else { s };
    (root, first_num, had_suffix)
}

/// Build the next numbered fork title `(N) <root> (fork [@ checkpoint])` by scanning
/// existing session titles that share the same root title.
fn next_fork_title<I, S>(existing_titles: I, source_title: &str, at_checkpoint: bool) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let (root, _, _) = parse_fork_title(source_title);
    let mut max_num: u32 = 0;
    for item in existing_titles {
        let (other_root, num, had_suffix) = parse_fork_title(item.as_ref());
        if other_root == root {
            if let Some(n) = num {
                max_num = max_num.max(n);
            } else if had_suffix {
                max_num = max_num.max(1);
            }
        }
    }
    let next = max_num + 1;
    let suffix = if at_checkpoint {
        "(fork @ checkpoint)"
    } else {
        "(fork)"
    };
    format!("({next}) {root} {suffix}")
}

/// Fork a session — optionally copy only up to a checkpoint message (inclusive).
#[tauri::command]
pub async fn chat_fork_session(
    app: AppHandle,
    session_id: String,
    title: Option<String>,
    up_to_message_id: Option<String>,
    project_dir: Option<String>,
) -> Result<SessionMeta, String> {
    with_db(&app, ProjectDirParam { project_dir }, |db| {
        let source = db
            .get_session(&session_id)
            .map_err(|e| format!("get_session failed: {e}"))?
            .ok_or_else(|| format!("session '{session_id}' not found"))?;

        let fork_title = title.unwrap_or_else(|| {
            let resolved_base = if is_workz_task_source(&source.source) {
                resolve_workz_task_list_title(db, &source)
            } else if is_codez_source(&source.source) {
                resolve_codez_list_title(db, &source)
            } else {
                source.title.clone()
            }
            .unwrap_or_else(|| "Chat".to_string());
            let existing = db.list_sessions(500, 0).unwrap_or_default();
            let existing_titles = existing.iter().filter_map(|s| s.title.as_deref());
            next_fork_title(
                existing_titles,
                &resolved_base,
                up_to_message_id.is_some(),
            )
        });
        let fork_source = if source.source == SOURCE_LEGACY {
            normalize_source(&source.source).to_string()
        } else {
            source.source.clone()
        };
        let created = db
            .create_session_with_source(Some(&fork_title), &fork_source)
            .map_err(|e| format!("create_session failed: {e}"))?;

        copy_messages_to_session(db, &session_id, &created.id, up_to_message_id.as_deref())?;

        let refreshed = db
            .get_session(&created.id)
            .map_err(|e| format!("get_session failed: {e}"))?
            .unwrap_or(created);
        let meta = load_workz_meta(db, &refreshed.id).unwrap_or_default();
        Ok(SessionMeta {
            id: refreshed.id,
            title: refreshed.title,
            status: refreshed.status,
            message_count: refreshed.message_count,
            updated_at: refreshed.updated_at.to_rfc3339(),
            source: refreshed.source,
            team_id: meta.team_id,
            pool_id: meta.pool_id,
            cwd: meta.cwd,
        })
    })
    .await
}

pub const SESSION_EXPORT_FORMAT: &str = "agentz-session";
pub const SESSION_EXPORT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_results_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_index: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments_json: Option<String>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedSession {
    pub format: String,
    pub version: u32,
    pub title: Option<String>,
    pub source: String,
    /// Project root on the exporting machine; used to rebase absolute paths on import.
    #[serde(default)]
    pub project_dir: Option<String>,
    pub exported_at: String,
    pub messages: Vec<ExportedMessage>,
}

pub fn export_session_data(
    db: &Database,
    session_id: &str,
    project_dir: Option<&str>,
) -> Result<ExportedSession, String> {
    let session = db
        .get_session(session_id)
        .map_err(|e| format!("get_session failed: {e}"))?
        .ok_or_else(|| format!("session '{session_id}' not found"))?;
    let title = if is_codez_source(&session.source) {
        resolve_codez_list_title(db, &session).or(session.title.clone())
    } else {
        session.title.clone()
    };
    let mut messages = Vec::new();
    let page = 500;
    let mut offset = 0;
    loop {
        let batch = db
            .get_messages(session_id, page, offset)
            .map_err(|e| format!("get_messages failed: {e}"))?;
        let n = batch.len() as i64;
        messages.extend(batch.into_iter().map(|m| ExportedMessage {
            role: m.role,
            content: m.content,
            tool_calls_json: m.tool_calls_json,
            tool_results_json: m.tool_results_json,
            turn_index: m.turn_index,
            attachments_json: m.attachments_json,
            created_at: m.created_at.to_rfc3339(),
        }));
        if n < page {
            break;
        }
        offset += n;
    }
    Ok(ExportedSession {
        format: SESSION_EXPORT_FORMAT.to_string(),
        version: SESSION_EXPORT_VERSION,
        title,
        source: normalize_source(&session.source).to_string(),
        project_dir: project_dir.map(str::to_string),
        exported_at: chrono::Utc::now().to_rfc3339(),
        messages,
    })
}

/// Variants of a path as they may appear in stored text: native, forward-slash,
/// and JSON-escaped backslashes. Longest first so escaped forms win.
fn path_spellings(p: &str) -> [String; 3] {
    let trimmed = p.trim_end_matches(['/', '\\']);
    let back = trimmed.replace('/', "\\");
    [back.replace('\\', "\\\\"), back, trimmed.replace('\\', "/")]
}

pub fn rebase_project_paths(text: &str, from: &str, to: &str) -> String {
    if from.trim().is_empty() || to.trim().is_empty() {
        return text.to_string();
    }
    let froms = path_spellings(from);
    let mut tos = path_spellings(to);
    if !to.contains('\\') && !to.contains(':') {
        // POSIX target: every source spelling maps to the plain forward-slash path.
        tos = [tos[2].clone(), tos[2].clone(), tos[2].clone()];
    }
    let mut out = text.to_string();
    for (f, t) in froms.iter().zip(tos.iter()) {
        if f != t && out.contains(f.as_str()) {
            out = out.replace(f.as_str(), t);
        }
    }
    out
}

pub fn import_session_data(
    db: &Database,
    data: &ExportedSession,
    project_dir: Option<&str>,
) -> Result<Session, String> {
    if data.format != SESSION_EXPORT_FORMAT {
        return Err(format!("not an AgentZ session file (format '{}')", data.format));
    }
    if data.version > SESSION_EXPORT_VERSION {
        return Err(format!(
            "session file version {} is newer than supported ({SESSION_EXPORT_VERSION}); please upgrade AgentZ",
            data.version
        ));
    }
    let rebase = |s: &str| match (data.project_dir.as_deref(), project_dir) {
        (Some(from), Some(to)) => rebase_project_paths(s, from, to),
        _ => s.to_string(),
    };
    let title = data.title.clone().unwrap_or_else(|| "Imported".to_string());
    let created = db
        .create_session_with_source(Some(&title), normalize_source(&data.source))
        .map_err(|e| format!("create_session failed: {e}"))?;
    for m in &data.messages {
        db.append_message_full_with_attachments(
            &created.id,
            &m.role,
            &rebase(&m.content),
            m.tool_calls_json.as_deref().map(|s| rebase(s)).as_deref(),
            m.tool_results_json.as_deref().map(|s| rebase(s)).as_deref(),
            m.turn_index,
            m.attachments_json.as_deref().map(|s| rebase(s)).as_deref(),
        )
        .map_err(|e| format!("append_message failed: {e}"))?;
    }
    Ok(db
        .get_session(&created.id)
        .map_err(|e| format!("get_session failed: {e}"))?
        .unwrap_or(created))
}

/// Export a session to a portable JSON file so it can be resumed on another machine.
#[tauri::command]
pub async fn chat_export_session(
    app: AppHandle,
    session_id: String,
    dest_path: String,
    project_dir: Option<String>,
) -> Result<usize, String> {
    let pd = project_dir.clone();
    let data = with_db(&app, ProjectDirParam { project_dir }, move |db| {
        export_session_data(db, &session_id, pd.as_deref())
    })
    .await?;
    let count = data.messages.len();
    let json = serde_json::to_string_pretty(&data).map_err(|e| format!("serialize failed: {e}"))?;
    let dest = Path::new(&dest_path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir failed: {e}"))?;
    }
    std::fs::write(dest, json).map_err(|e| format!("write {dest_path} failed: {e}"))?;
    Ok(count)
}

/// Import a session file written by `chat_export_session` as a new session.
#[tauri::command]
pub async fn chat_import_session(
    app: AppHandle,
    src_path: String,
    project_dir: Option<String>,
) -> Result<SessionMeta, String> {
    let raw = std::fs::read_to_string(&src_path).map_err(|e| format!("read {src_path} failed: {e}"))?;
    let data: ExportedSession = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("invalid session file: {e}"))?;
    let pd = project_dir.clone();
    with_db(&app, ProjectDirParam { project_dir }, move |db| {
        let s = import_session_data(db, &data, pd.as_deref())?;
        Ok(SessionMeta {
            id: s.id,
            title: s.title,
            status: s.status,
            message_count: s.message_count,
            updated_at: s.updated_at.to_rfc3339(),
            source: s.source,
            team_id: None,
            pool_id: None,
            cwd: None,
        })
    })
    .await
}

/// Remember the working directory of a (plain) session. `None` clears it.
#[tauri::command]
pub async fn chat_set_session_cwd(
    app: AppHandle,
    project_dir: Option<String>,
    session_id: String,
    cwd: Option<String>,
) -> Result<(), String> {
    let cwd = cwd.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    if let Some(c) = &cwd {
        if !std::path::Path::new(c).is_dir() {
            return Err(format!("directory not found: {c}"));
        }
    }
    with_db(&app, ProjectDirParam { project_dir }, move |db| {
        let mut meta = load_workz_meta(db, &session_id)?;
        meta.cwd = cwd;
        save_workz_meta(db, &session_id, meta)
    })
    .await
}

/// Restore session to a checkpoint — deletes all messages after the given
/// message. When `restore_files` is true, also rolls back the file edits made
/// by the most recent agent turn that touched the workspace (best-effort), so
/// restoring a checkpoint reverts both the conversation *and* the changes the
/// agent applied afterwards.
#[tauri::command]
pub async fn chat_restore_checkpoint(
    app: AppHandle,
    session_id: String,
    message_id: String,
    project_dir: Option<String>,
    restore_files: Option<bool>,
) -> Result<Vec<String>, String> {
    let project = ProjectDirParam {
        project_dir: project_dir.clone(),
    }
    .required()?;

    with_db(&app, ProjectDirParam { project_dir }, |db| {
        let rowid: i64 = db
            .conn
            .query_row(
                "SELECT rowid FROM messages WHERE id = ?1 AND session_id = ?2",
                params![message_id, session_id],
                |r| r.get(0),
            )
            .map_err(|e| format!("checkpoint message not found: {e}"))?;

        db.conn
            .execute(
                "DELETE FROM messages WHERE session_id = ?1 AND rowid > ?2",
                params![session_id, rowid],
            )
            .map_err(|e| format!("delete messages failed: {e}"))?;

        db.recompute_session_message_count(&session_id)
            .map_err(|e| format!("recompute message count failed: {e}"))?;
        Ok(())
    })
    .await?;

    if !restore_files.unwrap_or(false) {
        return Ok(Vec::new());
    }

    // Roll back the latest turn's file edits via the project journal.
    let journal = crate::journal::open_project_journal(&project)?;
    match journal
        .latest_turn_with_changes(&session_id)
        .map_err(|e| e.to_string())?
    {
        Some(turn_id) => journal
            .undo_turn(&session_id, &turn_id)
            .map_err(|e| e.to_string()),
        None => Ok(Vec::new()),
    }
}

/// Delete a session and its messages.
#[tauri::command]
pub async fn chat_delete_session(
    app: AppHandle,
    session_id: String,
    project_dir: Option<String>,
) -> Result<(), String> {
    // Deleting a session terminates its background tasks and drops any parked
    // continuation, so a deleted session never resurrects via auto-resume.
    if let Some(state) = app.try_state::<crate::state::AppState>() {
        state.bg_hub.cancel_pending_for_session(&session_id);
        state.background_runs.kill_session(&session_id);
    }
    with_db(&app, ProjectDirParam { project_dir }, |db| {
        db.delete_session(&session_id)
            .map_err(|e| format!("delete_session failed: {e}"))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_import_round_trip_rebases_project_paths() {
        let a = Database::open_in_memory().unwrap();
        let s = a.create_session_with_source(Some("修复 bug"), "codez").unwrap();
        a.append_message(&s.id, "user", r"look at C:\work\proj\src\main.rs").unwrap();
        a.append_message_full(
            &s.id,
            "assistant",
            "ok",
            Some(r#"[{"input":{"path":"C:\\work\\proj\\src\\main.rs"}}]"#),
            None,
            Some(1),
        )
        .unwrap();

        let data = export_session_data(&a, &s.id, Some(r"C:\work\proj")).unwrap();
        let json = serde_json::to_string(&data).unwrap();
        let back: ExportedSession = serde_json::from_str(&json).unwrap();

        let b = Database::open_in_memory().unwrap();
        let imported = import_session_data(&b, &back, Some("/home/me/proj")).unwrap();
        assert_eq!(imported.title.as_deref(), Some("修复 bug"));
        let msgs = b.get_messages(&imported.id, 10, 0).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, r"look at /home/me/proj\src\main.rs");
        assert_eq!(
            msgs[1].tool_calls_json.as_deref(),
            Some(r#"[{"input":{"path":"/home/me/proj\\src\\main.rs"}}]"#)
        );
        assert_eq!(msgs[1].turn_index, Some(1));
    }

    #[test]
    fn import_rejects_foreign_format() {
        let db = Database::open_in_memory().unwrap();
        let data = ExportedSession {
            format: "other".into(),
            version: 1,
            title: None,
            source: "codez".into(),
            project_dir: None,
            exported_at: String::new(),
            messages: vec![],
        };
        assert!(import_session_data(&db, &data, None).is_err());
    }

    #[test]
    fn fork_title_numbers_increment_and_strip_nested_affixes() {
        let existing: Vec<&str> = vec!["实现 OAuth 登录"];
        let first = next_fork_title(&existing, "实现 OAuth 登录", true);
        assert_eq!(first, "(1) 实现 OAuth 登录 (fork @ checkpoint)");

        let existing = vec!["实现 OAuth 登录", &first];
        let second = next_fork_title(&existing, "实现 OAuth 登录", true);
        assert_eq!(second, "(2) 实现 OAuth 登录 (fork @ checkpoint)");

        // Forking from an already-forked session strips both `(1)` and `(fork @ checkpoint)`
        // instead of nesting `(1) (1) ... (fork @ checkpoint) (fork @ checkpoint)`.
        let existing = vec!["实现 OAuth 登录", &first, &second];
        let third_from_fork = next_fork_title(&existing, &first, false);
        assert_eq!(third_from_fork, "(3) 实现 OAuth 登录 (fork)");

        // Legacy unnumbered forks count as `(1)` so the next fork becomes `(2)`.
        let legacy = vec!["修复路径 (fork @ checkpoint)"];
        assert_eq!(
            next_fork_title(&legacy, "修复路径 (fork @ checkpoint)", true),
            "(2) 修复路径 (fork @ checkpoint)"
        );
    }
}
