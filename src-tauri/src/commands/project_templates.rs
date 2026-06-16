//! Project templates — optional `.agentz/rules` + `hooks.json` applied when opening a folder.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::warn;

use crate::commands::data_scope::resolve_global_config_dir;
use crate::commands::workbench::{HookDef, HooksConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTemplateMeta {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub name_zh: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub description_zh: String,
    #[serde(default)]
    pub source_plugin: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectTemplateInfo {
    pub id: String,
    pub name: String,
    pub name_zh: String,
    pub description: String,
    pub description_zh: String,
    pub source_plugin: String,
}

fn bundled_templates_root(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("preinstall").join("project-templates");
        if p.is_dir() {
            return Some(p);
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bundled/preinstall/project-templates");
    if dev.is_dir() {
        return Some(dev);
    }
    None
}

fn list_from_dir(root: &Path) -> Vec<ProjectTemplateInfo> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join("template.json");
        if !meta_path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&meta_path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<ProjectTemplateMeta>(&text) else {
            continue;
        };
        out.push(ProjectTemplateInfo {
            id: meta.id,
            name: meta.name,
            name_zh: meta.name_zh,
            description: meta.description,
            description_zh: meta.description_zh,
            source_plugin: meta.source_plugin,
        });
    }
    out.sort_by_key(|t| t.id.clone());
    out
}

fn copy_dir_if_absent(src: &Path, dest: &Path) {
    if !src.is_dir() {
        return;
    }
    std::fs::create_dir_all(dest).ok();
    for entry in std::fs::read_dir(src).into_iter().flatten().flatten() {
        let s = entry.path();
        let d = dest.join(entry.file_name());
        if s.is_dir() {
            copy_dir_if_absent(&s, &d);
        } else if !d.exists() {
            if let Err(e) = std::fs::copy(&s, &d) {
                warn!("template copy {}: {}", s.display(), e);
            }
        }
    }
}

fn convert_codebuddy_hooks(raw: &str) -> Result<HooksConfig, String> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let hooks_obj = v
        .get("hooks")
        .and_then(|h| h.as_object())
        .or_else(|| v.as_object());
    let Some(obj) = hooks_obj else {
        return Ok(HooksConfig::default());
    };
    let event_map = [
        ("PreToolUse", "beforeAgentTurn"),
        ("PostToolUse", "afterAgentTurn"),
        ("UserPromptSubmit", "beforeAgentTurn"),
        ("Stop", "afterAgentTurn"),
        ("SessionStart", "beforeAgentTurn"),
        ("SessionEnd", "afterAgentTurn"),
    ];
    let mut hooks = Vec::new();
    let mut idx = 0u32;
    for (cb_event, az_event) in event_map {
        let Some(arr) = obj.get(cb_event).and_then(|a| a.as_array()) else {
            continue;
        };
        for block in arr {
            let Some(inner) = block.get("hooks").and_then(|h| h.as_array()) else {
                continue;
            };
            for hook in inner {
                let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) else {
                    continue;
                };
                idx += 1;
                hooks.push(HookDef {
                    id: format!("tpl-{cb_event}-{idx}"),
                    name: format!("{cb_event} hook"),
                    event: az_event.to_string(),
                    command: cmd.replace("${AGENTZ_BUNDLE_ROOT}", "."),
                    enabled: false,
                });
            }
        }
    }
    Ok(HooksConfig {
        version: 1,
        hooks,
    })
}

#[tauri::command]
pub fn project_templates_list(app: AppHandle) -> Result<Vec<ProjectTemplateInfo>, String> {
    let Some(root) = bundled_templates_root(&app) else {
        return Ok(Vec::new());
    };
    Ok(list_from_dir(&root))
}

#[tauri::command]
pub fn project_apply_template(
    app: AppHandle,
    project_dir: String,
    template_id: String,
) -> Result<(), String> {
    let trimmed = project_dir.trim();
    if trimmed.is_empty() {
        return Err("project directory is empty".to_string());
    }
    let project = PathBuf::from(trimmed);
    if !project.is_dir() {
        return Err(format!("project directory not found: {trimmed}"));
    }
    let Some(root) = bundled_templates_root(&app) else {
        return Err("bundled project templates not found".to_string());
    };
    let tpl_dir = root.join(template_id.trim());
    if !tpl_dir.is_dir() {
        return Err(format!("unknown template: {template_id}"));
    }
    let agentz = project.join(".agentz");
    std::fs::create_dir_all(&agentz).map_err(|e| e.to_string())?;

    let rules_src = tpl_dir.join(".agentz").join("rules");
    if rules_src.is_dir() {
        copy_dir_if_absent(&rules_src, &agentz.join("rules"));
    }

    let hooks_src = tpl_dir.join(".agentz").join("hooks.json");
    let hooks_dest = agentz.join("hooks.json");
    if hooks_src.is_file() && !hooks_dest.exists() {
        let raw = std::fs::read_to_string(&hooks_src).map_err(|e| e.to_string())?;
        let converted = convert_codebuddy_hooks(&raw)?;
        let json = serde_json::to_string_pretty(&converted).map_err(|e| e.to_string())?;
        std::fs::write(&hooks_dest, json).map_err(|e| e.to_string())?;
    }

    let _ = resolve_global_config_dir(&app)?;
    Ok(())
}

#[tauri::command]
pub fn project_has_agentz(project_dir: String) -> Result<bool, String> {
    let trimmed = project_dir.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    Ok(PathBuf::from(trimmed).join(".agentz").is_dir())
}
