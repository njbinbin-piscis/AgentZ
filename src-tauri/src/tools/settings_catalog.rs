//! Settings catalog for the `app_control` tool: which `config.json` fields the
//! agent may touch, at which authorization tier, and what values are valid.
//!
//! * `Free` — low-risk preferences; written without a prompt (still audited).
//! * `Privileged` — safety, harness or routing knobs; every write prompts the user.
//! * `Locked` — secrets, authorization switches themselves, anything not listed.
//!   The agent can never change these.

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Free,
    Privileged,
    Locked,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Free => "free",
            Tier::Privileged => "privileged",
            Tier::Locked => "locked",
        }
    }
}

const FREE: &[(&str, &str)] = &[
    ("language", "UI / reply language, e.g. zh or en"),
    ("enable_streaming", "Stream model output"),
    ("temperature", "Sampling temperature 0..2 (null = provider default)"),
    ("top_p", "Nucleus sampling 0..1 (null = provider default)"),
    ("thinking", "Enable model thinking mode (null = default)"),
    ("max_tokens", "Max output tokens per reply"),
    ("context_window", "Model context window in tokens"),
    ("browser_headless", "Run the automation browser headless"),
    ("vision_enabled", "Allow image understanding"),
    ("vision_use_main_llm", "Use the main model for vision"),
    ("summary_model", "Model used for summaries (null = main model)"),
    ("project_instruction_budget_chars", "Budget for project instruction files"),
    ("enable_project_instructions", "Load project instruction files"),
    ("llm_read_timeout_secs", "LLM read timeout in seconds"),
    ("koi_timeout_secs", "Sub-agent task timeout in seconds"),
    ("heartbeat_enabled", "Enable the periodic heartbeat"),
    ("heartbeat_interval_mins", "Heartbeat interval in minutes"),
    ("im_message_mode", "Message mode for IM channels"),
];

const PRIVILEGED: &[(&str, &str)] = &[
    ("policy_mode", "Tool policy profile: strict | balanced | dev"),
    ("confirm_shell_commands", "Ask before running shell commands"),
    ("confirm_file_writes", "Ask before writing files"),
    ("allow_outside_workspace", "Allow file access outside the workspace"),
    ("tool_rate_limit_per_minute", "Tool calls per minute limit"),
    ("workspace_root", "Default working directory"),
    ("provider", "Default model provider"),
    ("model", "Default model name"),
    ("custom_base_url", "Custom API base URL"),
    ("fallback_models", "Ordered fallback model list"),
    ("llm_providers", "Configured model providers (merged by id; API keys are never exposed)"),
    ("mcp_servers", "MCP server definitions (merged by name)"),
    ("max_iterations", "Max agent loop iterations per turn"),
    ("auto_compact_input_tokens_threshold", "Auto-compaction input token threshold"),
    ("compaction_micro_percent", "Micro compaction trigger percent"),
    ("compaction_auto_percent", "Auto compaction trigger percent"),
    ("compaction_full_percent", "Full compaction trigger percent"),
    ("max_tool_result_tokens", "Max tokens kept from one tool result"),
    ("skill_evolution", "Skill self-evolution settings object"),
    ("piscis_personal_prompt", "Custom instructions (use append_instructions)"),
];

const SECRET_HINTS: &[&str] = &["api_key", "secret", "token", "password", "private_key"];

pub fn is_secret_key(key: &str) -> bool {
    let k = key.to_lowercase();
    SECRET_HINTS.iter().any(|h| k.contains(h))
}

pub fn tier_of(key: &str) -> Tier {
    if is_secret_key(key) {
        return Tier::Locked;
    }
    if FREE.iter().any(|(k, _)| *k == key) {
        Tier::Free
    } else if PRIVILEGED.iter().any(|(k, _)| *k == key) {
        Tier::Privileged
    } else {
        Tier::Locked
    }
}

