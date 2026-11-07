//! First-run seeding of built-in preinstall packs (Phase 5).
//!
//! On the first launch (guarded by a sentinel file in the config dir) this
//! writes a small set of starter **agents** (Architect / Coder / Reviewer +
//! Researcher / Writer) and **teams** (`fullstack-squad`, `research-duo`) into
//! `{config}/agents/<id>/agent.json` and `{config}/teams/<id>/team.json`.
//!
//! Seeding is non-destructive: an entry is only written when its target file is
//! absent, so user edits or deletions are never clobbered. The sentinel records
//! the seed version so the set can be extended in future releases without
//! re-creating things the user removed.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

use crate::commands::agents::{safe_id, AgentManifest};
use crate::commands::data_scope::resolve_global_config_dir;
use crate::commands::teams::TeamManifest;
use crate::commands::workflow::WorkflowGraph;

/// Bump when the built-in pack contents change to re-run the (non-destructive) seed.
const SEED_SENTINEL: &str = ".layered-seed-v3";
const PREINSTALL_SENTINEL: &str = ".preinstall-codebuddy-v1";
const PREINSTALL_COMPAT_SENTINEL: &str = ".preinstall-codebuddy-v2-compat";
const PREINSTALL_REBRAND_SENTINEL: &str = ".preinstall-agentz-v3-rebrand";
const PREINSTALL_ALIGN_SENTINEL: &str = ".preinstall-agentz-v4-align";

/// A demonstrable workflow team: coder ⇄ reviewer review loop that exits once
/// the reviewer's output contains "approved" (capped at 3 iterations).
fn code_review_workflow() -> Option<WorkflowGraph> {
    serde_json::from_value(serde_json::json!({
        "entry": "start",
        "max_total_steps": 40,
        "nodes": [
            { "id": "start", "type": "start", "label": "开始", "x": 60, "y": 200 },
            {
                "id": "review-loop",
                "type": "loop",
                "label": "评审循环",
                "x": 280,
                "y": 200,
                "guard": { "max_iterations": 3, "exit_when": "review contains approved" }
            },
            {
                "id": "code",
                "type": "agent",
                "label": "编码",
                "agent_id": "coder",
                "output_key": "code",
                "x": 520,
                "y": 100,
                "prompt_template": "实现下面的任务。如果有上一轮评审意见，请逐条修复。\n\n任务：{{goal}}\n\n上一轮代码：\n{{code}}\n\n上一轮评审意见：\n{{review}}"
            },
            {
                "id": "review",
                "type": "agent",
                "label": "评审",
                "agent_id": "reviewer",
                "output_key": "review",
                "x": 520,
                "y": 300,
                "prompt_template": "评审下面的实现是否正确、是否满足任务要求。若可以接受，请在回复中明确包含单词 approved；否则列出必须修改的问题清单。\n\n任务：{{goal}}\n\n实现：\n{{code}}"
            },
            { "id": "end", "type": "end", "label": "结束", "x": 760, "y": 200 }
        ],
        "edges": [
            { "from": "start", "to": "review-loop" },
            { "from": "review-loop", "to": "code", "label": "body" },
            { "from": "review-loop", "to": "end" },
            { "from": "code", "to": "review" },
            { "from": "review", "to": "review-loop" }
        ]
    }))
    .ok()
}

/// Build an agent manifest with sensible defaults for the seed set.
fn agent(
    id: &str,
    name: &str,
    role: &str,
    icon: &str,
    color: &str,
    description: &str,
    system_prompt: &str,
) -> AgentManifest {
    AgentManifest {
        id: id.to_string(),
        name: name.to_string(),
        role: role.to_string(),
        icon: icon.to_string(),
        color: color.to_string(),
        description: description.to_string(),
        description_zh: String::new(),
        system_prompt: system_prompt.to_string(),
        system_prompt_zh: String::new(),
        skills: Vec::new(),
        tools: Vec::new(),
        mcp_servers: Vec::new(),
        connectors: Vec::new(),
        llm_provider_id: None,
        max_iterations: 0,
        task_timeout_secs: 0,
        koi_id: None,
        tool_allowlist: Vec::new(),
        permission_mode: String::new(),
        delegable: false,
    }
}

