//! Diagnostic correlation and redacted log export.
//!
//! Settings secret keys use `tools::settings_catalog::is_secret_key`, the same
//! predicate `app_control` uses. Connector credential fields are all treated
//! as secret because connector schemas may choose their own field names.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde::Serialize;
use serde_json::Value;

use crate::tools::settings_catalog::is_secret_key;

/// Version stamped on every turn span so logs can be correlated with a build.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Replacement written in place of any known secret value.
pub const REDACTED: &str = "***";
const MAX_LOG_FILES: usize = 32;
const MAX_LOG_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DiagnosticContextSummary {
    pub latest_run_id: Option<String>,
    pub latest_task_id: Option<String>,
    pub latest_environment_id: Option<String>,
    pub failure_count: usize,
}

/// Background-task summary attached to diagnostic exports so a support
/// bundle shows what was running (builds/tests/dev servers) at export time.
/// Labels are command lines the user's own agent started; no output or
/// secrets are included.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BackgroundDiagSummary {
    pub running: usize,
    pub total: usize,
    pub tasks: Vec<BackgroundDiagTask>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackgroundDiagTask {
    pub id: String,
    pub label: String,
    pub status: String,
    pub session_id: String,
    pub exit_code: Option<i32>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct DiagnosticExportPreview {
    pub schema_version: u32,
    pub app_version: &'static str,
    pub files: Vec<DiagnosticLogFile>,
    pub total_bytes: u64,
    pub redaction_ready: bool,
    pub context_summary: DiagnosticContextSummary,
    pub background_tasks: BackgroundDiagSummary,
}

#[derive(Debug, Serialize)]
pub struct DiagnosticLogFile {
    pub name: String,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
struct DiagnosticFailureLocator {
    file: String,
    line: usize,
    level: &'static str,
    run_id: Option<String>,
    task_id: Option<String>,
    environment_id: Option<String>,
    tool_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct DiagnosticManifest {
    schema_version: u32,
    app_version: &'static str,
    files: Vec<DiagnosticLogFile>,
    failure_locators: Vec<DiagnosticFailureLocator>,
    locators_truncated: bool,
    context_summary: DiagnosticContextSummary,
    background_tasks: BackgroundDiagSummary,
}

fn span_field(line: &str, key: &str) -> Option<String> {
    let span = line.split_once("agent_turn{")?.1.split_once('}')?.0;
    event_field(span, key)
}

fn event_field(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    let raw = text
        .split_whitespace()
        .find_map(|part| part.strip_prefix(&prefix))?;
    let value = raw.trim_matches(',').trim_matches('"');
    if value.is_empty()
        || value == REDACTED
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return None;
    }
    Some(value.to_owned())
}

fn failure_level(line: &str) -> Option<&'static str> {
    if line.split_whitespace().any(|part| part == "ERROR") {
        Some("ERROR")
    } else if line.split_whitespace().any(|part| part == "WARN") {
        Some("WARN")
    } else {
        None
    }
}

/// Collect the non-empty string values stored under secret keys, longest first.
pub fn secret_values(settings: &Value) -> Vec<String> {
    let mut out = Vec::new();
    collect_secret_values(settings, &mut out);
    normalize_secrets(&mut out);
    out
}

fn normalize_secrets(out: &mut Vec<String>) {
    out.retain(|s| !s.is_empty() && s != REDACTED);
    out.sort();
    out.dedup();
    // Longest first, so a secret that contains another is replaced whole.
    out.sort_by_key(|s| std::cmp::Reverse(s.len()));
}

/// Read current connector credentials at export time, including rotated tokens.
/// A malformed or unreadable credential file prevents an incomplete redaction.
fn export_secret_values(config_dir: &Path, settings: &Value) -> Result<Vec<String>, String> {
    let mut secrets = secret_values(settings);
    let connectors = config_dir.join("connectors");
    let root_type = match std::fs::symlink_metadata(&connectors) {
        Ok(metadata) => metadata.file_type(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(secrets),
        Err(e) => return Err(format!("inspect connector credentials directory: {e}")),
    };
    if !root_type.is_dir() || root_type.is_symlink() {
        return Err("connector credentials directory is not a real directory".into());
    }
    let entries = match std::fs::read_dir(&connectors) {
        Ok(entries) => entries,
        Err(e) => return Err(format!("read connector credentials directory: {e}")),
    };
    for entry in entries {
        let entry = entry.map_err(|e| format!("read connector entry: {e}"))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("inspect connector entry: {e}"))?;
        if file_type.is_symlink() {
            return Err("connector entry is a symlink; redaction scope is incomplete".into());
        }
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path().join("credentials.json");
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("inspect connector credential: {e}")),
        };
        if !metadata.is_file() || metadata.len() > 256 * 1024 {
            return Err("connector credential is not a bounded regular file".into());
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("read connector credential: {e}"))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|e| format!("parse connector credential: {e}"))?;
        if !value.is_object() {
            return Err("connector credential is not an object".into());
        }
        // OAuth tokens are stored sealed; redact their plaintext, not only the ciphertext.
        if let Some(map) = value.as_object() {
            let text_fields: std::collections::HashMap<String, String> = map
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect();
            let opened = crate::commands::connectors::unseal_credentials(&connectors, text_fields)
                .map_err(|e| format!("unseal connector credential: {e}"))?;
            secrets.extend(opened.into_values());
        }
        collect_all_strings(&value, &mut secrets);
    }
    normalize_secrets(&mut secrets);
    Ok(secrets)
}

