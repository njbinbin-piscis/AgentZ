//! Slash commands — CodeBuddy-style `/command` presets stored under `{config}/commands/`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::commands::data_scope::resolve_global_config_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommandManifest {
    pub id: String,
    #[serde(default)]
    pub slash_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub description_zh: String,
    #[serde(default)]
    pub argument_hint: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub prompt_zh: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub source_plugin: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlashCommandInfo {
    pub id: String,
    pub slash_id: String,
    pub name: String,
    pub description: String,
    pub description_zh: String,
    pub argument_hint: String,
    pub tools: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlashCommandResolveResult {
    pub id: String,
    pub slash_id: String,
    pub display: String,
    pub prompt: String,
    pub tools: Vec<String>,
}

pub fn commands_root(config_dir: &Path) -> PathBuf {
    config_dir.join("commands")
}

fn command_dir(config_dir: &Path, id: &str) -> PathBuf {
    commands_root(config_dir).join(safe_id(id))
}

fn safe_id(id: &str) -> String {
    id.trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ':' || *c == '.')
        .collect()
}

pub fn load_manifest(path: &Path) -> Result<SlashCommandManifest, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("invalid command.json: {e}"))
}

pub fn list_commands_from_dir(config_dir: &Path) -> Vec<SlashCommandInfo> {
    let root = commands_root(config_dir);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("command.json");
        if !manifest_path.is_file() {
            continue;
        }
        let Ok(m) = load_manifest(&manifest_path) else {
            continue;
        };
        let slash_id = if m.slash_id.is_empty() {
            m.id.clone()
        } else {
            m.slash_id.clone()
        };
        out.push(SlashCommandInfo {
            id: m.id,
            slash_id,
            name: m.name,
            description: m.description,
            description_zh: m.description_zh,
            argument_hint: m.argument_hint,
            tools: m.tools,
            source: m.source,
        });
    }
    out.sort_by_key(|c| c.slash_id.to_lowercase());
    out
}

pub fn resolve_command(
    config_dir: &Path,
    raw_input: &str,
    prefer_zh: bool,
) -> Result<Option<SlashCommandResolveResult>, String> {
    let trimmed = raw_input.trim();
    if !trimmed.starts_with('/') {
        return Ok(None);
    }
    let body = trimmed.trim_start_matches('/').trim();
    if body.is_empty() {
        return Ok(None);
    }
    let (cmd_token, args) = match body.find(char::is_whitespace) {
        Some(i) => (body[..i].trim(), body[i..].trim()),
        None => (body, ""),
    };
    if cmd_token.is_empty() {
        return Ok(None);
    }
    let commands = list_commands_from_dir(config_dir);
    let hit = commands
        .iter()
        .find(|c| c.slash_id.eq_ignore_ascii_case(cmd_token) || c.id.eq_ignore_ascii_case(cmd_token));
    let Some(info) = hit else {
        return Ok(None);
    };
    let manifest_path = command_dir(config_dir, &info.id).join("command.json");
    let manifest = load_manifest(&manifest_path)?;
    let template = if prefer_zh && !manifest.prompt_zh.is_empty() {
        &manifest.prompt_zh
    } else {
        &manifest.prompt
    };
    let prompt = template
        .replace("$ARGUMENTS", args)
        .replace("$arguments", args);
    let display = if args.is_empty() {
        format!("/{}", info.slash_id)
    } else {
        format!("/{} {}", info.slash_id, args)
    };
    Ok(Some(SlashCommandResolveResult {
        id: info.id.clone(),
        slash_id: info.slash_id.clone(),
        display,
        prompt,
        tools: manifest.tools,
    }))
}

#[tauri::command]
pub fn slash_commands_list(app: AppHandle) -> Result<Vec<SlashCommandInfo>, String> {
    let config_dir = resolve_global_config_dir(&app)?;
    Ok(list_commands_from_dir(&config_dir))
}

#[tauri::command]
pub fn slash_commands_resolve(
    app: AppHandle,
    input: String,
    prefer_zh: Option<bool>,
) -> Result<Option<SlashCommandResolveResult>, String> {
    let config_dir = resolve_global_config_dir(&app)?;
    resolve_command(&config_dir, &input, prefer_zh.unwrap_or(false))
}

pub fn tool_allowlist_map(allow: &[String]) -> Option<std::collections::HashMap<String, bool>> {
    if allow.is_empty() {
        return None;
    }
    let allow_set: std::collections::HashSet<String> = allow.iter().cloned().collect();
    let mut map = std::collections::HashMap::new();
    for tool in crate::commands::agents::agents_list_builtin_tools() {
        map.insert(tool.id.clone(), allow_set.contains(&tool.id));
    }
    Some(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_substitutes_arguments() {
        let dir = std::env::temp_dir().join("agentz-slash-cmd-test");
        let _ = std::fs::remove_dir_all(&dir);
        let cmd_dir = dir.join("commands").join("test-cmd");
        std::fs::create_dir_all(&cmd_dir).unwrap();
        let manifest = SlashCommandManifest {
            id: "test-cmd".into(),
            slash_id: "review".into(),
            name: "Review".into(),
            description: String::new(),
            description_zh: String::new(),
            argument_hint: String::new(),
            tools: vec!["file_read".into()],
            prompt: "Review this: $ARGUMENTS".into(),
            prompt_zh: String::new(),
            source: "test".into(),
            source_plugin: String::new(),
        };
        std::fs::write(
            cmd_dir.join("command.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let resolved = resolve_command(&dir, "/review main branch", false)
            .unwrap()
            .unwrap();
        assert_eq!(resolved.prompt, "Review this: main branch");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