fn builtin_agents() -> Vec<AgentManifest> {
    vec![
        agent(
            "architect",
            "架构师 Architect",
            "架构设计",
            "🏛️",
            "#7c6af7",
            "负责需求拆解、技术选型与系统设计，输出清晰的实现方案。",
            "你是资深软件架构师。先澄清需求与约束，再给出模块划分、接口契约与分步实现计划。优先考虑可维护性、扩展性与简洁性，避免过度设计。",
        ),
        agent(
            "coder",
            "工程师 Coder",
            "编码实现",
            "🛠️",
            "#26de81",
            "依据方案实现功能，编写整洁、可测试的代码并修复缺陷。",
            "你是高效的软件工程师。严格按既定方案与代码规范实现功能，保持改动聚焦，必要时补充测试，并在完成后简要说明关键改动与验证方式。",
        ),
        agent(
            "reviewer",
            "评审 Reviewer",
            "代码评审",
            "🔍",
            "#f7b731",
            "审查代码正确性、安全性与风格，给出可执行的改进建议。",
            "你是严谨的代码评审者。检查正确性、边界条件、安全性、性能与可读性，按严重程度分级列出问题，并给出具体修改建议而非泛泛而谈。",
        ),
        agent(
            "researcher",
            "研究员 Researcher",
            "调研分析",
            "📚",
            "#4ab3f7",
            "收集与核实资料，梳理事实与来源，形成结构化结论。",
            "你是细致的研究员。围绕问题系统检索与交叉验证信息，区分事实与推测，标注来源，输出结构化、可追溯的调研结论。",
        ),
        agent(
            "writer",
            "撰稿 Writer",
            "内容撰写",
            "✍️",
            "#fc5c65",
            "将调研与方案转化为清晰、准确、面向受众的文稿。",
            "你是专业撰稿人。根据受众与目标组织内容，做到结构清晰、表达准确、详略得当，并保持术语与风格一致。",
        ),
    ]
}

const FULLSTACK_ORG_SPEC: &str = r#"# 全栈小队 org_spec

## 角色
- architect：拆解需求、产出方案与接口契约。
- coder：按方案实现并自测。
- reviewer：评审实现，提出修改意见直至达标。

## 规则
- 由协调者按波次（waves）分派 todo：先设计、再实现、后评审。
- 每个成员只在自己职责范围内行动，产出需可被下一环节直接使用。
- reviewer 发现的问题以新 todo 形式回流给 coder，闭环后方可完成。

## 集成
- 最终交付由协调者汇总：方案、实现说明、评审结论。
"#;

const RESEARCH_ORG_SPEC: &str = r#"# 研究二人组 org_spec

## 角色
- researcher：检索、核实并结构化关键事实与来源。
- writer：基于调研结论撰写面向受众的成稿。

## 规则
- sequential 工作流：researcher 先完成调研并交付结构化要点，writer 再据此成文。
- writer 如发现信息缺口，回流问题给 researcher 补充。

## 集成
- 协调者交付：调研要点 + 终稿，并附主要来源。
"#;

fn builtin_teams() -> Vec<TeamManifest> {
    vec![
        TeamManifest {
            id: "fullstack-squad".to_string(),
            name: "全栈小队 Fullstack Squad".to_string(),
            description: "架构师 + 工程师 + 评审的端到端研发小队。".to_string(),
            mode: "swarm".to_string(),
            org_spec: FULLSTACK_ORG_SPEC.to_string(),
            members: vec![
                "architect".to_string(),
                "coder".to_string(),
                "reviewer".to_string(),
            ],
            workflow_hint: "waves".to_string(),
            workflow: None,
            task_timeout_secs: 0,
        },
        TeamManifest {
            id: "research-duo".to_string(),
            name: "研究二人组 Research Duo".to_string(),
            description: "研究员 + 撰稿的调研到成文小队。".to_string(),
            mode: "swarm".to_string(),
            org_spec: RESEARCH_ORG_SPEC.to_string(),
            members: vec!["researcher".to_string(), "writer".to_string()],
            workflow_hint: "sequential".to_string(),
            workflow: None,
            task_timeout_secs: 0,
        },
        TeamManifest {
            id: "code-review-loop".to_string(),
            name: "代码评审循环 Code Review Loop".to_string(),
            description: "工作流示例：编码 → 评审 → 不通过则回炉重写，最多 3 轮。".to_string(),
            mode: "workflow".to_string(),
            org_spec: String::new(),
            members: vec!["coder".to_string(), "reviewer".to_string()],
            workflow_hint: "review".to_string(),
            workflow: code_review_workflow(),
            task_timeout_secs: 0,
        },
    ]
}

