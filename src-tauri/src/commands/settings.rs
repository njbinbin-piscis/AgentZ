//! LLM / kernel settings — load & save `config.json` for the desktop app.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use piscis_kernel::store::settings::{LlmProviderConfig, McpServerConfig, Settings};

use crate::commands::chat::resolve_config_dir;

// Serialize writes from the settings IPC surface across load-modify-save.
// Other configuration writers still need a shared cross-surface contract.
static SETTINGS_WRITE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) async fn settings_write_guard() -> tokio::sync::MutexGuard<'static, ()> {
    SETTINGS_WRITE_LOCK.lock().await
}

/// MCP server config exposed to / from the settings UI (M6).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerDto {
    pub name: String,
    pub transport: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub command_configured: bool,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub args_configured: bool,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub url_configured: bool,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// For sse / http transports: extra HTTP headers (e.g. Authorization).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderDto {
    pub id: String,
    pub label: String,
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub max_tokens: u32,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub thinking: Option<bool>,
    #[serde(default)]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveLlmSettings {
    pub provider: String,
    pub model: String,
    pub custom_base_url: String,
    pub max_tokens: u32,
    pub context_window: u32,
    pub max_iterations: u32,
    pub policy_mode: String,
    pub enable_streaming: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub thinking: Option<bool>,
    pub language: String,
    pub vision_enabled: bool,
    pub anthropic_api_key: String,
    pub openai_api_key: String,
    pub deepseek_api_key: String,
    pub qwen_api_key: String,
    pub minimax_api_key: String,
    pub zhipu_api_key: String,
    pub kimi_api_key: String,
    pub llm_providers: Vec<LlmProviderDto>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecretReplacement {
    pub reference: String,
    pub value: String,
}

/// Lean payload returned to the settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettingsDto {
    pub config_dir: String,
    pub is_configured: bool,
    pub secret_references: Vec<SecretReferenceDto>,
    pub provider: String,
    pub model: String,
    pub custom_base_url: String,
    pub max_tokens: u32,
    pub context_window: u32,
    pub max_iterations: u32,
    pub policy_mode: String,
    pub enable_streaming: bool,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub thinking: Option<bool>,
    pub language: String,
    pub vision_enabled: bool,
    pub anthropic_api_key: String,
    pub openai_api_key: String,
    pub deepseek_api_key: String,
    pub qwen_api_key: String,
    pub minimax_api_key: String,
    pub zhipu_api_key: String,
    pub kimi_api_key: String,
    pub llm_providers: Vec<LlmProviderDto>,
    pub mcp_servers: Vec<McpServerDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretReferenceDto {
    pub reference: String,
    pub exists: bool,
}

const GLOBAL_SECRET_REFS: [&str; 7] = [
    "model:anthropic",
    "model:openai",
    "model:deepseek",
    "model:qwen",
    "model:minimax",
    "model:zhipu",
    "model:kimi",
];

fn secret_slot_mut<'a>(
    settings: &'a mut Settings,
    reference: &str,
) -> Result<&'a mut String, String> {
    match reference {
        "model:anthropic" => Ok(&mut settings.anthropic_api_key),
        "model:openai" => Ok(&mut settings.openai_api_key),
        "model:deepseek" => Ok(&mut settings.deepseek_api_key),
        "model:qwen" => Ok(&mut settings.qwen_api_key),
        "model:minimax" => Ok(&mut settings.minimax_api_key),
        "model:zhipu" => Ok(&mut settings.zhipu_api_key),
        "model:kimi" => Ok(&mut settings.kimi_api_key),
        _ => {
            if let Some((name, field)) = reference
                .strip_prefix("mcp:")
                .and_then(|rest| rest.rsplit_once(':'))
            {
                if !name.is_empty() {
                    if let Some(server) = settings
                        .mcp_servers
                        .iter_mut()
                        .find(|server| server.name == name)
                    {
                        return match field {
                            "command" => Ok(&mut server.command),
                            "url" => Ok(&mut server.url),
                            _ => Err("unknown secret reference".into()),
                        };
                    }
                }
                return Err("unknown secret reference".into());
            }
            let id = reference
                .strip_prefix("provider:")
                .filter(|id| !id.is_empty())
                .ok_or_else(|| "unknown secret reference".to_string())?;
            settings
                .llm_providers
                .iter_mut()
                .find(|provider| provider.id == id)
                .map(|provider| &mut provider.api_key)
                .ok_or_else(|| "unknown secret reference".to_string())
        }
    }
}

