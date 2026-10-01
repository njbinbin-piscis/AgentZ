//! Audit trail for agent-made configuration changes (`app_control`): an
//! append-only log, pre-change snapshots for rollback, a per-session write
//! rate limit, and the Tauri commands behind the settings "change log" panel.

use piscis_kernel::store::settings::Settings;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

use crate::commands::agents::{agents_get, agents_save, agents_uninstall, AgentManifest};
use crate::commands::data_scope::resolve_global_config_dir;
use crate::commands::teams::{teams_get, teams_save, teams_uninstall, TeamManifest};

pub const APP_CONTROL_UPDATED_EVENT: &str = "agentz:app-control-updated";

const MAX_WRITES: usize = 12;
const WINDOW: Duration = Duration::from_secs(600);

fn audit_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("audit")
}

fn log_path(config_dir: &Path) -> PathBuf {
    audit_dir(config_dir).join("changes.jsonl")
}

fn snapshot_path(config_dir: &Path, id: &str) -> PathBuf {
    audit_dir(config_dir)
        .join("snapshots")
        .join(format!("{id}.json"))
}

fn new_id() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{ms}-{}", SEQ.fetch_add(1, Ordering::Relaxed))
}

/// Max agent writes per session within a sliding window.
pub fn check_rate_limit(session_id: &str) -> Result<(), String> {
    static HITS: once_cell::sync::Lazy<Mutex<HashMap<String, Vec<Instant>>>> =
        once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));
    let mut map = HITS.lock().map_err(|_| "rate limiter poisoned".to_string())?;
    let hits = map.entry(session_id.to_string()).or_default();
    let now = Instant::now();
    hits.retain(|t| now.duration_since(*t) < WINDOW);
    if hits.len() >= MAX_WRITES {
        return Err(format!(
            "Too many configuration changes in this session (limit {MAX_WRITES} per {} min). \
             Ask the user before continuing.",
            WINDOW.as_secs() / 60
        ));
    }
    hits.push(now);
    Ok(())
}

pub struct ChangeRecord<'a> {
    pub session: &'a str,
    /// settings | assistant | team
    pub kind: &'a str,
    pub target: &'a str,
    pub summary: String,
    pub diff: Vec<Value>,
    /// Data needed to undo: settings -> {field: old value}; manifests -> old manifest or null.
    pub before: Value,
}

/// Persist snapshot + log line. Returns the change id.
pub fn record(config_dir: &Path, rec: ChangeRecord<'_>) -> Result<String, String> {
    let id = new_id();
    let snap = snapshot_path(config_dir, &id);
    std::fs::create_dir_all(snap.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&snap, serde_json::to_vec_pretty(&rec.before).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = json!({
        "id": id, "ts": ts, "session": rec.session, "kind": rec.kind,
        "target": rec.target, "summary": rec.summary, "diff": rec.diff,
    });
    append_line(config_dir, &line)?;
    Ok(id)
}

fn append_line(config_dir: &Path, line: &Value) -> Result<(), String> {
    use std::io::Write;
    let path = log_path(config_dir);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| e.to_string())
}

/// Newest first. Each entry gets `rolled_back` set when a later rollback entry references it.
pub fn list(config_dir: &Path, limit: usize) -> Vec<Value> {
    let text = std::fs::read_to_string(log_path(config_dir)).unwrap_or_default();
    let all: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let undone: std::collections::HashSet<String> = all
        .iter()
        .filter(|e| e["kind"] == "rollback")
        .filter_map(|e| e["of"].as_str().map(str::to_string))
        .collect();
    all.into_iter()
        .rev()
        .filter(|e| e["kind"] != "rollback")
        .take(limit)
        .map(|mut e| {
            let done = e["id"].as_str().map(|i| undone.contains(i)).unwrap_or(false);
            e["rolled_back"] = json!(done);
            e
        })
        .collect()
}

fn find(config_dir: &Path, id: &str) -> Option<Value> {
    list(config_dir, usize::MAX)
        .into_iter()
        .find(|e| e["id"] == id)
}