fn write_if_absent(path: &Path, contents: &str) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("seed: cannot create {}: {}", parent.display(), e);
            return;
        }
    }
    if let Err(e) = std::fs::write(path, contents) {
        warn!("seed: cannot write {}: {}", path.display(), e);
    }
}

fn bundled_preinstall_root(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("preinstall");
        if p.is_dir() {
            return Some(p);
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bundled/preinstall");
    if dev.is_dir() {
        return Some(dev);
    }
    None
}

fn copy_file_if_absent(src: &Path, dest: &Path) {
    if dest.exists() {
        return;
    }
    if let Some(parent) = dest.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("seed: cannot create {}: {}", parent.display(), e);
            return;
        }
    }
    if let Err(e) = std::fs::copy(src, dest) {
        warn!("seed: copy {} → {}: {}", src.display(), dest.display(), e);
    }
}

fn copy_tree_if_absent(src: &Path, dest: &Path) {
    if !src.is_dir() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(dest) {
        warn!("seed: cannot create {}: {}", dest.display(), e);
        return;
    }
    let Ok(entries) = std::fs::read_dir(src) else {
        return;
    };
    for entry in entries.flatten() {
        let s = entry.path();
        let d = dest.join(entry.file_name());
        if s.is_dir() {
            copy_tree_if_absent(&s, &d);
        } else if !d.exists() {
            copy_file_if_absent(&s, &d);
        }
    }
}

fn seed_preinstall_resources(app: &AppHandle, config_dir: &Path) {
    let Some(bundle) = bundled_preinstall_root(app) else {
        warn!("seed: bundled preinstall directory not found");
        return;
    };

    let skills_root = crate::skills::service::skills_root_from_config_dir(config_dir);
    let _ = crate::skills::provenance::ensure_evolution_dirs(&skills_root);
    let skills_dest = crate::skills::provenance::installed_dir(&skills_root);
    let skills_src = bundle.join("skills");
    if skills_src.is_dir() {
        for entry in std::fs::read_dir(&skills_src)
            .into_iter()
            .flatten()
            .flatten()
        {
            if entry.path().is_dir() {
                let slug = entry.file_name().to_string_lossy().to_string();
                copy_tree_if_absent(&entry.path(), &skills_dest.join(&slug));
            }
        }
    }

    let agents_src = bundle.join("agents");
    if agents_src.is_dir() {
        let agents_dest = config_dir.join("agents");
        for entry in std::fs::read_dir(&agents_src)
            .into_iter()
            .flatten()
            .flatten()
        {
            if !entry.path().is_dir() {
                continue;
            }
            let agent_json = entry.path().join("agent.json");
            if agent_json.is_file() {
                let id = entry.file_name().to_string_lossy().to_string();
                copy_file_if_absent(
                    &agent_json,
                    &agents_dest.join(safe_id(&id)).join("agent.json"),
                );
            }
        }
    }

    let commands_src = bundle.join("commands");
    if commands_src.is_dir() {
        let commands_dest = config_dir.join("commands");
        for entry in std::fs::read_dir(&commands_src)
            .into_iter()
            .flatten()
            .flatten()
        {
            if !entry.path().is_dir() {
                continue;
            }
            let cmd_json = entry.path().join("command.json");
            if cmd_json.is_file() {
                let id = entry.file_name().to_string_lossy().to_string();
                copy_file_if_absent(
                    &cmd_json,
                    &commands_dest.join(safe_id(&id)).join("command.json"),
                );
            }
        }
    }

    info!(
        "seed: copied CodeBuddy preinstall from {}",
        bundle.display()
    );
}

#[derive(Debug, Deserialize, Default)]
struct ExcludeManifest {
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    agents: Vec<String>,
    #[serde(default)]
    commands: Vec<String>,
}

