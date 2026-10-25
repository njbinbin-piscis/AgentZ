//! BotZ rooms — isolated team workspaces.
//!
//! A room binds a display name to a working directory and its own member list
//! (any bots, optionally seeded from a team template). The room's pool lives in that directory's project DB, so rooms in different
//! directories never see each other's todos, messages or sessions. The registry
//! itself is global: `{config}/botz/rooms.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use piscis_core::models::KoiTodo;
use piscis_kernel::pool::git::MergeOutcome;

use crate::commands::data_scope::{open_project_kernel_state, resolve_global_config_dir};
use crate::commands::teams::{
    is_room_team_id, save_team_manifest, sync_pool_from_manifest, TeamManifest, ROOM_TEAM_PREFIX,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BotzRoom {
    pub id: String,
    pub name: String,
    pub dir: String,
    pub team_id: String,
    #[serde(default)]
    pub pool_id: Option<String>,
    /// Coordinator session the user talks to in this room.
    #[serde(default)]
    pub coordinator_session_id: Option<String>,
    #[serde(default)]
    pub created_at: String,
    /// Member agent ids. The room owns them; `team_id` points at a private
    /// manifest regenerated from these (legacy rooms point at a template team
    /// and get converted on first open).
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub org_spec: String,
}

fn rooms_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_global_config_dir(app)?.join("botz").join("rooms.json"))
}

pub fn load_rooms(path: &Path) -> Vec<BotzRoom> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

fn store_rooms(path: &Path, rooms: &[BotzRoom]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir failed: {e}"))?;
    }
    let json = serde_json::to_string_pretty(rooms).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("write rooms failed: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace rooms failed: {e}"))
}