fn mcp_args_slot_mut<'a>(
    settings: &'a mut Settings,
    reference: &str,
) -> Option<&'a mut Vec<String>> {
    let name = reference.strip_prefix("mcp:")?.strip_suffix(":args")?;
    settings
        .mcp_servers
        .iter_mut()
        .find(|server| server.name == name)
        .map(|server| &mut server.args)
}

fn parse_mcp_args_replacement(value: &str) -> Result<Vec<String>, String> {
    let parsed: Vec<String> = serde_json::from_str(value)
        .map_err(|_| "MCP args replacement must be a JSON string array".to_string())?;
    if parsed.is_empty() {
        return Err("MCP args replacement must be nonempty; use delete to clear".into());
    }
    Ok(parsed)
}

fn mcp_map_reference(kind: &str, name: &str, key: &str) -> String {
    format!(
        "mcp-{kind}:{}",
        serde_json::to_string(&(name, key)).expect("string pair serializes")
    )
}

fn parse_mcp_map_reference(reference: &str) -> Option<(&str, String, String)> {
    let (kind, encoded) = if let Some(encoded) = reference.strip_prefix("mcp-env:") {
        ("env", encoded)
    } else {
        ("header", reference.strip_prefix("mcp-header:")?)
    };
    let (name, key): (String, String) = serde_json::from_str(encoded).ok()?;
    if name.is_empty() || key.is_empty() {
        return None;
    }
    Some((kind, name, key))
}

fn mcp_map_mut<'a>(
    settings: &'a mut Settings,
    kind: &str,
    name: &str,
) -> Option<&'a mut HashMap<String, String>> {
    let server = settings
        .mcp_servers
        .iter_mut()
        .find(|server| server.name == name)?;
    match kind {
        "env" => Some(&mut server.env),
        "header" => Some(&mut server.headers),
        _ => None,
    }
}

fn secret_references(settings: &Settings) -> Vec<SecretReferenceDto> {
    let global_values = [
        &settings.anthropic_api_key,
        &settings.openai_api_key,
        &settings.deepseek_api_key,
        &settings.qwen_api_key,
        &settings.minimax_api_key,
        &settings.zhipu_api_key,
        &settings.kimi_api_key,
    ];
    let mut references: Vec<SecretReferenceDto> = GLOBAL_SECRET_REFS
        .iter()
        .zip(global_values)
        .map(|(reference, value)| SecretReferenceDto {
            reference: (*reference).into(),
            exists: !value.is_empty(),
        })
        .collect();
    references.extend(
        settings
            .llm_providers
            .iter()
            .map(|provider| SecretReferenceDto {
                reference: format!("provider:{}", provider.id),
                exists: !provider.api_key.is_empty(),
            }),
    );
    for server in &settings.mcp_servers {
        references.push(SecretReferenceDto {
            reference: format!("mcp:{}:command", server.name),
            exists: !server.command.is_empty(),
        });
        references.push(SecretReferenceDto {
            reference: format!("mcp:{}:url", server.name),
            exists: !server.url.is_empty(),
        });
        references.push(SecretReferenceDto {
            reference: format!("mcp:{}:args", server.name),
            exists: !server.args.is_empty(),
        });
        for (key, value) in &server.env {
            references.push(SecretReferenceDto {
                reference: mcp_map_reference("env", &server.name, key),
                exists: !value.is_empty(),
            });
        }
        for (key, value) in &server.headers {
            references.push(SecretReferenceDto {
                reference: mcp_map_reference("header", &server.name, key),
                exists: !value.is_empty(),
            });
        }
    }
    references
}

