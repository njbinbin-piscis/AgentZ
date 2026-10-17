//! Audit trail for agent-made configuration changes (`app_control`): an
//! append-only log, pre-change snapshots for rollback, a per-session write
//! rate limit, and the Tauri commands behind the settings "change log" panel.

use piscis_kernel::security::secrets::SecretStore;
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
use crate::tools::settings_catalog::is_secret_key;

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
    let mut map = HITS
        .lock()
        .map_err(|_| "rate limiter poisoned".to_string())?;
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
    std::fs::write(&snap, encode_snapshot(config_dir, &id, &rec.before)?)
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

fn encode_snapshot(config_dir: &Path, id: &str, before: &Value) -> Result<Vec<u8>, String> {
    let store = SecretStore::new(config_dir).map_err(|error| error.to_string())?;
    let payload = json!({"id": id, "before": before});
    let ciphertext = store
        .encrypt_hex(&payload.to_string())
        .map_err(|error| error.to_string())?;
    serde_json::to_vec(&json!({
        "agentz_audit_snapshot_format": 1,
        "ciphertext": ciphertext,
    }))
    .map_err(|error| error.to_string())
}

fn decode_snapshot(config_dir: &Path, id: &str, bytes: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if value.get("agentz_audit_snapshot_format").is_none() {
        // Historical snapshots were plaintext; retain rollback compatibility.
        return Ok(value);
    }
    if value["agentz_audit_snapshot_format"] != 1 {
        return Err("unsupported audit snapshot format".into());
    }
    let ciphertext = value["ciphertext"]
        .as_str()
        .ok_or_else(|| "audit snapshot ciphertext missing".to_string())?;
    let store = SecretStore::new(config_dir).map_err(|error| error.to_string())?;
    let plaintext = store
        .decrypt_hex(ciphertext)
        .map_err(|error| error.to_string())?;
    let payload: Value = serde_json::from_str(&plaintext).map_err(|error| error.to_string())?;
    if payload["id"] != id {
        return Err("audit snapshot identity mismatch".into());
    }
    payload
        .get("before")
        .cloned()
        .ok_or_else(|| "audit snapshot payload missing".to_string())
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

fn redact_audit_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if is_secret_key(key)
                    || matches!(
                        key.to_ascii_lowercase().as_str(),
                        "mcp_servers" | "env" | "headers" | "credentials" | "auth"
                    )
                {
                    *child = json!("***");
                } else {
                    redact_audit_value(child);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact_audit_value),
        _ => {}
    }
}

fn redact_audit_diff(entry: &mut Value) {
    let Some(diffs) = entry.get_mut("diff").and_then(Value::as_array_mut) else {
        return;
    };
    for diff in diffs {
        let secret_path = diff
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| {
                path.split(['.', '[', ']']).any(|part| {
                    is_secret_key(part)
                        || matches!(
                            part,
                            "mcp_servers" | "env" | "headers" | "credentials" | "auth"
                        )
                })
            });
        for field in ["old", "new"] {
            if let Some(value) = diff.get_mut(field) {
                if secret_path {
                    *value = json!("***");
                } else {
                    redact_audit_value(value);
                }
            }
        }
    }
}

#[cfg(test)]
mod secret_audit_tests {
    use super::*;

    #[test]
    fn legacy_plaintext_snapshot_restores_into_encrypted_settings_v2() {
        let directory = std::env::temp_dir().join(format!("agentz-audit-rollback-{}", new_id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("config.json");
        let marker = "AZ031_LEGACY_ROLLBACK_SECRET_MARKER";
        let mut previous = Settings {
            config_path: path.clone(),
            ..Settings::default()
        };
        previous
            .mcp_servers
            .push(piscis_kernel::store::settings::McpServerConfig {
                name: "fixture".into(),
                command: marker.into(),
                ..Default::default()
            });
        let before = json!({"mcp_servers": previous.mcp_servers});
        let legacy = serde_json::to_vec(&before).unwrap();
        let decoded = decode_snapshot(&directory, "old-change", &legacy).unwrap();
        let current = Settings {
            config_path: path.clone(),
            ..Settings::default()
        };
        current.save().unwrap();
        let current_bytes = std::fs::read(&path).unwrap();
        assert!(restore_settings_snapshot(&path, &Value::Null).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), current_bytes);
        restore_settings_snapshot(&path, &decoded).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.mcp_servers[0].command, marker);
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("\"agentz_secret_format\": 2"));
        assert!(!saved.contains(marker));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn snapshots_are_encrypted_and_bound_to_change_id() {
        let directory = std::env::temp_dir().join(format!("agentz-audit-secret-{}", new_id()));
        std::fs::create_dir(&directory).unwrap();
        let marker = "AZ031_AUDIT_SNAPSHOT_SECRET_MARKER";
        let before = json!({"mcp_servers": [{"env": {"TOKEN": marker}}]});
        let encoded = encode_snapshot(&directory, "change-one", &before).unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains(marker));
        assert_eq!(
            decode_snapshot(&directory, "change-one", &encoded).unwrap(),
            before
        );
        assert!(decode_snapshot(&directory, "change-two", &encoded).is_err());
        assert_eq!(
            decode_snapshot(&directory, "legacy", &serde_json::to_vec(&before).unwrap()).unwrap(),
            before
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn historical_audit_diff_is_masked_on_read() {
        let marker = "AZ031_HISTORICAL_AUDIT_SYNTHETIC_SECRET";
        let mut entry = json!({"diff": [
            {"path":"mcp_servers[old].url", "old":marker, "new":marker},
            {"path":"llm_providers[p]", "old":{"api_key":marker}, "new":{"api_key":marker}}
        ]});
        redact_audit_diff(&mut entry);
        assert!(!entry.to_string().contains(marker));
        assert_eq!(entry["diff"][0]["old"], "***");
        assert_eq!(entry["diff"][1]["new"]["api_key"], "***");
    }
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
            let done = e["id"]
                .as_str()
                .map(|i| undone.contains(i))
                .unwrap_or(false);
            e["rolled_back"] = json!(done);
            redact_audit_diff(&mut e);
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
        .and_then(|bytes| decode_snapshot(&config_dir, id, &bytes))?;
    let kind = entry["kind"].as_str().unwrap_or("");
    let target = entry["target"].as_str().unwrap_or("").to_string();
    match kind {
        "settings" => {
            let _write_guard = crate::commands::settings::settings_write_guard().await;
            let path = config_dir.join("config.json");
            restore_settings_snapshot(&path, &before)?;
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

fn restore_settings_snapshot(path: &Path, before: &Value) -> Result<(), String> {
    let current = Settings::load(path).map_err(|error| error.to_string())?;
    let mut value = serde_json::to_value(&current).map_err(|error| error.to_string())?;
    let old = before
        .as_object()
        .ok_or_else(|| "settings snapshot must be an object".to_string())?;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "settings shape must be an object".to_string())?;
    for (key, field) in old {
        obj.insert(key.clone(), field.clone());
    }
    let mut restored: Settings =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    restored.config_path = path.to_path_buf();
    restored.save().map_err(|error| error.to_string())
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