fn norm_dir(d: &str) -> String {
    let s = d.trim().replace('\\', "/");
    let s = s.trim_end_matches('/').to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// Insert or update `room`. Two rooms may not share a directory with the same
/// team, because they would resolve to the same pool and stop being isolated.
pub fn upsert_room(rooms: &mut Vec<BotzRoom>, mut room: BotzRoom) -> Result<BotzRoom, String> {
    room.name = room.name.trim().to_string();
    room.dir = room.dir.trim().to_string();
    if room.name.is_empty() || room.dir.is_empty() {
        return Err("room name and directory are required".into());
    }
    if crate::remote::is_remote(&room.dir) {
        return Err("rooms need a local directory; remote projects are not supported yet".into());
    }
    if room.id.trim().is_empty() {
        room.id = uuid::Uuid::new_v4().to_string();
    }
    let mut seen = std::collections::HashSet::new();
    room.members.retain(|m| !m.trim().is_empty() && seen.insert(m.clone()));
    if room.team_id.trim().is_empty() {
        room.team_id = room_team_id(&room.id);
    }
    if room.created_at.is_empty() {
        room.created_at = chrono::Utc::now().to_rfc3339();
    }
    let key = norm_dir(&room.dir);
    if let Some(other) = rooms
        .iter()
        .find(|r| r.id != room.id && norm_dir(&r.dir) == key && r.team_id == room.team_id)
    {
        return Err(format!(
            "room '{}' already runs this team in {}",
            other.name, other.dir
        ));
    }
    match rooms.iter_mut().find(|r| r.id == room.id) {
        Some(slot) => *slot = room.clone(),
        None => rooms.push(room.clone()),
    }
    Ok(room)
}

#[tauri::command]
pub async fn botz_list_rooms(app: AppHandle) -> Result<Vec<BotzRoom>, String> {
    Ok(load_rooms(&rooms_file(&app)?))
}

#[tauri::command]
pub async fn botz_save_room(app: AppHandle, room: BotzRoom) -> Result<BotzRoom, String> {
    let path = rooms_file(&app)?;
    let mut rooms = load_rooms(&path);
    let saved = upsert_room(&mut rooms, room)?;
    store_rooms(&path, &rooms)?;
    Ok(saved)
}

/// Remove a room from the registry. Its pool, sessions and files stay in the
/// directory's project DB.
#[tauri::command]
pub async fn botz_delete_room(app: AppHandle, id: String) -> Result<(), String> {
    let path = rooms_file(&app)?;
    let mut rooms = load_rooms(&path);
    rooms.retain(|r| r.id != id);
    store_rooms(&path, &rooms)
}

pub fn room_team_id(room_id: &str) -> String {
    let short: String = room_id.chars().filter(|c| c.is_ascii_alphanumeric()).take(12).collect();
    format!("{}{}", ROOM_TEAM_PREFIX, short.to_lowercase())
}

const DEFAULT_ROOM_ORG_SPEC: &str = "Members share one working directory. The coordinator \
splits the user's request into todos and assigns each to the best-fitting member. Members \
finish their own todo, submit it for review, and message teammates when they depend on \
each other's output.";

/// Fold a legacy template-backed room into a room-owned member list: members
/// and org_spec are copied from the template the first time.
pub fn adopt_template(room: &mut BotzRoom, template: Option<&TeamManifest>) {
    if is_room_team_id(&room.team_id) {
        return;
    }
    if let Some(t) = template {
        if room.members.is_empty() {
            room.members = t.members.clone();
        }
        if room.org_spec.trim().is_empty() {
            room.org_spec = t.org_spec.clone();
        }
    }
    room.team_id = room_team_id(&room.id);
}

pub fn room_manifest(room: &BotzRoom) -> TeamManifest {
    TeamManifest {
        id: room.team_id.clone(),
        name: room.name.clone(),
        description: String::new(),
        mode: "swarm".into(),
        org_spec: if room.org_spec.trim().is_empty() {
            DEFAULT_ROOM_ORG_SPEC.into()
        } else {
            room.org_spec.clone()
        },
        members: room.members.clone(),
        workflow_hint: "waves".into(),
        workflow: None,
        task_timeout_secs: 0,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RoomSync {
    pub room: BotzRoom,
    pub pool_id: String,
    pub member_koi_ids: Vec<String>,
    /// Members asked to leave who still own active todos; they stay until done.
    pub kept_busy: Vec<String>,
}

async fn sync_room(app: &AppHandle, mut room: BotzRoom) -> Result<RoomSync, String> {
    if !is_room_team_id(&room.team_id) {
        let template = TeamManifest::load_by_id(app, &room.team_id).ok();
        adopt_template(&mut room, template.as_ref());
    }
    let manifest = room_manifest(&room);
    save_team_manifest(app, &manifest)?;
    let synced =
        sync_pool_from_manifest(app, &room.dir, &manifest, room.pool_id.as_deref(), true).await?;
    room.pool_id = Some(synced.created.pool_id.clone());

    let path = rooms_file(app)?;
    let mut rooms = load_rooms(&path);
    let room = upsert_room(&mut rooms, room)?;
    store_rooms(&path, &rooms)?;
    emit_board_changed(app);
    Ok(RoomSync {
        room,
        pool_id: synced.created.pool_id,
        member_koi_ids: synced.created.member_koi_ids,
        kept_busy: synced.kept_busy,
    })
}

/// Make sure the room's pool exists and matches its member list.
#[tauri::command]
pub async fn botz_open_room(app: AppHandle, id: String) -> Result<RoomSync, String> {
    let room = load_rooms(&rooms_file(&app)?)
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("room {id} not found"))?;
    sync_room(&app, room).await
}

#[tauri::command]
pub async fn botz_set_room_members(
    app: AppHandle,
    id: String,
    members: Vec<String>,
    org_spec: Option<String>,
) -> Result<RoomSync, String> {
    let mut room = load_rooms(&rooms_file(&app)?)
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("room {id} not found"))?;
    if !is_room_team_id(&room.team_id) {
        let template = TeamManifest::load_by_id(&app, &room.team_id).ok();
        adopt_template(&mut room, template.as_ref());
    }
    room.members = members;
    if let Some(spec) = org_spec {
        room.org_spec = spec;
    }
    sync_room(&app, room).await
}

// ─── Bot profiles (memory, routines, budget) ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BotRoutine {
    pub id: String,
    pub name: String,
    pub prompt: String,
    /// `interval` (every `minutes`) or `daily` (at local `time` "HH:MM").
    pub kind: String,
    #[serde(default)]
    pub minutes: Option<u32>,
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub last_run_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BotProfile {
    pub id: String,
    /// Long-lived notes injected into every turn of the bot's home thread.
    #[serde(default)]
    pub memory: String,
    #[serde(default)]
    pub routines: Vec<BotRoutine>,
    /// Daily token cap for the home thread and routines; 0 = unlimited.
    #[serde(default)]
    pub daily_token_budget: u64,
    #[serde(default)]
    pub usage_date: String,
    #[serde(default)]
    pub usage_tokens: u64,
    /// Session of the bot's persistent home thread (in `botz_home_dir`).
    #[serde(default)]
    pub home_session_id: Option<String>,
}

fn bots_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_global_config_dir(app)?.join("botz").join("bots"))
}