/// Machine-readable catalog for the `describe_settings` action.
pub fn describe() -> Value {
    let items = |list: &[(&str, &str)], tier: Tier| -> Vec<Value> {
        list.iter()
            .map(|(k, d)| json!({ "key": k, "tier": tier.as_str(), "description": d }))
            .collect()
    };
    json!({
        "free": items(FREE, Tier::Free),
        "privileged": items(PRIVILEGED, Tier::Privileged),
        "locked": "API keys / tokens / passwords, tool allow-lists, SSH servers, runtime paths, \
                   gateway and channel credentials, and any field not listed above cannot be \
                   changed by the agent.",
        "notes": [
            "Privileged writes prompt the user every time.",
            "Arrays of objects with an `id` or `name` are merged by that key; omitted entries are kept.",
            "Every change is audited and can be reverted with `rollback`."
        ]
    })
}

fn num_in(v: &Value, lo: f64, hi: f64, key: &str) -> Result<(), String> {
    match v.as_f64() {
        Some(n) if n >= lo && n <= hi => Ok(()),
        _ => Err(format!("`{key}` must be a number in {lo}..={hi}")),
    }
}

/// Range / enum validation for one top-level field. Type errors are caught later
/// when the merged value is deserialized into `Settings`.
pub fn validate(key: &str, v: &Value) -> Result<(), String> {
    if let Value::String(s) = v {
        if s.len() > 4000 {
            return Err(format!("`{key}` is too long"));
        }
    }
    match key {
        "policy_mode" => match v.as_str() {
            Some("strict" | "balanced" | "dev") => Ok(()),
            _ => Err("`policy_mode` must be strict, balanced or dev".into()),
        },
        "temperature" if !v.is_null() => num_in(v, 0.0, 2.0, key),
        "top_p" if !v.is_null() => num_in(v, 0.0, 1.0, key),
        "max_iterations" => num_in(v, 1.0, 500.0, key),
        "compaction_micro_percent" | "compaction_auto_percent" | "compaction_full_percent" => {
            num_in(v, 1.0, 100.0, key)
        }
        "tool_rate_limit_per_minute" => num_in(v, 1.0, 10_000.0, key),
        "max_tokens" => num_in(v, 256.0, 1_000_000.0, key),
        "context_window" => num_in(v, 1024.0, 10_000_000.0, key),
        "llm_read_timeout_secs" | "koi_timeout_secs" => num_in(v, 1.0, 86_400.0, key),
        "heartbeat_interval_mins" => num_in(v, 1.0, 10_080.0, key),
        "confirm_shell_commands"
        | "confirm_file_writes"
        | "allow_outside_workspace"
        | "enable_streaming"
        | "browser_headless"
        | "vision_enabled"
        | "vision_use_main_llm"
        | "enable_project_instructions"
        | "heartbeat_enabled" => {
            if v.is_boolean() {
                Ok(())
            } else {
                Err(format!("`{key}` must be true or false"))
            }
        }
        _ => Ok(()),
    }
}

/// True if the patch contains a secret-looking key with a real value anywhere.
/// `***` (what `get_settings` shows) and empty strings are tolerated and later stripped.
pub fn find_secret_write(patch: &Value, path: &str) -> Option<String> {
    match patch {
        Value::Object(map) => {
            for (k, v) in map {
                let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                if is_secret_key(k) {
                    let benign = matches!(v, Value::String(s) if s.is_empty() || s == "***")
                        || v.is_null();
                    if !benign {
                        return Some(p);
                    }
                } else if let Some(hit) = find_secret_write(v, &p) {
                    return Some(hit);
                }
            }
            None
        }
        Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(i, v)| find_secret_write(v, &format!("{path}[{i}]"))),
        _ => None,
    }
}

/// Remove secret-looking keys from a patch (they were checked as benign).
pub fn strip_secrets(patch: &mut Value) {
    match patch {
        Value::Object(map) => {
            map.retain(|k, _| !is_secret_key(k));
            map.values_mut().for_each(strip_secrets);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_secrets),
        _ => {}
    }
}

/// Deep merge; arrays of objects keyed by `id`/`name` merge element-wise and
/// keep entries the patch omits, other arrays replace.
pub fn merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p {
                merge(b.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (Value::Array(b), Value::Array(p)) if keyed(p) => {
            for item in p {
                let key = item_key(item);
                match b.iter_mut().find(|e| item_key(e) == key) {
                    Some(existing) => merge(existing, item),
                    None => b.push(item.clone()),
                }
            }
        }
        (b, p) => *b = p.clone(),
    }
}