fn mcp_to_dto(s: &McpServerConfig) -> McpServerDto {
    McpServerDto {
        name: s.name.clone(),
        transport: s.transport.clone(),
        command: String::new(),
        command_configured: !s.command.is_empty(),
        args: Vec::new(),
        args_configured: !s.args.is_empty(),
        url: String::new(),
        url_configured: !s.url.is_empty(),
        env: s
            .env
            .keys()
            .map(|key| (key.clone(), String::new()))
            .collect(),
        headers: s
            .headers
            .keys()
            .map(|key| (key.clone(), String::new()))
            .collect(),
        enabled: s.enabled,
    }
}

fn load_settings(app: &AppHandle) -> Result<(Settings, String), String> {
    let config_dir = resolve_config_dir(app)?;
    let config_path = config_dir.join("config.json");
    let mut settings = Settings::load(&config_path).map_err(|e| e.to_string())?;
    // Settings from before this field was exposed used the kernel's old
    // implicit 50-iteration default. There was no desktop control for that
    // value, so migrate the legacy default (and a missing field) to 200; any
    // subsequent value saved through the new UI is preserved.
    let has_max_iterations = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|value| value.get("max_iterations").cloned())
        .is_some();
    if !has_max_iterations || settings.max_iterations == 50 {
        settings.max_iterations = 200;
    }
    settings.config_path = config_path;
    Ok((settings, config_dir.display().to_string()))
}

fn provider_to_dto(p: &LlmProviderConfig) -> LlmProviderDto {
    LlmProviderDto {
        id: p.id.clone(),
        label: p.label.clone(),
        provider: p.provider.clone(),
        model: p.model.clone(),
        // A settings read is a presence/configuration view, never a key export.
        // Empty updates already mean "keep the stored value" in save_settings.
        api_key: String::new(),
        base_url: p.base_url.clone(),
        max_tokens: p.max_tokens,
        temperature: p.temperature,
        top_p: p.top_p,
        thinking: p.thinking,
        stream: p.stream,
    }
}

fn to_dto(settings: &Settings, config_dir: String) -> LlmSettingsDto {
    LlmSettingsDto {
        config_dir,
        is_configured: settings.is_configured(),
        secret_references: secret_references(settings),
        provider: settings.provider.clone(),
        model: settings.model.clone(),
        custom_base_url: settings.custom_base_url.clone(),
        max_tokens: settings.max_tokens,
        context_window: settings.context_window,
        max_iterations: settings.max_iterations,
        policy_mode: settings.policy_mode.clone(),
        enable_streaming: settings.enable_streaming,
        temperature: settings.temperature,
        top_p: settings.top_p,
        thinking: settings.thinking,
        language: settings.language.clone(),
        vision_enabled: settings.vision_enabled,
        anthropic_api_key: String::new(),
        openai_api_key: String::new(),
        deepseek_api_key: String::new(),
        qwen_api_key: String::new(),
        minimax_api_key: String::new(),
        zhipu_api_key: String::new(),
        kimi_api_key: String::new(),
        llm_providers: settings.llm_providers.iter().map(provider_to_dto).collect(),
        mcp_servers: settings.mcp_servers.iter().map(mcp_to_dto).collect(),
    }
}

fn merge_masked_values(
    incoming: HashMap<String, String>,
    existing: Option<&HashMap<String, String>>,
    preserve_missing: bool,
) -> HashMap<String, String> {
    let mut merged = if preserve_missing {
        existing.cloned().unwrap_or_default()
    } else {
        HashMap::new()
    };
    for (key, value) in incoming {
        let value = if value.is_empty() {
            existing
                .and_then(|values| values.get(&key))
                .cloned()
                .unwrap_or_default()
        } else {
            value
        };
        merged.insert(key, value);
    }
    merged
}

fn mcp_from_dto(m: McpServerDto, existing: Option<&McpServerConfig>) -> McpServerConfig {
    McpServerConfig {
        name: m.name,
        transport: if m.transport.is_empty() {
            "stdio".to_string()
        } else {
            m.transport
        },
        command: if m.command.is_empty() {
            existing
                .map(|server| server.command.clone())
                .unwrap_or_default()
        } else {
            m.command
        },
        args: if m.args.is_empty() {
            existing
                .map(|server| server.args.clone())
                .unwrap_or_default()
        } else {
            m.args
        },
        url: if m.url.is_empty() {
            existing
                .map(|server| server.url.clone())
                .unwrap_or_default()
        } else {
            m.url
        },
        env: merge_masked_values(m.env, existing.map(|server| &server.env), true),
        // The current settings form does not edit headers, so an omitted
        // header map must not silently erase stored authentication.
        headers: merge_masked_values(m.headers, existing.map(|server| &server.headers), true),
        enabled: m.enabled,
    }
}