fn bot_file(dir: &Path, id: &str) -> PathBuf {
    let safe: String = id
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    dir.join(format!("{safe}.json"))
}

pub fn load_bot(dir: &Path, id: &str) -> BotProfile {
    std::fs::read_to_string(bot_file(dir, id))
        .ok()
        .and_then(|raw| serde_json::from_str::<BotProfile>(raw.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_else(|| BotProfile { id: id.to_string(), ..Default::default() })
}

#[tauri::command]
pub async fn botz_get_bot(app: AppHandle, id: String) -> Result<BotProfile, String> {
    Ok(load_bot(&bots_dir(&app)?, &id))
}

#[tauri::command]
pub async fn botz_list_bots(app: AppHandle) -> Result<Vec<BotProfile>, String> {
    let dir = bots_dir(&app)?;
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    out.push(load_bot(&dir, stem));
                }
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn botz_save_bot(app: AppHandle, profile: BotProfile) -> Result<BotProfile, String> {
    if profile.id.trim().is_empty() {
        return Err("bot id is required".into());
    }
    let dir = bots_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create dir failed: {e}"))?;
    let path = bot_file(&dir, &profile.id);
    let json = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("write bot failed: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("replace bot failed: {e}"))?;
    Ok(profile)
}

/// Directory whose project DB holds every bot's home thread.
#[tauri::command]
pub async fn botz_home_dir(app: AppHandle) -> Result<String, String> {
    let dir = resolve_global_config_dir(&app)?.join("botz").join("home");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create dir failed: {e}"))?;
    Ok(dir.to_string_lossy().into_owned())
}

// ─── Room board, direct assignment and review ────────────────────────────────

const USER_SENDER: &str = "user";

/// Every todo of a pool, including `needs_review` / `done` (the WorkZ board only
/// shows active ones).
#[tauri::command]
pub async fn botz_room_todos(
    app: AppHandle,
    project_dir: String,
    pool_id: String,
) -> Result<Vec<KoiTodo>, String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let db = db.lock().await;
    let mut all = db.list_koi_todos(None).map_err(|e| e.to_string())?;
    all.retain(|t| t.pool_session_id.as_deref() == Some(pool_id.as_str()));
    all.sort_by_key(|t| t.created_at);
    Ok(all)
}

/// Statuses the board may set by drag and drop.
pub fn board_status_allowed(status: &str) -> bool {
    matches!(status, "todo" | "blocked" | "needs_review" | "done" | "cancelled")
}

/// Move a todo between board columns. Moving back to `todo` re-queues it for its
/// owner; patrol dispatches it on the next (immediate) round.
#[tauri::command]
pub async fn botz_set_todo_status(
    app: AppHandle,
    project_dir: String,
    todo_id: String,
    status: String,
) -> Result<(), String> {
    if !board_status_allowed(&status) {
        return Err(format!("status '{status}' cannot be set from the board"));
    }
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    {
        let db = db.lock().await;
        let todo = db
            .get_koi_todo(&todo_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("todo '{todo_id}' not found"))?;
        if todo.status == "in_progress" {
            return Err("this todo is running; stop it before moving it".into());
        }
        if status == "done" {
            db.complete_koi_todo(&todo_id, None).map_err(|e| e.to_string())?;
        } else {
            db.update_koi_todo(&todo_id, None, None, Some(&status), None)
                .map_err(|e| e.to_string())?;
        }
        if let Some(pool) = todo.pool_session_id.as_deref() {
            let _ = db.insert_pool_message_ext(
                pool,
                USER_SENDER,
                &format!("moved \"{}\" to {status}", todo.title),
                "status",
                "{}",
                Some(&todo_id),
                None,
                Some("todo_moved"),
            );
        }
    }
    emit_board_changed(&app);
    crate::runtime::patrol::kick_pool_patrols();
    Ok(())
}

/// Give a member work directly: a todo owned by that member, picked up by patrol.
#[tauri::command]
pub async fn botz_assign_member(
    app: AppHandle,
    project_dir: String,
    pool_id: String,
    koi_id: String,
    text: String,
) -> Result<KoiTodo, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("message is empty".into());
    }
    let title: String = text.lines().next().unwrap_or(text).chars().take(80).collect();
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let todo = {
        let db = db.lock().await;
        let todo = db
            .create_koi_todo(&koi_id, &title, text, "medium", USER_SENDER, Some(&pool_id), "user", None, 0)
            .map_err(|e| e.to_string())?;
        let _ = db.insert_pool_message_ext(
            &pool_id,
            USER_SENDER,
            &format!("@{koi_id} {text}"),
            "text",
            "{}",
            Some(&todo.id),
            None,
            Some("user_assignment"),
        );
        todo
    };
    crate::runtime::patrol::ensure_pool_patrol(&app, &project_dir);
    emit_board_changed(&app);
    crate::runtime::patrol::kick_pool_patrols();
    Ok(todo)
}