fn collect_secret_values(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "mcp_servers" {
                    if let Some(servers) = child.as_array() {
                        for server in servers {
                            for field in ["command", "args", "url", "env", "headers"] {
                                if let Some(value) = server.get(field) {
                                    collect_all_strings(value, out);
                                }
                            }
                        }
                    }
                } else if is_secret_key(key) || is_sensitive_url_key(key) {
                    collect_all_strings(child, out);
                } else {
                    collect_secret_values(child, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_secret_values(v, out)),
        _ => {}
    }
}

fn is_sensitive_url_key(key: &str) -> bool {
    // URLs can carry credentials in their authority, path or query even when
    // their setting name does not contain "token" or "secret".
    key == "url" || key.ends_with("_url")
}

fn collect_all_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|v| collect_all_strings(v, out)),
        Value::Object(map) => map.values().for_each(|v| collect_all_strings(v, out)),
        _ => {}
    }
}

/// Replace every occurrence of each known secret value in `text`.
/// `secrets` must be ordered longest first (as produced by [`secret_values`]).
pub fn redact_known_secrets(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_owned();
    let mut variants = secrets.to_vec();
    for secret in secrets {
        // Encoded credentials can appear in request URLs or authorization logs.
        // Keep one- and two-byte values literal to avoid masking unrelated text.
        if secret.len() < 3 {
            continue;
        }
        let bytes = secret.as_bytes();
        let percent: String = bytes
            .iter()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                    (*byte as char).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect();
        if percent != *secret {
            let lower_hex: String = bytes
                .iter()
                .map(|byte| {
                    if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                        (*byte as char).to_string()
                    } else {
                        format!("%{byte:02x}")
                    }
                })
                .collect();
            variants.push(percent);
            variants.push(lower_hex);
        }
        for encoded in [
            base64::engine::general_purpose::STANDARD.encode(bytes),
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes),
            base64::engine::general_purpose::URL_SAFE.encode(bytes),
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
        ] {
            variants.push(encoded);
        }
    }
    normalize_secrets(&mut variants);
    for secret in &variants {
        out = out.replace(secret.as_str(), REDACTED);
    }
    out
}