fn bundled_codebuddy_root(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("codebuddy");
        if p.is_dir() {
            return Some(p);
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bundled/codebuddy");
    if dev.is_dir() {
        return Some(dev);
    }
    None
}

fn load_exclude_manifest(app: &AppHandle) -> Option<ExcludeManifest> {
    let root = bundled_codebuddy_root(app)?;
    let path = root.join("exclude.json");
    if !path.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

fn is_codebuddy_agent(path: &Path) -> bool {
    #[derive(Deserialize)]
    struct Partial {
        #[serde(default)]
        source: String,
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    serde_json::from_str::<Partial>(&text)
        .ok()
        .map(|p| p.source == "codebuddy")
        .unwrap_or(false)
}

fn is_codebuddy_command(path: &Path) -> bool {
    is_codebuddy_agent(path)
}

/// Remove deprecated CodeBuddy preinstall entries listed in bundled exclude.json.
pub fn seed_preinstall_compat_cleanup(app: &AppHandle) {
    let config_dir = match resolve_global_config_dir(app) {
        Ok(d) => d,
        Err(e) => {
            warn!("seed: compat cleanup cannot resolve config dir: {}", e);
            return;
        }
    };
    let sentinel = config_dir.join(PREINSTALL_COMPAT_SENTINEL);
    if sentinel.exists() {
        return;
    }
    let Some(exclude) = load_exclude_manifest(app) else {
        return;
    };

    let skills_root = crate::skills::service::skills_root_from_config_dir(&config_dir);
    let skills_installed = crate::skills::provenance::installed_dir(&skills_root);
    for slug in &exclude.skills {
        let dir = skills_installed.join(slug);
        if dir.is_dir() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    for id in &exclude.agents {
        let dir = config_dir.join("agents").join(safe_id(id));
        let manifest = dir.join("agent.json");
        if manifest.is_file() && is_codebuddy_agent(&manifest) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    for id in &exclude.commands {
        let dir = config_dir.join("commands").join(safe_id(id));
        let manifest = dir.join("command.json");
        if manifest.is_file() && is_codebuddy_command(&manifest) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    if let Err(e) = std::fs::write(&sentinel, b"1") {
        warn!("seed: cannot write compat sentinel: {}", e);
    } else {
        info!("seed: applied CodeBuddy preinstall compat cleanup");
    }
}

/// True when `path` is a small text file that still carries the old vendor
/// branding (case-insensitive "codebuddy"). Binary or oversized files are
/// never considered stale content.
fn file_still_branded(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() > 2 * 1024 * 1024 {
        return false;
    }
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    if bytes.contains(&0) {
        return false;
    }
    String::from_utf8_lossy(&bytes)
        .to_lowercase()
        .contains("codebuddy")
}

/// Refresh one installed skill directory from its bundled counterpart:
/// overwrite files that still carry the old branding, add files the
/// installed copy lacks, and leave anything else (user edits) untouched.
fn refresh_skill_dir(bundle_dir: &Path, installed_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(bundle_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let src = entry.path();
        let dest = installed_dir.join(entry.file_name());
        if src.is_dir() {
            if dest.is_dir() {
                refresh_skill_dir(&src, &dest);
            } else {
                copy_tree_if_absent(&src, &dest);
            }
        } else if !dest.exists() {
            copy_file_if_absent(&src, &dest);
        } else if file_still_branded(&dest) {
            if let Err(e) = std::fs::copy(&src, &dest) {
                warn!("seed: rebrand refresh {}: {}", dest.display(), e);
            }
        }
    }
}

/// One-time refresh for installs seeded before the skill pack was
/// rebranded: bundled preinstall skills no longer mention the old vendor,
/// but seeded copies were written once and never updated. Overwrite only
/// installed files that still carry the old branding; user-created or
/// user-edited content without it is left alone. Also swaps the renamed
/// `codebuddy-md-improver` skill for its `agentz-md-improver` successor.
pub fn seed_preinstall_rebrand_refresh(app: &AppHandle) {
    let config_dir = match resolve_global_config_dir(app) {
        Ok(d) => d,
        Err(e) => {
            warn!("seed: rebrand refresh cannot resolve config dir: {}", e);
            return;
        }
    };
    let sentinel = config_dir.join(PREINSTALL_REBRAND_SENTINEL);
    if sentinel.exists() {
        return;
    }
    let Some(bundle) = bundled_preinstall_root(app) else {
        return;
    };
    let skills_src = bundle.join("skills");
    if !skills_src.is_dir() {
        return;
    }
    let skills_root = crate::skills::service::skills_root_from_config_dir(&config_dir);
    let skills_installed = crate::skills::provenance::installed_dir(&skills_root);

    if let Ok(entries) = std::fs::read_dir(&skills_src) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let slug = entry.file_name().to_string_lossy().to_string();
            let dest = skills_installed.join(&slug);
            if dest.is_dir() {
                refresh_skill_dir(&entry.path(), &dest);
            }
        }
    }

    // Renamed skill: install the successor only when the old one is present
    // (never resurrect a skill the user deliberately deleted), then drop it.
    let old_dir = skills_installed.join("codebuddy-md-improver");
    let new_src = skills_src.join("agentz-md-improver");
    if old_dir.is_dir() && new_src.is_dir() {
        let new_dest = skills_installed.join("agentz-md-improver");
        if !new_dest.exists() {
            copy_tree_if_absent(&new_src, &new_dest);
        }
        let _ = std::fs::remove_dir_all(&old_dir);
    }

    if let Err(e) = std::fs::write(&sentinel, b"1") {
        warn!("seed: cannot write rebrand sentinel: {}", e);
    } else {
        info!("seed: applied preinstall skill rebrand refresh");
    }
}

#[derive(Debug, Deserialize, Default)]
struct RenameMaps {
    #[serde(default)]
    agents: std::collections::HashMap<String, String>,
    #[serde(default)]
    commands: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize, Default)]
struct SuccessorMaps {
    #[serde(default)]
    skills: std::collections::HashMap<String, String>,
}

/// What the AgentZ alignment of the bundled packs retired or renamed
/// (`bundled/codebuddy/retired.json`).
#[derive(Debug, Deserialize, Default)]
struct RetiredManifest {
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    agents: Vec<String>,
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    renames: RenameMaps,
    #[serde(default)]
    successors: SuccessorMaps,
}

fn source_is_preinstall(value: &str) -> bool {
    let v = value
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_ascii_lowercase();
    v == "codebuddy" || v == "agentz"
}

/// True when an installed agent/command manifest was seeded by the preinstall
/// pack (its `source` is a bundle marker) rather than created by the user.
fn json_is_preinstall_origin(path: &Path) -> bool {
    #[derive(Deserialize)]
    struct Partial {
        #[serde(default)]
        source: String,
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Partial>(&t).ok())
        .map(|p| source_is_preinstall(&p.source))
        .unwrap_or(false)
}

/// Same check for a skill: the `source:` line of the SKILL.md frontmatter.
/// A missing SKILL.md counts as a leftover of the pack.
fn skill_is_preinstall_origin(skill_dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(skill_dir.join("SKILL.md")) else {
        return true;
    };
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return false;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("source:") {
            return source_is_preinstall(rest);
        }
    }
    false
}

/// Move `path` into `{config}/preinstall-retired/<kind>/` so nothing is
/// lost. Returns true when `path` no longer exists afterwards.
fn retire_to_backup(config_dir: &Path, kind: &str, id: &str, path: &Path) -> bool {
    let base = config_dir.join("preinstall-retired").join(kind);
    if std::fs::create_dir_all(&base).is_err() {
        return false;
    }
    let clean: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut dest = base.join(&clean);
    let mut n = 1;
    while dest.exists() {
        n += 1;
        dest = base.join(format!("{clean}-{n}"));
    }
    if std::fs::rename(path, &dest).is_ok() {
        return true;
    }
    copy_tree_if_absent(path, &dest);
    std::fs::remove_dir_all(path).is_ok()
}

fn read_bytes_eq(a: &Path, b: &Path) -> bool {
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn align_json_items(
    config_dir: &Path,
    bundle: &Path,
    kind: &str,
    file: &str,
    renames: &std::collections::HashMap<String, String>,
    retired: &[String],
) {
    let installed = config_dir.join(kind);
    for (old, new) in renames {
        let old_dir = installed.join(safe_id(old));
        let manifest = old_dir.join(file);
        if !manifest.is_file() || !json_is_preinstall_origin(&manifest) {
            continue;
        }
        copy_file_if_absent(
            &bundle.join(kind).join(new).join(file),
            &installed.join(safe_id(new)).join(file),
        );
        retire_to_backup(config_dir, kind, old, &old_dir);
    }
    for old in retired {
        let old_dir = installed.join(safe_id(old));
        let manifest = old_dir.join(file);
        if manifest.is_file() && json_is_preinstall_origin(&manifest) {
            retire_to_backup(config_dir, kind, old, &old_dir);
        }
    }
}

fn rename_ids_in_value(
    v: &mut serde_json::Value,
    map: &std::collections::HashMap<String, String>,
) -> bool {
    let mut changed = false;
    match v {
        serde_json::Value::Object(obj) => {
            for (k, val) in obj.iter_mut() {
                if k == "agent_id" {
                    if let Some(new) = val.as_str().and_then(|s| map.get(s)) {
                        *val = serde_json::Value::String(new.clone());
                        changed = true;
                        continue;
                    }
                }
                if k == "members" {
                    if let serde_json::Value::Array(items) = val {
                        for item in items.iter_mut() {
                            if let Some(new) = item.as_str().and_then(|s| map.get(s)) {
                                *item = serde_json::Value::String(new.clone());
                                changed = true;
                            }
                        }
                        continue;
                    }
                }
                changed |= rename_ids_in_value(val, map);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                changed |= rename_ids_in_value(item, map);
            }
        }
        _ => {}
    }
    changed
}

/// Apply the AgentZ alignment of the bundled packs to an existing install:
/// retire removed items, rename `cb-*` agents/commands, refresh rewritten
/// skills. Everything replaced or removed is moved to
/// `{config}/preinstall-retired/`; items the user created are never touched.
fn apply_preinstall_align(config_dir: &Path, bundle: &Path, retired: &RetiredManifest) {
    let skills_root = crate::skills::service::skills_root_from_config_dir(config_dir);
    let skills_installed = crate::skills::provenance::installed_dir(&skills_root);

    for (old, new) in &retired.successors.skills {
        let old_dir = skills_installed.join(old);
        if old_dir.is_dir() {
            copy_tree_if_absent(&bundle.join("skills").join(new), &skills_installed.join(new));
            if skill_is_preinstall_origin(&old_dir) {
                retire_to_backup(config_dir, "skills", old, &old_dir);
            }
        }
    }
    for slug in &retired.skills {
        let dir = skills_installed.join(slug);
        if dir.is_dir() && skill_is_preinstall_origin(&dir) {
            retire_to_backup(config_dir, "skills", slug, &dir);
        }
    }
    if let Ok(entries) = std::fs::read_dir(bundle.join("skills")) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let slug = entry.file_name().to_string_lossy().to_string();
            let dest = skills_installed.join(&slug);
            if !dest.is_dir()
                || !skill_is_preinstall_origin(&dest)
                || read_bytes_eq(&entry.path().join("SKILL.md"), &dest.join("SKILL.md"))
            {
                continue;
            }
            if retire_to_backup(config_dir, "skills", &slug, &dest) {
                copy_tree_if_absent(&entry.path(), &dest);
            }
        }
    }

    align_json_items(
        config_dir,
        bundle,
        "agents",
        "agent.json",
        &retired.renames.agents,
        &retired.agents,
    );
    align_json_items(
        config_dir,
        bundle,
        "commands",
        "command.json",
        &retired.renames.commands,
        &retired.commands,
    );

    // Teams reference member agents by id: follow the renames.
    if let Ok(entries) = std::fs::read_dir(config_dir.join("teams")) {
        for entry in entries.flatten() {
            let file = entry.path().join("team.json");
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if rename_ids_in_value(&mut value, &retired.renames.agents) {
                if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                    let _ = std::fs::write(&file, pretty);
                }
            }
        }
    }
}