#[derive(Debug, Serialize)]
pub struct TodoDiff {
    pub branch: Option<String>,
    pub base: String,
    pub stat: String,
    pub diff: String,
    pub truncated: bool,
}

const MAX_DIFF_BYTES: usize = 400_000;

/// Unified diff of a member's `koi/*` branch against the room's checked-out HEAD.
#[tauri::command]
pub async fn botz_todo_diff(
    app: AppHandle,
    project_dir: String,
    todo_id: String,
) -> Result<TodoDiff, String> {
    let branch = {
        let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
        let db = db.lock().await;
        db.get_koi_todo(&todo_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("todo '{todo_id}' not found"))?
            .git_branch
            .filter(|b| !b.trim().is_empty())
    };
    let dir = Path::new(&project_dir);
    let Some(branch) = branch else {
        return Ok(TodoDiff {
            branch: None,
            base: "HEAD".into(),
            stat: String::new(),
            diff: String::new(),
            truncated: false,
        });
    };
    let range = format!("HEAD...{branch}");
    let stat = run_git(dir, &["diff", "--stat", &range]).await?;
    let mut diff = run_git(dir, &["diff", "--no-color", &range]).await?;
    let truncated = diff.len() > MAX_DIFF_BYTES;
    if truncated {
        let mut cut = MAX_DIFF_BYTES;
        while !diff.is_char_boundary(cut) {
            cut -= 1;
        }
        diff.truncate(cut);
    }
    Ok(TodoDiff { branch: Some(branch), base: "HEAD".into(), stat, diff, truncated })
}

#[derive(Debug, Serialize)]
pub struct ReviewOutcome {
    pub status: String,
    pub merged: bool,
    pub message: String,
}