/// Concatenate bounded `agentz.log*` files under `config_dir/logs`, redact
/// current settings and connector credentials, and write to `config_dir/exports`.
fn bounded_log_files(config_dir: &Path) -> Result<Vec<(PathBuf, DiagnosticLogFile)>, String> {
    let logs_dir = config_dir.join("logs");
    let root_type = std::fs::symlink_metadata(&logs_dir)
        .map_err(|e| format!("inspect logs directory: {e}"))?
        .file_type();
    if !root_type.is_dir() || root_type.is_symlink() {
        return Err("diagnostic logs directory is not a real directory".into());
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&logs_dir).map_err(|e| format!("read logs: {e}"))? {
        let entry = entry.map_err(|e| format!("read log entry: {e}"))?;
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("agentz.log"))
        {
            files.push(path);
        }
    }
    files.sort();
    if files.is_empty() {
        return Err("no log files to export".to_string());
    }
    if files.len() > MAX_LOG_FILES {
        return Err("too many log files for a bounded export".into());
    }
    let mut total_bytes = 0u64;
    let mut selected = Vec::with_capacity(files.len());
    for file in files {
        let metadata = std::fs::symlink_metadata(&file).map_err(|e| format!("inspect log: {e}"))?;
        if !metadata.is_file() {
            return Err("diagnostic log is not a regular file".into());
        }
        total_bytes = total_bytes.saturating_add(metadata.len());
        if total_bytes > MAX_LOG_BYTES {
            return Err("diagnostic logs exceed bounded export size".into());
        }
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "diagnostic log name is not UTF-8".to_string())?
            .to_owned();
        selected.push((
            file,
            DiagnosticLogFile {
                name,
                bytes: metadata.len(),
            },
        ));
    }
    Ok(selected)
}