/// One-time alignment of already-seeded installs with the AgentZ-ified packs.
pub fn seed_preinstall_align(app: &AppHandle) {
    let config_dir = match resolve_global_config_dir(app) {
        Ok(d) => d,
        Err(e) => {
            warn!("seed: align cannot resolve config dir: {}", e);
            return;
        }
    };
    let sentinel = config_dir.join(PREINSTALL_ALIGN_SENTINEL);
    if sentinel.exists() {
        return;
    }
    let (Some(bundle), Some(root)) = (bundled_preinstall_root(app), bundled_codebuddy_root(app))
    else {
        return;
    };
    let retired = std::fs::read_to_string(root.join("retired.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<RetiredManifest>(&t).ok());
    let Some(retired) = retired else {
        warn!("seed: retired.json missing or invalid; skipping align");
        return;
    };
    apply_preinstall_align(&config_dir, &bundle, &retired);
    if let Err(e) = std::fs::write(&sentinel, b"1") {
        warn!("seed: cannot write align sentinel: {}", e);
    } else {
        info!("seed: aligned preinstall packs with AgentZ");
    }
}

/// Copy bundled CodeBuddy resources once (skills / agents / commands).
pub fn seed_preinstall_packs(app: &AppHandle) {
    let config_dir = match resolve_global_config_dir(app) {
        Ok(d) => d,
        Err(e) => {
            warn!("seed: cannot resolve config dir: {}", e);
            return;
        }
    };
    let sentinel = config_dir.join(PREINSTALL_SENTINEL);
    if sentinel.exists() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        warn!("seed: cannot create config dir: {}", e);
        return;
    }
    seed_preinstall_resources(app, &config_dir);
    if let Err(e) = std::fs::write(&sentinel, b"1") {
        warn!("seed: cannot write preinstall sentinel: {}", e);
    }
}

/// Seed built-in agent/team packs on first run. Idempotent and non-destructive.
pub fn seed_builtin_packs(app: &AppHandle) {
    let config_dir = match resolve_global_config_dir(app) {
        Ok(d) => d,
        Err(e) => {
            warn!("seed: cannot resolve config dir: {}", e);
            return;
        }
    };
    let sentinel = config_dir.join(SEED_SENTINEL);
    if sentinel.exists() {
        return;
    }

    let agents_dir = config_dir.join("agents");
    for manifest in builtin_agents() {
        let target = agents_dir.join(safe_id(&manifest.id)).join("agent.json");
        match serde_json::to_string_pretty(&manifest) {
            Ok(json) => write_if_absent(&target, &json),
            Err(e) => warn!("seed: serialize agent {} failed: {}", manifest.id, e),
        }
    }

    let teams_dir = config_dir.join("teams");
    for manifest in builtin_teams() {
        let target = teams_dir.join(safe_id(&manifest.id)).join("team.json");
        match serde_json::to_string_pretty(&manifest) {
            Ok(json) => write_if_absent(&target, &json),
            Err(e) => warn!("seed: serialize team {} failed: {}", manifest.id, e),
        }
    }

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        warn!("seed: cannot create config dir: {}", e);
        return;
    }
    if let Err(e) = std::fs::write(&sentinel, b"1") {
        warn!("seed: cannot write sentinel: {}", e);
    } else {
        info!("seed: installed built-in agent/team packs");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "agentz-seed-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn retired_fixture() -> RetiredManifest {
        serde_json::from_str(
            r#"{
              "skills": ["gone-skill"],
              "agents": ["cb-gone-agent"],
              "commands": ["cb-gone-cmd"],
              "renames": {
                "agents": {"cb-foo": "foo"},
                "commands": {"cb-run": "run"}
              },
              "successors": {"skills": {"old-improver": "rules-improver"}}
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn align_renames_retires_and_never_touches_user_items() {
        let root = temp_root("align");
        let cfg = root.join("cfg");
        let bundle = root.join("bundle");
        let skills_root = crate::skills::service::skills_root_from_config_dir(&cfg);
        let installed = crate::skills::provenance::installed_dir(&skills_root);

        // Bundle (new content).
        write(&bundle.join("agents/foo/agent.json"), r#"{"id":"foo","source":"agentz"}"#);
        write(&bundle.join("commands/run/command.json"), r#"{"id":"run","source":"agentz"}"#);
        write(
            &bundle.join("skills/rules-improver/SKILL.md"),
            "---\nname: rules-improver\nsource: agentz\n---\nnew",
        );
        write(
            &bundle.join("skills/kept/SKILL.md"),
            "---\nname: kept\nsource: agentz\n---\nrewritten",
        );

        // Installed (old content).
        write(&cfg.join("agents/cb-foo/agent.json"), r#"{"id":"cb-foo","source":"codebuddy"}"#);
        write(&cfg.join("agents/cb-gone-agent/agent.json"), r#"{"source":"codebuddy"}"#);
        write(&cfg.join("agents/cb-mine/agent.json"), r#"{"id":"cb-mine","source":"user"}"#);
        write(&cfg.join("commands/cb-run/command.json"), r#"{"source":"codebuddy"}"#);
        write(&cfg.join("commands/cb-gone-cmd/command.json"), r#"{"source":"codebuddy"}"#);
        write(&installed.join("gone-skill/SKILL.md"), "---\nsource: codebuddy\n---\nx");
        write(&installed.join("old-improver/SKILL.md"), "---\nsource: AgentZ\n---\nx");
        write(&installed.join("kept/SKILL.md"), "---\nname: kept\nsource: codebuddy\n---\nold");
        write(&installed.join("mine/SKILL.md"), "---\nname: mine\nsource: user\n---\nmine");
        write(
            &cfg.join("teams/t/team.json"),
            r#"{"id":"t","members":["cb-foo","other"]}"#,
        );

        apply_preinstall_align(&cfg, &bundle, &retired_fixture());

        assert!(cfg.join("agents/foo/agent.json").is_file());
        assert!(!cfg.join("agents/cb-foo").exists());
        assert!(!cfg.join("agents/cb-gone-agent").exists());
        assert!(cfg.join("agents/cb-mine/agent.json").is_file(), "user agent kept");
        assert!(cfg.join("commands/run/command.json").is_file());
        assert!(!cfg.join("commands/cb-run").exists());
        assert!(!cfg.join("commands/cb-gone-cmd").exists());

        assert!(!installed.join("gone-skill").exists());
        assert!(!installed.join("old-improver").exists());
        assert!(installed.join("rules-improver/SKILL.md").is_file(), "successor installed");
        let kept = std::fs::read_to_string(installed.join("kept/SKILL.md")).unwrap();
        assert!(kept.contains("rewritten"));
        assert!(installed.join("mine/SKILL.md").is_file(), "user skill kept");
        let mine = std::fs::read_to_string(installed.join("mine/SKILL.md")).unwrap();
        assert!(mine.contains("mine"));

        // Everything replaced is recoverable.
        let backup = cfg.join("preinstall-retired");
        assert!(backup.join("agents/cb-foo/agent.json").is_file());
        assert!(backup.join("agents/cb-gone-agent/agent.json").is_file());
        assert!(backup.join("skills/gone-skill/SKILL.md").is_file());
        let old_kept = std::fs::read_to_string(backup.join("skills/kept/SKILL.md")).unwrap();
        assert!(old_kept.contains("old"));

        let team: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(cfg.join("teams/t/team.json")).unwrap())
                .unwrap();
        assert_eq!(team["members"], serde_json::json!(["foo", "other"]));

        // Second run is a no-op.
        apply_preinstall_align(&cfg, &bundle, &retired_fixture());
        assert!(cfg.join("agents/foo/agent.json").is_file());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn shipped_retired_manifest_matches_bundle() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bundled");
        let text = std::fs::read_to_string(base.join("codebuddy/retired.json")).unwrap();
        let r: RetiredManifest = serde_json::from_str(&text).unwrap();
        let pre = base.join("preinstall");
        for (old, new) in &r.renames.agents {
            assert!(pre.join("agents").join(new).join("agent.json").is_file(), "{old}->{new}");
        }
        for (old, new) in &r.renames.commands {
            assert!(pre.join("commands").join(new).join("command.json").is_file(), "{old}->{new}");
        }
        for id in &r.agents {
            assert!(!pre.join("agents").join(id).exists(), "retired agent still bundled: {id}");
        }
        for id in &r.commands {
            assert!(!pre.join("commands").join(id).exists(), "retired command still bundled: {id}");
        }
        for s in &r.skills {
            assert!(!pre.join("skills").join(s).exists(), "retired skill still bundled: {s}");
        }
        for new in r.successors.skills.values() {
            assert!(pre.join("skills").join(new).join("SKILL.md").is_file(), "successor {new}");
        }
    }

    #[test]
    fn align_leaves_identical_skills_alone() {
        let root = temp_root("align-same");
        let cfg = root.join("cfg");
        let bundle = root.join("bundle");
        let skills_root = crate::skills::service::skills_root_from_config_dir(&cfg);
        let installed = crate::skills::provenance::installed_dir(&skills_root);
        let body = "---\nname: same\nsource: agentz\n---\nbody";
        write(&bundle.join("skills/same/SKILL.md"), body);
        write(&installed.join("same/SKILL.md"), body);

        apply_preinstall_align(&cfg, &bundle, &RetiredManifest::default());

        assert!(installed.join("same/SKILL.md").is_file());
        assert!(!cfg.join("preinstall-retired").exists(), "no needless backup");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn builtin_workflow_team_is_valid() {
        let graph = code_review_workflow().expect("workflow JSON deserializes");
        graph.validate().expect("built-in workflow graph is valid");
        // Loop body + exit edges and agent ids are present.
        assert_eq!(graph.entry, "start");
        assert!(graph.nodes.iter().any(|n| n.kind == "loop"));
        assert!(graph.nodes.iter().filter(|n| n.kind == "agent").all(|n| n
            .agent_id
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)));
    }
}