/// Undo one change using its snapshot. Records a rollback entry.
pub async fn rollback(app: &AppHandle, id: &str, session: &str) -> Result<String, String> {
    let config_dir = resolve_global_config_dir(app)?;
    let entry = find(&config_dir, id).ok_or_else(|| format!("change '{id}' not found"))?;
    if entry["rolled_back"] == true {
        return Err("this change was already rolled back".into());
    }
    let before: Value = std::fs::read(snapshot_path(&config_dir, id))
        .map_err(|e| format!("snapshot missing: {e}"))
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))?;
    let kind = entry["kind"].as_str().unwrap_or("");
    let target = entry["target"].as_str().unwrap_or("").to_string();
    match kind {
        "settings" => {
            let path = config_dir.join("config.json");
            let current = Settings::load(&path).map_err(|e| e.to_string())?;
            let mut value = serde_json::to_value(&current).map_err(|e| e.to_string())?;
            if let (Some(obj), Some(old)) = (value.as_object_mut(), before.as_object()) {
                for (k, v) in old {
                    obj.insert(k.clone(), v.clone());
                }
            }
            let mut restored: Settings = serde_json::from_value(value).map_err(|e| e.to_string())?;
            restored.config_path = path;
            restored.save().map_err(|e| e.to_string())?;
        }
        "assistant" => {
            if before.is_null() {
                agents_uninstall(app.clone(), target.clone()).await?;
            } else {
                let m: AgentManifest = serde_json::from_value(before).map_err(|e| e.to_string())?;
                agents_save(app.clone(), m).await?;
            }
        }
        "team" => {
            if before.is_null() {
                teams_uninstall(app.clone(), target.clone()).await?;
            } else {
                let m: TeamManifest = serde_json::from_value(before).map_err(|e| e.to_string())?;
                teams_save(app.clone(), m).await?;
            }
        }
        other => return Err(format!("cannot roll back change kind '{other}'")),
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    append_line(
        &config_dir,
        &json!({ "id": new_id(), "ts": ts, "session": session, "kind": "rollback", "of": id }),
    )?;
    let _ = app.emit(APP_CONTROL_UPDATED_EVENT, json!({ "kind": kind }));
    Ok(format!("Rolled back {kind} change {id}."))
}

pub async fn current_manifest_json(app: &AppHandle, kind: &str, id: &str) -> Value {
    match kind {
        "assistant" => agents_get(app.clone(), id.to_string())
            .await
            .ok()
            .and_then(|m| serde_json::to_value(m).ok())
            .unwrap_or(Value::Null),
        _ => teams_get(app.clone(), id.to_string())
            .await
            .ok()
            .and_then(|m| serde_json::to_value(m).ok())
            .unwrap_or(Value::Null),
    }
}

#[tauri::command]
pub async fn app_changes_list(app: AppHandle, limit: Option<usize>) -> Result<Vec<Value>, String> {
    let dir = resolve_global_config_dir(&app)?;
    Ok(list(&dir, limit.unwrap_or(100)))
}

#[tauri::command]
pub async fn app_changes_rollback(app: AppHandle, id: String) -> Result<String, String> {
    rollback(&app, &id, "user").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_list_and_rate_limit() {
        let dir = std::env::temp_dir().join(format!("agentz-audit-{}", new_id()));
        let id = record(
            &dir,
            ChangeRecord {
                session: "s",
                kind: "settings",
                target: "language",
                summary: "x".into(),
                diff: vec![json!({"path":"language","old":"en","new":"zh"})],
                before: json!({"language":"en"}),
            },
        )
        .unwrap();
        let items = list(&dir, 10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], id.as_str());
        assert_eq!(items[0]["rolled_back"], false);
        append_line(&dir, &json!({"id":"r","kind":"rollback","of":id})).unwrap();
        assert_eq!(list(&dir, 10)[0]["rolled_back"], true);
        let _ = std::fs::remove_dir_all(&dir);

        let sid = format!("rl-{}", new_id());
        for _ in 0..MAX_WRITES {
            assert!(check_rate_limit(&sid).is_ok());
        }
        assert!(check_rate_limit(&sid).is_err());
    }
}