fn read_bounded_log(file: &Path, expected_bytes: u64) -> Result<String, String> {
    let opened = std::fs::File::open(file).map_err(|e| format!("open {}: {e}", file.display()))?;
    let metadata = opened
        .metadata()
        .map_err(|e| format!("inspect opened log: {e}"))?;
    if !metadata.is_file() || metadata.len() != expected_bytes {
        return Err("diagnostic log changed during export".into());
    }
    let mut bytes = Vec::new();
    opened
        .take(expected_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read {}: {e}", file.display()))?;
    if bytes.len() as u64 != expected_bytes {
        return Err("diagnostic log changed during export".into());
    }
    String::from_utf8(bytes).map_err(|e| format!("decode {}: {e}", file.display()))
}

/// Describe the exact bounded inputs before exporting. No log contents or secret
/// values are returned; unreadable credentials make preview fail closed.
pub fn preview_redacted_logs(
    config_dir: &Path,
    settings: &Value,
) -> Result<DiagnosticExportPreview, String> {
    preview_redacted_logs_with_background(config_dir, settings, BackgroundDiagSummary::default())
}

pub fn preview_redacted_logs_with_background(
    config_dir: &Path,
    settings: &Value,
    background_tasks: BackgroundDiagSummary,
) -> Result<DiagnosticExportPreview, String> {
    let files = bounded_log_files(config_dir)?;
    export_secret_values(config_dir, settings)?;
    let total_bytes = files.iter().map(|(_, file)| file.bytes).sum();
    let mut latest_run_id = None;
    let mut latest_task_id = None;
    let mut latest_environment_id = None;
    let mut failure_count = 0usize;
    for (file, info) in &files {
        let text = read_bounded_log(file, info.bytes)?;
        for line in text.lines() {
            if let Some(v) = span_field(line, "run_id") {
                latest_run_id = Some(v);
            }
            if let Some(v) = span_field(line, "task_id") {
                latest_task_id = Some(v);
            }
            if let Some(v) = span_field(line, "environment_id") {
                latest_environment_id = Some(v);
            }
            if failure_level(line).is_some() {
                failure_count += 1;
            }
        }
    }
    Ok(DiagnosticExportPreview {
        schema_version: 1,
        app_version: APP_VERSION,
        files: files.into_iter().map(|(_, file)| file).collect(),
        total_bytes,
        redaction_ready: true,
        context_summary: DiagnosticContextSummary {
            latest_run_id,
            latest_task_id,
            latest_environment_id,
            failure_count,
        },
        background_tasks,
    })
}

pub fn export_redacted_logs(config_dir: &Path, settings: &Value) -> Result<PathBuf, String> {
    export_redacted_logs_with_background(config_dir, settings, BackgroundDiagSummary::default())
}

pub fn export_redacted_logs_with_background(
    config_dir: &Path,
    settings: &Value,
    background_tasks: BackgroundDiagSummary,
) -> Result<PathBuf, String> {
    let files = bounded_log_files(config_dir)?;

    let secrets = export_secret_values(config_dir, settings)?;
    let mut body = String::new();
    let mut locators = Vec::new();
    let mut locators_truncated = false;
    let mut latest_run_id = None;
    let mut latest_task_id = None;
    let mut latest_environment_id = None;
    let mut failure_count = 0usize;
    for (file, info) in &files {
        let text = read_bounded_log(file, info.bytes)?;
        let redacted = redact_known_secrets(&text, &secrets);
        for (index, line) in redacted.lines().enumerate() {
            if let Some(v) = span_field(line, "run_id") {
                latest_run_id = Some(v);
            }
            if let Some(v) = span_field(line, "task_id") {
                latest_task_id = Some(v);
            }
            if let Some(v) = span_field(line, "environment_id") {
                latest_environment_id = Some(v);
            }
            if let Some(level) = failure_level(line) {
                failure_count += 1;
                if locators.len() == 100 {
                    locators_truncated = true;
                    continue;
                }
                locators.push(DiagnosticFailureLocator {
                    file: info.name.clone(),
                    line: index + 1,
                    level,
                    run_id: span_field(line, "run_id"),
                    task_id: span_field(line, "task_id"),
                    environment_id: span_field(line, "environment_id"),
                    tool_name: span_field(line, "tool_name")
                        .or_else(|| event_field(line, "tool_name")),
                });
            }
        }
        body.push_str(&redacted);
    }

    let manifest = DiagnosticManifest {
        schema_version: 1,
        app_version: APP_VERSION,
        files: files
            .iter()
            .map(|(_, info)| DiagnosticLogFile {
                name: info.name.clone(),
                bytes: info.bytes,
            })
            .collect(),
        failure_locators: locators,
        locators_truncated,
        context_summary: DiagnosticContextSummary {
            latest_run_id,
            latest_task_id,
            latest_environment_id,
            failure_count,
        },
        background_tasks,
    };
    let manifest_json =
        serde_json::to_string(&manifest).map_err(|e| format!("serialize manifest: {e}"))?;
    body.insert_str(
        0,
        &format!("# agentz-diagnostic-manifest {manifest_json}\n"),
    );

    let export_dir = config_dir.join("exports");
    std::fs::create_dir_all(&export_dir).map_err(|e| format!("create exports dir: {e}"))?;
    let export_type = std::fs::symlink_metadata(&export_dir)
        .map_err(|e| format!("inspect exports directory: {e}"))?
        .file_type();
    if !export_type.is_dir() || export_type.is_symlink() {
        return Err("diagnostic exports directory is not a real directory".into());
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let out = export_dir.join(format!(
        "agentz-diagnostics-{stamp}-{}.log",
        uuid::Uuid::new_v4()
    ));
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&out)
        .map_err(|e| format!("create export: {e}"))?;
    output
        .write_all(body.as_bytes())
        .map_err(|e| format!("write export: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bounded_log_read_rejects_growth_and_truncation() {
        let directory = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("agentz.log");
        std::fs::write(&file, "abc").unwrap();
        assert_eq!(read_bounded_log(&file, 3).unwrap(), "abc");
        assert!(read_bounded_log(&file, 2).is_err());
        assert!(read_bounded_log(&file, 4).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn export_manifest_locates_redacted_failures_and_correlated_turns() {
        let directory = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(directory.join("logs")).unwrap();
        let marker = "AZ061_MANIFEST_SECRET_MARKER";
        std::fs::write(
            directory.join("logs").join("agentz.log"),
            format!("2026-10-16 ERROR agent_turn{{run_id=run-1 task_id=task-1 environment_id=env-1 tool_name=file_read}}: {marker}\n2026-10-16 WARN agent_turn{{run_id=run-2 task_id=task-2 environment_id=env-2}}: tool_name=file_edit tool execution failed\n"),
        ).unwrap();
        let exported = export_redacted_logs(&directory, &json!({"api_key": marker})).unwrap();
        let body = std::fs::read_to_string(exported).unwrap();
        assert!(!body.contains(marker));
        let header = body
            .lines()
            .next()
            .unwrap()
            .strip_prefix("# agentz-diagnostic-manifest ")
            .unwrap();
        let manifest: Value = serde_json::from_str(header).unwrap();
        assert_eq!(manifest["files"][0]["name"], "agentz.log");
        assert_eq!(manifest["failure_locators"][0]["line"], 1);
        assert_eq!(manifest["failure_locators"][0]["run_id"], "run-1");
        assert_eq!(manifest["failure_locators"][0]["task_id"], "task-1");
        assert_eq!(manifest["failure_locators"][0]["environment_id"], "env-1");
        assert_eq!(manifest["failure_locators"][0]["tool_name"], "file_read");
        assert_eq!(manifest["failure_locators"][1]["line"], 2);
        assert_eq!(manifest["failure_locators"][1]["run_id"], "run-2");
        assert_eq!(manifest["failure_locators"][1]["tool_name"], "file_edit");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn collects_secret_values_from_nested_settings_including_short_values() {
        let settings = json!({
            "api_key": "sk-live-0123456789",
            "providers": [{ "auth": { "token": "tok-abcdefghij" }, "name": "openai" }],
            "model": "gpt-test-model",
            "short_secret": "abc",
            "already": "***",
        });
        let secrets = secret_values(&settings);
        assert!(secrets.contains(&"sk-live-0123456789".to_string()));
        assert!(secrets.contains(&"tok-abcdefghij".to_string()));
        assert!(!secrets.iter().any(|s| s == "gpt-test-model"));
        assert!(secrets.iter().any(|s| s == "abc"));
        assert!(!secrets.iter().any(|s| s == REDACTED));
    }

    #[test]
    fn diagnostic_redaction_includes_token_bearing_webhook_urls() {
        let marker = "synthetic-webhook-secret-marker";
        let settings = json!({
            "slack_webhook_url": format!("https://example.test/slack/{marker}"),
            "discord_webhook_url": format!("https://example.test/discord/{marker}"),
            "teams_webhook_url": format!("https://example.test/teams/{marker}"),
            "webhook_outbound_url": format!("https://example.test/outbound/{marker}"),
            "custom_base_url": "https://example.test/model/secret-marker",
            "public_name": "fixture"
        });
        let secrets = secret_values(&settings);
        assert_eq!(secrets.len(), 5);
        let text = format!("failed https://example.test/slack/{marker}");
        assert_eq!(redact_known_secrets(&text, &secrets), "failed ***");
    }

    #[test]
    fn redacts_every_occurrence_and_prefers_the_longest_secret() {
        let secrets = secret_values(&json!({
            "api_key": "sk-abcdefgh",
            "refresh_token": "sk-abcdefgh-refresh",
        }));
        let text = "auth sk-abcdefgh-refresh then sk-abcdefgh again";
        assert_eq!(
            redact_known_secrets(text, &secrets),
            "auth *** then *** again"
        );
    }

    #[test]
    fn redacts_url_and_base64_encoded_secrets() {
        let secret = "Key+/中";
        let variants = secret_values(&json!({ "api_key": secret }));
        let encoded = base64::engine::general_purpose::STANDARD.encode(secret);
        let url_safe = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);
        let text = format!("literal {secret} url Key%2B%2F%E4%B8%AD lower Key%2b%2f%e4%b8%ad base64 {encoded} safe {url_safe}");
        let redacted = redact_known_secrets(&text, &variants);
        assert!(!redacted.contains(secret));
        assert!(!redacted.contains("Key%2B%2F%E4%B8%AD"));
        assert!(!redacted.contains("Key%2b%2f%e4%b8%ad"));
        assert!(!redacted.contains(&encoded));
        assert!(!redacted.contains(&url_safe));
    }

    #[test]
    fn mcp_values_are_redacted_without_masking_server_identity() {
        let marker = "AZ031_MCP_DIAGNOSTIC_SYNTHETIC_SECRET";
        let settings = json!({"mcp_servers":[{
            "name":"fixture", "transport":"stdio", "command":marker,
            "args":[marker], "url":marker,
            "env":{"CUSTOM":marker}, "headers":{"X-Custom":marker}
        }]});
        let secrets = secret_values(&settings);
        assert_eq!(secrets, vec![marker.to_string()]);
        assert!(!secrets.contains(&"fixture".to_string()));
        assert!(!redact_known_secrets(&format!("request {marker}"), &secrets).contains(marker));
        let dir = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(
            dir.join("logs").join("agentz.log"),
            format!("request {marker}\n"),
        )
        .unwrap();
        let export = export_redacted_logs(&dir, &settings).unwrap();
        assert!(!std::fs::read_to_string(export).unwrap().contains(marker));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn export_writes_redacted_copy_and_refuses_missing_logs() {
        let dir = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        assert!(export_redacted_logs(&dir, &json!({})).is_err());

        std::fs::write(
            dir.join("logs").join("agentz.log.2026-10-13"),
            "request with key sk-live-0123456789 ok\n",
        )
        .unwrap();
        std::fs::write(dir.join("logs").join("unrelated.txt"), "sk-live-0123456789").unwrap();

        let out = export_redacted_logs(&dir, &json!({ "api_key": "sk-live-0123456789" })).unwrap();
        let body = std::fs::read_to_string(&out).unwrap();
        assert!(body.contains("request with key *** ok"));
        assert!(!body.contains("sk-live-0123456789"));
        let second =
            export_redacted_logs(&dir, &json!({ "api_key": "sk-live-0123456789" })).unwrap();
        assert_ne!(out, second);
        assert_eq!(std::fs::read_to_string(&out).unwrap(), body);
        assert_eq!(std::fs::read_dir(dir.join("exports")).unwrap().count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg_attr(
        not(any(windows, target_os = "macos")),
        ignore = "needs the OS-protected credential store; Linux CI has no Secret Service"
    )]
    fn export_secret_values_opens_sealed_oauth_tokens() {
        let config = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        let connectors = config.join("connectors");
        std::fs::create_dir_all(connectors.join("demo")).unwrap();
        crate::commands::connectors::store_oauth_tokens(
            &connectors,
            "demo",
            "tok-diag-secret",
            Some("ref-diag-secret"),
            None,
            0,
        )
        .unwrap();

        // Redaction must see the plaintext tokens, not only their sealed form.
        let secrets = export_secret_values(&config, &serde_json::json!({})).unwrap();
        assert!(secrets.iter().any(|s| s == "tok-diag-secret"));
        assert!(secrets.iter().any(|s| s == "ref-diag-secret"));
        std::fs::remove_dir_all(config).unwrap();
    }

    #[test]
    fn export_redacts_short_settings_and_current_connector_credentials() {
        let dir = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        let connector = dir.join("connectors").join("sample");
        std::fs::create_dir_all(&connector).unwrap();
        std::fs::write(
            connector.join("credentials.json"),
            r#"{"access_token":"old"}"#,
        )
        .unwrap();
        // Rotation must be observed from disk when export starts.
        std::fs::write(
            connector.join("credentials.json"),
            r#"{"access_token":"new"}"#,
        )
        .unwrap();
        std::fs::write(dir.join("logs").join("agentz.log"), "short abc token new\n").unwrap();
        let out = export_redacted_logs(&dir, &json!({ "api_key": "abc" })).unwrap();
        let body = std::fs::read_to_string(out).unwrap();
        assert!(!body.contains("abc"));
        assert!(!body.contains("new"));
        assert!(body.contains("short *** token ***"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_connector_credential_fails_closed_before_export() {
        let dir = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        let connector = dir.join("connectors").join("sample");
        std::fs::create_dir_all(&connector).unwrap();
        std::fs::write(connector.join("credentials.json"), "{").unwrap();
        std::fs::write(dir.join("logs").join("agentz.log"), "token\n").unwrap();
        assert!(export_redacted_logs(&dir, &json!({})).is_err());
        assert!(!dir.join("exports").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn remove_link(path: &Path) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            std::fs::remove_dir(path)
        }
        #[cfg(unix)]
        {
            std::fs::remove_file(path)
        }
    }

    #[test]
    fn linked_connector_directory_fails_closed_before_export() {
        let dir = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        let external = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::create_dir_all(&external).unwrap();
        std::fs::write(dir.join("logs").join("agentz.log"), "secret marker\n").unwrap();
        let link = dir.join("connectors");
        #[cfg(windows)]
        {
            let output = piscis_kernel::proc::std_command("cmd")
                .args(["/D", "/C", "mklink", "/J"])
                .arg(&link)
                .arg(&external)
                .output()
                .expect("create connector junction");
            assert!(output.status.success(), "junction creation failed");
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&external, &link).expect("create connector symlink");
        let rejected = export_redacted_logs(&dir, &json!({})).is_err();
        let no_export = !dir.join("exports").exists();
        remove_link(&link).expect("remove only connector link");
        assert!(rejected);
        assert!(no_export);
        std::fs::create_dir(&link).unwrap();
        let child = link.join("linked-connector");
        #[cfg(windows)]
        {
            let output = piscis_kernel::proc::std_command("cmd")
                .args(["/D", "/C", "mklink", "/J"])
                .arg(&child)
                .arg(&external)
                .output()
                .expect("create connector child junction");
            assert!(output.status.success(), "child junction creation failed");
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&external, &child).expect("create connector child symlink");
        let child_rejected = export_redacted_logs(&dir, &json!({})).is_err();
        remove_link(&child).expect("remove only connector child link");
        assert!(child_rejected);
        assert!(!dir.join("exports").exists());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&external);
    }

    #[test]
    fn linked_log_and_export_directories_fail_closed() {
        let dir = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        let external = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&external).unwrap();
        std::fs::write(external.join("agentz.log"), "external marker\n").unwrap();
        let logs = dir.join("logs");
        #[cfg(windows)]
        {
            let output = piscis_kernel::proc::std_command("cmd")
                .args(["/D", "/C", "mklink", "/J"])
                .arg(&logs)
                .arg(&external)
                .output()
                .expect("create logs junction");
            assert!(output.status.success());
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&external, &logs).expect("create logs symlink");
        assert!(preview_redacted_logs(&dir, &json!({})).is_err());
        assert!(export_redacted_logs(&dir, &json!({})).is_err());
        remove_link(&logs).expect("remove only logs link");
        std::fs::create_dir(&logs).unwrap();
        std::fs::write(logs.join("agentz.log"), "local marker\n").unwrap();
        let exports = dir.join("exports");
        #[cfg(windows)]
        {
            let output = piscis_kernel::proc::std_command("cmd")
                .args(["/D", "/C", "mklink", "/J"])
                .arg(&exports)
                .arg(&external)
                .output()
                .expect("create exports junction");
            assert!(output.status.success());
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&external, &exports).expect("create exports symlink");
        assert!(export_redacted_logs(&dir, &json!({})).is_err());
        assert_eq!(std::fs::read_dir(&external).unwrap().count(), 1);
        remove_link(&exports).expect("remove only exports link");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&external);
    }

    #[test]
    fn oversized_logs_fail_before_export_file_is_created() {
        let dir = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        let file = std::fs::File::create(dir.join("logs").join("agentz.log")).unwrap();
        file.set_len(MAX_LOG_BYTES + 1).unwrap();
        assert!(export_redacted_logs(&dir, &json!({})).is_err());
        assert!(!dir.join("exports").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preview_lists_only_bounded_log_names_and_fails_closed_on_credentials() {
        let dir = std::env::temp_dir().join(format!("agentz-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join("logs").join("agentz.log"), "secret value\n").unwrap();
        std::fs::write(dir.join("logs").join("ignored.txt"), "ignored").unwrap();
        let preview = preview_redacted_logs(&dir, &json!({ "api_key": "secret" })).unwrap();
        assert_eq!(preview.schema_version, 1);
        assert_eq!(preview.app_version, APP_VERSION);
        assert_eq!(preview.total_bytes, 13);
        assert_eq!(preview.files.len(), 1);
        assert_eq!(preview.files[0].name, "agentz.log");
        let serialized = serde_json::to_string(&preview).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("ignored.txt"));
        let connector = dir.join("connectors").join("bad");
        std::fs::create_dir_all(&connector).unwrap();
        std::fs::write(connector.join("credentials.json"), "{").unwrap();
        assert!(preview_redacted_logs(&dir, &json!({})).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