#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<LlmSettingsDto, String> {
    let (settings, config_dir) = load_settings(&app)?;
    Ok(to_dto(&settings, config_dir))
}

#[tauri::command]
pub async fn is_configured(app: AppHandle) -> Result<bool, String> {
    let (settings, _) = load_settings(&app)?;
    Ok(settings.is_configured())
}

#[tauri::command]
pub async fn settings_secret_exists(
    app: AppHandle,
    reference: String,
) -> Result<SecretReferenceDto, String> {
    let (mut settings, _) = load_settings(&app)?;
    let exists = if let Some((kind, name, key)) = parse_mcp_map_reference(&reference) {
        let values = mcp_map_mut(&mut settings, kind, &name)
            .ok_or_else(|| "unknown secret reference".to_string())?;
        values.get(&key).is_some_and(|value| !value.is_empty())
    } else if let Some(args) = mcp_args_slot_mut(&mut settings, &reference) {
        !args.is_empty()
    } else {
        !secret_slot_mut(&mut settings, &reference)?.is_empty()
    };
    Ok(SecretReferenceDto { reference, exists })
}

#[tauri::command]
pub async fn settings_secret_replace(
    app: AppHandle,
    reference: String,
    value: String,
) -> Result<SecretReferenceDto, String> {
    let _write_guard = settings_write_guard().await;
    let (mut settings, _) = load_settings(&app)?;
    apply_secret_replace(&mut settings, &reference, value)?;
    settings.save().map_err(|error| error.to_string())?;
    Ok(SecretReferenceDto {
        reference,
        exists: true,
    })
}

fn apply_secret_replace(
    settings: &mut Settings,
    reference: &str,
    value: String,
) -> Result<(), String> {
    if value.is_empty() || value.len() > 64 * 1024 {
        return Err("secret replacement must be nonempty and at most 64 KiB".into());
    }
    if let Some((kind, name, key)) = parse_mcp_map_reference(&reference) {
        let values = mcp_map_mut(settings, kind, &name)
            .ok_or_else(|| "unknown secret reference".to_string())?;
        values.insert(key, value);
    } else if let Some(args) = mcp_args_slot_mut(settings, &reference) {
        *args = parse_mcp_args_replacement(&value)?;
    } else {
        *secret_slot_mut(settings, &reference)? = value;
    }
    Ok(())
}

#[tauri::command]
pub async fn settings_secret_delete(
    app: AppHandle,
    reference: String,
) -> Result<SecretReferenceDto, String> {
    let _write_guard = settings_write_guard().await;
    let (mut settings, _) = load_settings(&app)?;
    apply_secret_delete(&mut settings, &reference)?;
    settings.save().map_err(|error| error.to_string())?;
    Ok(SecretReferenceDto {
        reference,
        exists: false,
    })
}

fn apply_secret_delete(settings: &mut Settings, reference: &str) -> Result<(), String> {
    if let Some((kind, name, key)) = parse_mcp_map_reference(&reference) {
        let values = mcp_map_mut(settings, kind, &name)
            .ok_or_else(|| "unknown secret reference".to_string())?;
        values.remove(&key);
    } else if let Some(args) = mcp_args_slot_mut(settings, &reference) {
        args.clear();
    } else {
        secret_slot_mut(settings, &reference)?.clear();
    }
    Ok(())
}