fn item_key(v: &Value) -> Option<String> {
    ["id", "name"]
        .iter()
        .find_map(|k| v.get(*k).and_then(|x| x.as_str()))
        .map(str::to_string)
}

fn keyed(items: &[Value]) -> bool {
    !items.is_empty() && items.iter().all(|i| i.is_object() && item_key(i).is_some())
}

/// Field-level diff; secrets are masked so the result is safe to show or log.
pub fn diff(path: &str, old: &Value, new: &Value, out: &mut Vec<Value>) {
    if old == new {
        return;
    }
    match (old, new) {
        (Value::Object(o), Value::Object(n)) => {
            let mut keys: Vec<&String> = o.keys().chain(n.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                diff(&p, o.get(k).unwrap_or(&Value::Null), n.get(k).unwrap_or(&Value::Null), out);
            }
        }
        (Value::Array(o), Value::Array(n)) if keyed(o) && keyed(n) => {
            let mut keys: Vec<String> = o.iter().chain(n.iter()).filter_map(item_key).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let find = |list: &Vec<Value>| {
                    list.iter()
                        .find(|e| item_key(e).as_deref() == Some(k.as_str()))
                        .cloned()
                        .unwrap_or(Value::Null)
                };
                diff(&format!("{path}[{k}]"), &find(o), &find(n), out);
            }
        }
        _ => {
            let leaf = path.rsplit(['.', ']']).find(|s| !s.is_empty()).unwrap_or(path);
            let (o, n) = if is_secret_key(leaf) {
                (mask(old), mask(new))
            } else {
                (old.clone(), new.clone())
            };
            out.push(json!({ "path": path, "old": o, "new": n }));
        }
    }
}

fn mask(v: &Value) -> Value {
    match v {
        Value::String(s) if !s.is_empty() => json!("***"),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers() {
        assert_eq!(tier_of("language"), Tier::Free);
        assert_eq!(tier_of("policy_mode"), Tier::Privileged);
        assert_eq!(tier_of("openai_api_key"), Tier::Locked);
        assert_eq!(tier_of("builtin_tool_enabled"), Tier::Locked);
        assert_eq!(tier_of("whatever"), Tier::Locked);
    }

    #[test]
    fn validation() {
        assert!(validate("policy_mode", &json!("dev")).is_ok());
        assert!(validate("policy_mode", &json!("yolo")).is_err());
        assert!(validate("temperature", &json!(3)).is_err());
        assert!(validate("temperature", &Value::Null).is_ok());
        assert!(validate("max_iterations", &json!(0)).is_err());
        assert!(validate("confirm_shell_commands", &json!("no")).is_err());
    }

    #[test]
    fn secrets_in_patch() {
        let bad = json!({"llm_providers":[{"id":"a","api_key":"sk-1"}]});
        assert_eq!(find_secret_write(&bad, "").as_deref(), Some("llm_providers[0].api_key"));
        let ok = json!({"llm_providers":[{"id":"a","api_key":"***","model":"m"}]});
        assert!(find_secret_write(&ok, "").is_none());
        let mut stripped = ok.clone();
        strip_secrets(&mut stripped);
        assert_eq!(stripped, json!({"llm_providers":[{"id":"a","model":"m"}]}));
    }

    #[test]
    fn keyed_merge_keeps_others_and_secrets() {
        let mut base = json!({"llm_providers":[
            {"id":"a","api_key":"k1","model":"x"},
            {"id":"b","api_key":"k2","model":"y"}]});
        merge(&mut base, &json!({"llm_providers":[{"id":"a","model":"z"}]}));
        assert_eq!(base["llm_providers"][0]["api_key"], "k1");
        assert_eq!(base["llm_providers"][0]["model"], "z");
        assert_eq!(base["llm_providers"][1]["model"], "y");
    }

    #[test]
    fn diff_masks_secrets() {
        let mut out = vec![];
        diff("", &json!({"a":{"api_key":"k1","m":1}}), &json!({"a":{"api_key":"k2","m":2}}), &mut out);
        assert_eq!(out.len(), 2);
        let key = out.iter().find(|d| d["path"] == "a.api_key").unwrap();
        assert_eq!(key["old"], "***");
    }
}