/// Approve (complete + merge the member branch) or send the todo back with feedback.
#[tauri::command]
pub async fn botz_review_todo(
    app: AppHandle,
    project_dir: String,
    todo_id: String,
    approve: bool,
    feedback: Option<String>,
) -> Result<ReviewOutcome, String> {
    let (db, _s) = open_project_kernel_state(&app, &project_dir)?;
    let todo = {
        let db = db.lock().await;
        db.get_koi_todo(&todo_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("todo '{todo_id}' not found"))?
    };
    if todo.status == "in_progress" {
        return Err("this todo is still running".into());
    }
    let feedback = feedback.map(|f| f.trim().to_string()).filter(|f| !f.is_empty());
    let pool = todo.pool_session_id.clone();

    let outcome = if approve {
        let mut merged = false;
        let mut message = "approved".to_string();
        if let Some(branch) = todo.git_branch.as_deref().filter(|b| !b.trim().is_empty()) {
            let results = piscis_kernel::pool::git::merge_koi_branches(Path::new(&project_dir), None, Some(branch))
                .await
                .map_err(|e| e.to_string())?;
            let db = db.lock().await;
            match results.first().map(|r| &r.outcome) {
                Some(MergeOutcome::Merged) => {
                    merged = true;
                    message = format!("merged {branch}");
                    let _ = db.set_koi_todo_integration_status(&todo_id, piscis_core::integration::INTEGRATION_MERGED);
                }
                Some(MergeOutcome::Conflict { message: m }) | Some(MergeOutcome::Error { message: m }) => {
                    let _ = db.set_koi_todo_integration_status(&todo_id, piscis_core::integration::INTEGRATION_CONFLICT);
                    return Err(format!("merge of {branch} failed and was aborted: {m}"));
                }
                None => message = format!("branch {branch} not found; marked done without merging"),
            }
        }
        let db = db.lock().await;
        db.complete_koi_todo(&todo_id, None).map_err(|e| e.to_string())?;
        if merged {
            let _ = db.set_koi_todo_integration_status(&todo_id, piscis_core::integration::INTEGRATION_MERGED);
        }
        ReviewOutcome { status: "done".into(), merged, message }
    } else {
        let note = feedback.clone().unwrap_or_else(|| "Changes requested.".to_string());
        let description = format!("{}\n\n## Review feedback\n{}", todo.description.trim(), note);
        let db = db.lock().await;
        db.update_koi_todo(&todo_id, None, Some(description.trim()), Some("todo"), None)
            .map_err(|e| e.to_string())?;
        ReviewOutcome { status: "todo".into(), merged: false, message: "sent back with feedback".into() }
    };

    if let Some(pool) = pool.as_deref() {
        let db = db.lock().await;
        let text = match (&feedback, approve) {
            (_, true) => format!("approved \"{}\" ({})", todo.title, outcome.message),
            (Some(f), false) => format!("@{} changes requested on \"{}\": {f}", todo.owner_id, todo.title),
            (None, false) => format!("@{} changes requested on \"{}\"", todo.owner_id, todo.title),
        };
        let _ = db.insert_pool_message_ext(pool, USER_SENDER, &text, "review", "{}", Some(&todo_id), None, Some("todo_reviewed"));
    }
    emit_board_changed(&app);
    crate::runtime::patrol::kick_pool_patrols();
    Ok(outcome)
}

fn emit_board_changed(app: &AppHandle) {
    use tauri::Emitter;
    let _ = app.emit(
        crate::runtime::koi::POOL_EVENT_CHANNEL,
        serde_json::json!({ "kind": "board_changed" }),
    );
}