fn validate_scalar_secret_updates(updates: &SaveLlmSettings) -> Result<(), String> {
    let global_keys = [
        &updates.anthropic_api_key,
        &updates.openai_api_key,
        &updates.deepseek_api_key,
        &updates.qwen_api_key,
        &updates.minimax_api_key,
        &updates.zhipu_api_key,
        &updates.kimi_api_key,
    ];
    if global_keys.iter().any(|value| !value.is_empty())
        || updates
            .llm_providers
            .iter()
            .any(|provider| !provider.api_key.is_empty())
    {
        return Err("model keys must use settings_secret_replace".into());
    }
    if updates.mcp_servers.iter().any(|server| {
        !server.command.is_empty() || !server.url.is_empty() || !server.args.is_empty()
    }) {
        return Err("MCP command, URL and args must use settings_secret_replace".into());
    }
    if updates.mcp_servers.iter().any(|server| {
        server.env.values().any(|value| !value.is_empty())
            || server.headers.values().any(|value| !value.is_empty())
    }) {
        return Err("MCP environment and header values must use settings_secret_replace".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    updates: SaveLlmSettings,
) -> Result<LlmSettingsDto, String> {
    validate_scalar_secret_updates(&updates)?;
    let _write_guard = settings_write_guard().await;
    let (mut settings, config_dir) = load_settings(&app)?;

    apply_settings_updates(&mut settings, updates);
    settings.save().map_err(|e| e.to_string())?;
    Ok(to_dto(&settings, config_dir))
}

/// Apply the settings form and its reference operations to one in-memory
/// snapshot, then persist it once. Validation errors leave the file untouched.
#[tauri::command]
pub async fn save_settings_with_secrets(
    app: AppHandle,
    updates: SaveLlmSettings,
    deletions: Vec<String>,
    replacements: Vec<SecretReplacement>,
) -> Result<LlmSettingsDto, String> {
    let _write_guard = settings_write_guard().await;
    let (mut settings, config_dir) = load_settings(&app)?;
    apply_settings_transaction(&mut settings, updates, deletions, replacements)?;
    settings.save().map_err(|error| error.to_string())?;
    Ok(to_dto(&settings, config_dir))
}

fn apply_settings_transaction(
    settings: &mut Settings,
    updates: SaveLlmSettings,
    deletions: Vec<String>,
    replacements: Vec<SecretReplacement>,
) -> Result<(), String> {
    validate_scalar_secret_updates(&updates)?;
    apply_settings_updates(settings, updates);
    for reference in deletions {
        apply_secret_delete(settings, &reference)?;
    }
    for replacement in replacements {
        apply_secret_replace(settings, &replacement.reference, replacement.value)?;
    }
    Ok(())
}

fn apply_settings_updates(settings: &mut Settings, updates: SaveLlmSettings) {
    settings.provider = updates.provider;
    settings.model = updates.model;
    settings.custom_base_url = updates.custom_base_url;
    settings.max_tokens = updates.max_tokens;
    settings.context_window = updates.context_window;
    settings.max_iterations = updates.max_iterations.clamp(1, 1_000);
    settings.policy_mode = updates.policy_mode;
    settings.enable_streaming = updates.enable_streaming;
    settings.temperature = updates.temperature;
    settings.top_p = updates.top_p;
    settings.thinking = updates.thinking;
    settings.vision_enabled = updates.vision_enabled;
    if updates.language == "zh" || updates.language == "en" {
        settings.language = updates.language;
    }

    let mut providers: Vec<LlmProviderConfig> = Vec::new();
    for item in updates.llm_providers {
        if item.id.trim().is_empty() {
            continue;
        }
        let existing_api_key = settings
            .llm_providers
            .iter()
            .find(|p| p.id == item.id)
            .map(|p| p.api_key.clone())
            .unwrap_or_default();
        providers.push(LlmProviderConfig {
            id: item.id,
            label: item.label,
            provider: item.provider,
            model: item.model,
            api_key: existing_api_key,
            base_url: item.base_url,
            max_tokens: item.max_tokens,
            temperature: item.temperature,
            top_p: item.top_p,
            thinking: item.thinking,
            stream: item.stream,
        });
    }
    settings.llm_providers = providers;

    // Keep legacy provider/model in sync when the user only maintains the
    // multi-provider list — headless Koi / IM turns read these fields.
    if settings.model.trim().is_empty() {
        if let Some(first) = settings.llm_providers.first() {
            settings.provider = first.provider.clone();
            settings.model = first.model.clone();
            if !first.base_url.trim().is_empty() {
                settings.custom_base_url = first.base_url.clone();
            }
            if first.max_tokens > 0 {
                settings.max_tokens = first.max_tokens;
            }
        }
    }

    let previous_mcp_servers = &settings.mcp_servers;
    settings.mcp_servers = updates
        .mcp_servers
        .into_iter()
        .filter(|m| !m.name.trim().is_empty())
        .map(|m| {
            let existing = previous_mcp_servers
                .iter()
                .find(|server| server.name == m.name);
            mcp_from_dto(m, existing)
        })
        .collect();
}

#[cfg(test)]
mod secret_response_tests {
    use super::*;

    #[test]
    fn settings_transaction_persists_all_operations_once_and_rejects_invalid_batch() {
        let directory = std::env::temp_dir().join(format!(
            "agentz-settings-transaction-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("config.json");
        let mut original = Settings {
            config_path: path.clone(),
            openai_api_key: "original-marker".into(),
            ..Settings::default()
        };
        original.save().unwrap();
        let old_bytes = std::fs::read(&path).unwrap();
        let mut updates: SaveLlmSettings = serde_json::from_value(
            serde_json::to_value(to_dto(&original, "fixture".into())).unwrap(),
        )
        .unwrap();
        updates.model = "atomic-model".into();
        let failed = apply_settings_transaction(
            &mut original,
            updates,
            vec!["model:openai".into()],
            vec![SecretReplacement {
                reference: "unknown".into(),
                value: "new-marker".into(),
            }],
        );
        assert!(failed.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), old_bytes);

        let mut current = Settings::load(&path).unwrap();
        let mut updates: SaveLlmSettings = serde_json::from_value(
            serde_json::to_value(to_dto(&current, "fixture".into())).unwrap(),
        )
        .unwrap();
        updates.model = "atomic-model".into();
        apply_settings_transaction(
            &mut current,
            updates,
            vec!["model:openai".into()],
            vec![SecretReplacement {
                reference: "model:openai".into(),
                value: "new-marker".into(),
            }],
        )
        .unwrap();
        current.save().unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.model, "atomic-model");
        assert_eq!(loaded.openai_api_key, "new-marker");
        let saved_bytes = std::fs::read_to_string(&path).unwrap();
        assert!(!saved_bytes.contains("new-marker"));
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn general_settings_save_rejects_inline_scalar_secrets() {
        let mut value =
            serde_json::to_value(to_dto(&Settings::default(), "fixture".into())).unwrap();
        let empty: SaveLlmSettings = serde_json::from_value(value.clone()).unwrap();
        assert!(validate_scalar_secret_updates(&empty).is_ok());
        value["openai_api_key"] = "synthetic".into();
        let global: SaveLlmSettings = serde_json::from_value(value.clone()).unwrap();
        assert!(validate_scalar_secret_updates(&global).is_err());
        value["openai_api_key"] = "".into();
        value["llm_providers"] = serde_json::json!([{
            "id": "named", "label": "Named", "provider": "openai", "model": "fixture",
            "api_key": "synthetic", "base_url": "", "max_tokens": 0
        }]);
        let named: SaveLlmSettings = serde_json::from_value(value).unwrap();
        assert!(validate_scalar_secret_updates(&named).is_err());

        let mut mcp_value =
            serde_json::to_value(to_dto(&Settings::default(), "fixture".into())).unwrap();
        mcp_value["mcp_servers"] = serde_json::json!([{
            "name": "local", "transport": "stdio", "command": "synthetic", "args": [],
            "url": "", "env": {}, "headers": {}, "enabled": true
        }]);
        let mcp: SaveLlmSettings = serde_json::from_value(mcp_value).unwrap();
        assert!(validate_scalar_secret_updates(&mcp).is_err());
        let mut args_value =
            serde_json::to_value(to_dto(&Settings::default(), "fixture".into())).unwrap();
        args_value["mcp_servers"] = serde_json::json!([{
            "name": "local", "transport": "stdio", "command": "", "args": ["synthetic"],
            "url": "", "env": {}, "headers": {}, "enabled": true
        }]);
        let args: SaveLlmSettings = serde_json::from_value(args_value).unwrap();
        assert!(validate_scalar_secret_updates(&args).is_err());
        let mut env_value =
            serde_json::to_value(to_dto(&Settings::default(), "fixture".into())).unwrap();
        env_value["mcp_servers"] = serde_json::json!([{
            "name": "local", "transport": "stdio", "command": "", "args": [],
            "url": "", "env": {"CUSTOM": "synthetic"}, "headers": {}, "enabled": true
        }]);
        let env: SaveLlmSettings = serde_json::from_value(env_value).unwrap();
        assert!(validate_scalar_secret_updates(&env).is_err());
    }

    #[test]
    fn settings_response_never_echoes_model_api_keys() {
        let marker = "AZ031_SYNTHETIC_SECRET_UNIQUE";
        let mut settings = Settings {
            anthropic_api_key: marker.into(),
            openai_api_key: marker.into(),
            deepseek_api_key: marker.into(),
            qwen_api_key: marker.into(),
            minimax_api_key: marker.into(),
            zhipu_api_key: marker.into(),
            kimi_api_key: marker.into(),
            ..Settings::default()
        };
        settings.llm_providers.push(LlmProviderConfig {
            id: "provider-one".into(),
            api_key: marker.into(),
            ..LlmProviderConfig::default()
        });
        let response = serde_json::to_string(&to_dto(&settings, "fixture".into())).unwrap();
        assert!(!response.contains(marker));
        assert!(response.contains("provider-one"));
        assert!(response.contains("model:openai"));
        assert!(response.contains("provider:provider-one"));
    }

    #[test]
    fn secret_references_support_independent_replace_exists_and_delete() {
        let mut settings = Settings::default();
        settings.llm_providers.push(LlmProviderConfig {
            id: "named".into(),
            ..LlmProviderConfig::default()
        });
        assert!(
            !secret_references(&settings)
                .iter()
                .find(|item| item.reference == "model:openai")
                .unwrap()
                .exists
        );
        *secret_slot_mut(&mut settings, "model:openai").unwrap() = "synthetic".into();
        *secret_slot_mut(&mut settings, "provider:named").unwrap() = "synthetic".into();
        assert!(
            secret_references(&settings)
                .iter()
                .find(|item| item.reference == "model:openai")
                .unwrap()
                .exists
        );
        assert!(
            secret_references(&settings)
                .iter()
                .find(|item| item.reference == "provider:named")
                .unwrap()
                .exists
        );
        secret_slot_mut(&mut settings, "model:openai")
            .unwrap()
            .clear();
        assert!(
            !secret_references(&settings)
                .iter()
                .find(|item| item.reference == "model:openai")
                .unwrap()
                .exists
        );
        assert!(secret_slot_mut(&mut settings, "provider:missing").is_err());
    }

    #[test]
    fn mcp_scalar_references_support_independent_lifecycle() {
        let mut settings = Settings::default();
        settings.mcp_servers.push(McpServerConfig {
            name: "local:fixture".into(),
            ..McpServerConfig::default()
        });
        assert!(
            !secret_references(&settings)
                .iter()
                .find(|item| item.reference == "mcp:local:fixture:command")
                .unwrap()
                .exists
        );
        *secret_slot_mut(&mut settings, "mcp:local:fixture:command").unwrap() = "synthetic".into();
        *secret_slot_mut(&mut settings, "mcp:local:fixture:url").unwrap() = "synthetic-url".into();
        *mcp_args_slot_mut(&mut settings, "mcp:local:fixture:args").unwrap() =
            vec!["synthetic".into()];
        assert!(
            secret_references(&settings)
                .iter()
                .find(|item| item.reference == "mcp:local:fixture:command")
                .unwrap()
                .exists
        );
        assert!(
            secret_references(&settings)
                .iter()
                .find(|item| item.reference == "mcp:local:fixture:url")
                .unwrap()
                .exists
        );
        assert!(
            secret_references(&settings)
                .iter()
                .find(|item| item.reference == "mcp:local:fixture:args")
                .unwrap()
                .exists
        );
        secret_slot_mut(&mut settings, "mcp:local:fixture:command")
            .unwrap()
            .clear();
        assert!(
            !secret_references(&settings)
                .iter()
                .find(|item| item.reference == "mcp:local:fixture:command")
                .unwrap()
                .exists
        );
        assert!(secret_slot_mut(&mut settings, "mcp:missing:command").is_err());
        assert!(secret_slot_mut(&mut settings, "mcp:local:fixture:args").is_err());
        mcp_args_slot_mut(&mut settings, "mcp:local:fixture:args")
            .unwrap()
            .clear();
        assert!(
            !secret_references(&settings)
                .iter()
                .find(|item| item.reference == "mcp:local:fixture:args")
                .unwrap()
                .exists
        );
        assert_eq!(
            parse_mcp_args_replacement("[\"one\",\"two\"]").unwrap(),
            vec!["one", "two"]
        );
        assert!(parse_mcp_args_replacement("[]").is_err());
        assert!(parse_mcp_args_replacement("[1]").is_err());
    }

    #[test]
    fn mcp_map_references_support_unusual_names_and_keys() {
        let reference = mcp_map_reference("env", "local:fixture", "CUSTOM/KEY");
        assert_eq!(
            parse_mcp_map_reference(&reference),
            Some(("env", "local:fixture".into(), "CUSTOM/KEY".into()))
        );
        let mut settings = Settings::default();
        settings.mcp_servers.push(McpServerConfig {
            name: "local:fixture".into(),
            ..McpServerConfig::default()
        });
        mcp_map_mut(&mut settings, "env", "local:fixture")
            .unwrap()
            .insert("CUSTOM/KEY".into(), "synthetic".into());
        assert!(
            secret_references(&settings)
                .iter()
                .find(|item| item.reference == reference)
                .unwrap()
                .exists
        );
        mcp_map_mut(&mut settings, "env", "local:fixture")
            .unwrap()
            .remove("CUSTOM/KEY");
        assert!(secret_references(&settings)
            .iter()
            .all(|item| item.reference != reference));
        let header = mcp_map_reference("header", "local:fixture", "Authorization");
        mcp_map_mut(&mut settings, "header", "local:fixture")
            .unwrap()
            .insert("Authorization".into(), "synthetic".into());
        assert!(
            secret_references(&settings)
                .iter()
                .find(|item| item.reference == header)
                .unwrap()
                .exists
        );
        assert!(parse_mcp_map_reference("mcp-env:broken").is_none());
    }

    #[test]
    fn mcp_response_masks_credentials_and_roundtrip_preserves_them() {
        let marker = "AZ031_MCP_SYNTHETIC_SECRET_UNIQUE";
        let mut settings = Settings::default();
        settings.mcp_servers.push(McpServerConfig {
            name: "fixture".into(),
            command: marker.into(),
            args: vec![marker.into()],
            url: format!("https://example.invalid/endpoint?token={marker}"),
            env: HashMap::from([("TOKEN".into(), marker.into())]),
            headers: HashMap::from([("Authorization".into(), marker.into())]),
            ..McpServerConfig::default()
        });
        let dto = to_dto(&settings, "fixture".into());
        assert!(!serde_json::to_string(&dto).unwrap().contains(marker));
        let previous = &settings.mcp_servers[0];
        let submitted = &dto.mcp_servers[0];
        assert!(submitted.args_configured);
        assert!(submitted.command_configured);
        assert!(submitted.url_configured);
        assert!(submitted.command.is_empty());
        assert!(submitted.args.is_empty());
        assert!(submitted.url.is_empty());
        assert_eq!(
            merge_masked_values(submitted.env.clone(), Some(&previous.env), true)["TOKEN"],
            marker
        );
        assert_eq!(
            merge_masked_values(HashMap::new(), Some(&previous.env), true)["TOKEN"],
            marker
        );
        assert_eq!(
            merge_masked_values(HashMap::new(), Some(&previous.headers), true)["Authorization"],
            marker
        );
        let restored = mcp_from_dto(submitted.clone(), Some(previous));
        assert_eq!(restored.args, previous.args);
        assert_eq!(restored.command, previous.command);
        assert_eq!(restored.url, previous.url);
        assert_eq!(restored.env, previous.env);
        assert_eq!(restored.headers, previous.headers);
    }
}