async fn run_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        piscis_kernel::proc::tokio_command("git")
            .args(crate::commands::ide::GIT_UNQUOTED_PATHS)
            .args(args)
            .current_dir(dir)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "git timed out".to_string())?
    .map_err(|e| format!("git failed: {e}"))?;
    if !out.status.success() {
        return Err(format!("git error: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(name: &str, dir: &str, team: &str) -> BotzRoom {
        BotzRoom {
            id: String::new(),
            name: name.into(),
            dir: dir.into(),
            team_id: team.into(),
            pool_id: None,
            coordinator_session_id: None,
            created_at: String::new(),
            members: Vec::new(),
            org_spec: String::new(),
        }
    }

    fn template(members: &[&str]) -> TeamManifest {
        let mut r = room("T", "/w", "");
        r.team_id = "tpl".into();
        r.members = members.iter().map(|s| s.to_string()).collect();
        r.org_spec = "tpl spec".into();
        room_manifest(&r)
    }

    #[test]
    fn new_room_without_team_gets_private_team_and_dedups_members() {
        let mut rooms = Vec::new();
        let mut r = room("A", "/w/a", "");
        r.members = vec!["x".into(), "x".into(), " ".into(), "y".into()];
        let saved = upsert_room(&mut rooms, r).unwrap();
        assert!(is_room_team_id(&saved.team_id));
        assert_eq!(saved.members, vec!["x", "y"]);
        assert_eq!(room_manifest(&saved).members, vec!["x", "y"]);
        assert!(!room_manifest(&saved).org_spec.is_empty());
    }

    #[test]
    fn legacy_room_adopts_template_once() {
        let mut r = room("A", "/w/a", "tpl");
        r.id = "abc-123".into();
        adopt_template(&mut r, Some(&template(&["x", "y"])));
        assert_eq!(r.team_id, room_team_id("abc-123"));
        assert_eq!(r.members, vec!["x", "y"]);
        assert_eq!(r.org_spec, "tpl spec");
        r.members = vec!["z".into()];
        adopt_template(&mut r, Some(&template(&["x"])));
        assert_eq!(r.members, vec!["z"]);
    }

    #[test]
    fn upsert_assigns_id_and_updates_in_place() {
        let mut rooms = Vec::new();
        let a = upsert_room(&mut rooms, room("A", "/w/a", "t")).unwrap();
        assert!(!a.id.is_empty());
        let mut renamed = a.clone();
        renamed.name = "A2".into();
        renamed.pool_id = Some("p".into());
        upsert_room(&mut rooms, renamed).unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].name, "A2");
        assert_eq!(rooms[0].pool_id.as_deref(), Some("p"));
    }

    #[test]
    fn same_dir_and_team_is_rejected_other_team_allowed() {
        let mut rooms = Vec::new();
        upsert_room(&mut rooms, room("A", "/w/a/", "t")).unwrap();
        assert!(upsert_room(&mut rooms, room("B", "/w/a", "t")).is_err());
        assert!(upsert_room(&mut rooms, room("C", "/w/a", "t2")).is_ok());
        assert!(upsert_room(&mut rooms, room("", "/w/b", "t")).is_err());
        assert!(upsert_room(&mut rooms, room("R", "agentz-remote://ssh-remote+box/w", "t")).is_err());
    }

    #[test]
    fn store_and_load_round_trip() {
        let base = std::env::temp_dir().join(format!("agentz-botz-{}", uuid::Uuid::new_v4()));
        let path = base.join("botz").join("rooms.json");
        let mut rooms = Vec::new();
        upsert_room(&mut rooms, room("办公室", "/w/a", "t")).unwrap();
        store_rooms(&path, &rooms).unwrap();
        assert_eq!(load_rooms(&path), rooms);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn board_only_sets_safe_statuses() {
        assert!(board_status_allowed("todo"));
        assert!(board_status_allowed("done"));
        assert!(!board_status_allowed("in_progress"));
        assert!(!board_status_allowed("weird"));
    }

    #[test]
    fn bot_profile_defaults_when_missing_and_sanitizes_file_name() {
        let base = std::env::temp_dir().join(format!("agentz-bots-{}", uuid::Uuid::new_v4()));
        let p = load_bot(&base, "a/b");
        assert_eq!(p.id, "a/b");
        assert!(p.routines.is_empty());
        assert_eq!(bot_file(&base, "a/b"), base.join("a_b.json"));
    }
}
