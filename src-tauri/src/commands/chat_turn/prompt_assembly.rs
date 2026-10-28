use std::path::{Path, PathBuf};

use crate::browser::BrowserManager;
use crate::commands::agents::AgentManifest;
use crate::commands::system_prompt::{
    active_todo_context, agent_active_plan_context, agent_system_prompt, plan_mode_context,
    session_plan_rel_path, swarm_coordinator_append,
};
use crate::commands::teams::TeamManifest;
use piscis_kernel::agent::plan::PlanStore;

const MAX_FILE_REF_CHARS: usize = 12_000;
const MAX_TOTAL_REF_CHARS: usize = 48_000;

/// A parsed installed skill (SKILL.md + frontmatter), including its optional
/// `tools` / `mcp_servers` bindings (Phase 1).
pub(crate) struct SkillManifest {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub path: std::path::PathBuf,
    /// Builtin tool names this skill wants enabled while active.
    pub tools: Vec<String>,
    /// MCP server names (from `settings.mcp_servers`) this skill binds.
    pub mcp_servers: Vec<String>,
    /// Full SKILL.md body with the YAML frontmatter stripped.
    pub body: String,
}

/// Load installed + learned skills from quadrant storage.
pub(crate) fn load_installed_skills(config_dir: &std::path::Path) -> Vec<SkillManifest> {
    let root = crate::skills::service::skills_root_from_config_dir(config_dir);
    let mut loader = crate::skills::loader::SkillLoader::new(&root);
    let _ = loader.load_all();
    let mut out = Vec::new();
    for skill in loader.list_skills() {
        if skill.lifecycle != crate::skills::provenance::LIFECYCLE_INSTALLED
            && skill.lifecycle != crate::skills::provenance::LIFECYCLE_LEARNED
        {
            continue;
        }
        let skill_md = skill.source_path.join("SKILL.md");
        let slug = skill.skill_id.clone();
        out.push(SkillManifest {
            slug,
            name: skill.name.clone(),
            description: skill.description.clone(),
            path: skill_md,
            tools: skill.tools.clone(),
            mcp_servers: Vec::new(),
            body: skill.instructions.clone(),
        });
    }
    out.sort_by_key(|m| m.name.to_lowercase());
    out
}

/// True when `needle` matches a skill by slug or display name (case-insensitive).
pub(super) fn skill_matches(skill: &SkillManifest, needle: &str) -> bool {
    needle.eq_ignore_ascii_case(&skill.slug) || needle.eq_ignore_ascii_case(&skill.name)
}

/// Build the "## ... skills" system block.
pub(super) fn skills_context(config_dir: &std::path::Path, enabled: &[String]) -> Option<String> {
    let skills = load_installed_skills(config_dir);
    if skills.is_empty() {
        return None;
    }

    if enabled.is_empty() {
        let lines: Vec<String> = skills
            .iter()
            .map(|s| {
                format!(
                    "- **{}** — {}\n  (read `{}` for full instructions before using)",
                    s.name,
                    s.description,
                    s.path.display()
                )
            })
            .collect();
        return Some(format!(
            "## Available skills\nYou have these installed skills. When a task matches one, \
             read its SKILL.md first, then follow it:\n{}",
            lines.join("\n")
        ));
    }

    let selected: Vec<&SkillManifest> = skills
        .iter()
        .filter(|s| enabled.iter().any(|e| skill_matches(s, e)))
        .collect();
    if selected.is_empty() {
        return None;
    }
    let blocks: Vec<String> = selected
        .iter()
        .map(|s| format!("### Skill: {}\n{}", s.name, s.body))
        .collect();
    Some(format!(
        "## Enabled skills (user-selected for this conversation)\nThe user explicitly \
         enabled these skills for this conversation. Follow their instructions \
         carefully and use the tools they prescribe:\n\n{}",
        blocks.join("\n\n---\n\n")
    ))
}

/// Read project rules from `{workspace}/.agentz/rules/` (preferred) or
/// `{workspace}/.cursor/rules/` (compat).
pub(super) fn project_rules_context(workspace_root: &str) -> Option<String> {
    let root = std::path::Path::new(workspace_root.trim());
    if workspace_root.trim().is_empty() {
        return None;
    }
    let candidates = [
        root.join(".agentz").join("rules"),
        root.join(".cursor").join("rules"),
    ];
    let mut blocks = Vec::new();
    for dir in candidates.iter() {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut files: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                matches!(
                    p.extension().and_then(|e| e.to_str()),
                    Some("md") | Some("mdc")
                )
            })
            .collect();
        files.sort();
        for f in files {
            if let Ok(content) = std::fs::read_to_string(&f) {
                if !content.trim().is_empty() {
                    blocks.push(content.trim().to_string());
                }
            }
        }
        if !blocks.is_empty() {
            break;
        }
    }
    if blocks.is_empty() {
        return None;
    }
    Some(format!("## Project rules\n{}", blocks.join("\n\n")))
}

/// Read-only dependency source directories (Cargo git checkouts and registry).
pub(super) fn dependency_source_roots() -> Vec<PathBuf> {
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(|h| PathBuf::from(h).join(".cargo"))
        });
    let Some(home) = cargo_home else {
        return Vec::new();
    };
    ["git/checkouts", "registry/src"]
        .iter()
        .map(|p| home.join(p))
        .filter(|p| p.is_dir())
        .collect()
}

/// Inject compact coding brief so the agent has structural repo context without
/// an extra file_read round-trip.
pub(super) fn wiki_agent_coding_context(workspace_root: &str, chat_mode: &str) -> Option<String> {
    let root = std::path::Path::new(workspace_root.trim());
    if workspace_root.trim().is_empty() {
        return None;
    }

    crate::commands::graph_index::ensure_started(root);

    let max_chars = if chat_mode == "agent" { 4500 } else { 2200 };

    if let Some(excerpt) = crate::commands::graph_agent::coding_brief_excerpt(root, max_chars) {
        let mut block = format!(
            "## Repository coding brief (auto-injected)\n\
             Structural map for navigation — use `graph_explore` + `codebase_search` before editing, \
             and `repo_memory` to query/record source-anchored invariants.\n\
             Mention `@graph <topic>` for inline dependency context.\n\n{excerpt}"
        );
        if let Some(domains) = crate::commands::graph_agent::domain_context_excerpt(root, 6) {
            block.push_str("\n\n");
            block.push_str(&domains);
        }
        if let Some(deep) = crate::commands::graph_agent::deep_wiki_excerpt(root, 1200) {
            block.push_str("\n\n## Deep wiki excerpt\n\n");
            block.push_str(&deep);
        }
        if let Ok(notes) = crate::commands::graph_db::query_repo_notes(root, None, false, 8) {
            if !notes.is_empty() {
                block.push_str("\n\n## Active repository memory notes (`.agentz/REPO_MEMORY.md`)\n\n");
                for n in notes {
                    block.push_str(&format!(
                        "- **[{}]** `{}` — **{}**: {}\n",
                        n.kind, n.scope_target, n.title, n.body
                    ));
                }
            }
        }
        return Some(block);
    }

    let wiki = root.join(".agentz").join("REPO_WIKI.md");
    if wiki.is_file() {
        return Some(
            "## Repository wiki\n\
             Module overview at `.agentz/REPO_WIKI.md`. \
             Run Wiki → Rebuild graph to generate `.agentz/AGENT_CODING_BRIEF.md` for richer agent context."
                .to_string(),
        );
    }
    None
}

/// Load a short excerpt from the session plan file for Agent-mode context.
pub(super) fn active_plan_excerpt(
    workspace_root: &str,
    session_id: &str,
) -> Option<(String, String)> {
    let rel = session_plan_rel_path(session_id);
    let abs = Path::new(workspace_root).join(&rel);
    let content = std::fs::read_to_string(&abs).ok()?;
    if content.trim().is_empty() {
        return None;
    }
    let excerpt: String = content.chars().take(2400).collect();
    let excerpt = if content.chars().count() > 2400 {
        format!("{excerpt}\n…")
    } else {
        excerpt
    };
    Some((rel, excerpt))
}

fn collect_at_refs(raw: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            i += 1;
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if start < i {
                let path = &raw[start..i];
                if path.starts_with("browser-element(") || path.starts_with("terminal-snippet(") {
                    // Handled by dedicated expanders below.
                } else if !refs.iter().any(|p| p == path) {
                    refs.push(path.to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    refs
}

fn read_ref_block(workspace_root: &str, ref_path: &str) -> Option<String> {
    let root = workspace_root.trim_end_matches(['/', '\\']);
    if root.is_empty() {
        return None;
    }
    let rel = ref_path.trim_start_matches(['/', '\\']);
    let full = PathBuf::from(root).join(rel);
    let meta = std::fs::metadata(&full).ok()?;
    if meta.is_dir() {
        return Some(format!(
            "[Directory `@{}` — not inlined. Use file_list / file_read.]",
            ref_path
        ));
    }
    let size = meta.len();
    let raw = std::fs::read(&full).ok()?;
    let is_binary = raw[..raw.len().min(8192)].contains(&0);
    if is_binary {
        return Some(format!(
            "[Binary file `@{}` — {} bytes, not inlined. Use file_read if needed.]",
            ref_path, size
        ));
    }
    let content = String::from_utf8_lossy(&raw).into_owned();
    let truncated = if content.len() > MAX_FILE_REF_CHARS {
        let mut cut = MAX_FILE_REF_CHARS;
        while !content.is_char_boundary(cut) {
            cut -= 1;
        }
        format!(
            "{}\n… [truncated, {} chars omitted]",
            &content[..cut],
            content.len() - cut
        )
    } else {
        content
    };
    Some(format!("```{ref_path}\n{truncated}\n```"))
}

fn codebase_context_block(raw: &str, workspace_root: &str) -> Option<String> {
    let root = workspace_root.trim();
    if root.is_empty() {
        return None;
    }
    let query: String = raw
        .split_whitespace()
        .filter(|w| !w.starts_with('@'))
        .collect::<Vec<_>>()
        .join(" ");
    let hits =
        crate::commands::codebase::search_index(std::path::Path::new(root), &query, 8).ok()?;
    if hits.is_empty() {
        return None;
    }
    let mut block = String::from("Relevant code from @codebase search:\n");
    for h in hits {
        block.push_str(&format!(
            "\n```{}:{}-{}\n{}\n```\n",
            h.path, h.start_line, h.end_line, h.snippet
        ));
    }
    Some(block)
}

fn graph_context_block(raw: &str, workspace_root: &str) -> Option<String> {
    let root = workspace_root.trim();
    if root.is_empty() {
        return None;
    }
    let query: String = raw
        .split_whitespace()
        .filter(|w| !w.starts_with('@'))
        .collect::<Vec<_>>()
        .join(" ");
    let text =
        crate::commands::graph_db::explore_graph(std::path::Path::new(root), &query, 12).ok()?;
    if text.contains("No matches") {
        return None;
    }
    Some(format!("Knowledge graph (@graph):\n\n{text}"))
}

fn collect_browser_element_refs(raw: &str) -> Vec<String> {
    let needle = "@browser-element(";
    let mut refs = Vec::new();
    let mut i = 0usize;
    while let Some(rel) = raw[i..].find(needle) {
        let open = i + rel + needle.len();
        if let Some(close_rel) = raw[open..].find(')') {
            let selector = raw[open..open + close_rel].trim();
            if !selector.is_empty() && !refs.iter().any(|s| s == selector) {
                refs.push(selector.to_string());
            }
            i = open + close_rel + 1;
        } else {
            break;
        }
    }
    refs
}

pub(super) async fn expand_browser_element_refs(raw: &str, browser: &BrowserManager) -> String {
    let refs = collect_browser_element_refs(raw);
    if refs.is_empty() {
        return raw.to_string();
    }

    let browser_open = browser.is_open().await;
    let page_url = if browser_open {
        browser.current_url().await.unwrap_or_default()
    } else {
        String::new()
    };

    let mut blocks = Vec::new();
    for selector in refs {
        if !browser_open {
            blocks.push(format!(
                "[Browser element `@browser-element({selector})` — embedded browser is not open. \
                 Use the `browser` tool (get_text / eval) with selector `{selector}` on the live page.]"
            ));
            continue;
        }
        match browser.query_selector(&selector).await {
            Ok(Some(el)) => {
                let dom_path = if el.dom_path.is_empty() {
                    "(n/a)".to_string()
                } else {
                    el.dom_path.clone()
                };
                let react_component = if el.react_component.is_empty() {
                    "(n/a)".to_string()
                } else {
                    el.react_component.clone()
                };
                blocks.push(format!(
                    "Referenced browser element `@browser-element({selector})` on {page_url}:\n\
                     - Tag: {}\n\
                     - Selector: {}\n\
                     - DOM Path: {dom_path}\n\
                     - React Component: {react_component}\n\
                     - Position: {}×{} at ({}, {})\n\
                     - Text: {}\n\
                     - HTML:\n```html\n{}\n```\n\
                     (Re-query with the `browser` tool and selector `{selector}` for live updates.)",
                    el.tag,
                    el.selector,
                    el.rect_width,
                    el.rect_height,
                    el.rect_x,
                    el.rect_y,
                    el.text,
                    el.html,
                ));
            }
            Ok(None) => blocks.push(format!(
                "[Browser element `{selector}` not found on the current page ({page_url}).]"
            )),
            Err(e) => blocks.push(format!(
                "[Failed to query browser element `{selector}`: {e}]"
            )),
        }
    }

    format!(
        "Context from referenced browser elements:\n\n{}\n\n---\n\n{}",
        blocks.join("\n\n"),
        raw
    )
}

fn collect_terminal_snippet_refs(raw: &str) -> Vec<String> {
    let needle = "@terminal-snippet(";
    let mut refs = Vec::new();
    let mut i = 0usize;
    while let Some(rel) = raw[i..].find(needle) {
        let open = i + rel + needle.len();
        if let Some(close_rel) = raw[open..].find(')') {
            let id = raw[open..open + close_rel].trim();
            if !id.is_empty() && !refs.iter().any(|s| s == id) {
                refs.push(id.to_string());
            }
            i = open + close_rel + 1;
        } else {
            break;
        }
    }
    refs
}

pub(super) fn expand_terminal_snippets(
    raw: &str,
    snippets: &std::collections::HashMap<String, String>,
) -> String {
    let refs = collect_terminal_snippet_refs(raw);
    if refs.is_empty() {
        return raw.to_string();
    }
    let mut blocks = Vec::new();
    for id in refs {
        if let Some(text) = snippets.get(&id) {
            blocks.push(format!(
                "Terminal selection `@terminal-snippet({id})`:\n```\n{text}\n```"
            ));
        } else {
            blocks.push(format!(
                "[Terminal snippet `{id}` not found — it may have expired.]"
            ));
        }
    }
    format!(
        "Context from terminal selections:\n\n{}\n\n---\n\n{}",
        blocks.join("\n\n"),
        raw
    )
}

async fn remote_read_text(path: &str) -> Option<String> {
    use base64::Engine as _;
    let v = crate::remote::call("fs.readFile", serde_json::json!({ "path": path }))
        .await
        .ok()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(v.get("base64")?.as_str()?)
        .ok()?;
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

pub(super) async fn expand_file_refs_remote(raw: &str, workspace_root: &str) -> String {
    let refs = collect_at_refs(raw);
    if refs.is_empty() {
        return raw.to_string();
    }
    let Ok(session) = crate::remote::bind(workspace_root).await else {
        return raw.to_string();
    };
    crate::remote::with_session(
        session,
        expand_file_refs_remote_bound(raw, workspace_root, refs),
    )
    .await
}

pub(super) async fn expand_file_refs_remote_bound(
    raw: &str,
    workspace_root: &str,
    refs: Vec<String>,
) -> String {
    let Ok(Some(root)) = crate::remote::resolve(workspace_root).await else {
        return raw.to_string();
    };
    let mut blocks = Vec::new();
    let mut total = 0usize;
    for ref_path in refs.iter().filter(|r| *r != "codebase" && *r != "graph") {
        if total >= MAX_TOTAL_REF_CHARS {
            blocks.push(format!(
                "[Skipped remaining @file references — total inline limit ({MAX_TOTAL_REF_CHARS} chars) reached.]"
            ));
            break;
        }
        let full = format!(
            "{}/{}",
            root.trim_end_matches('/'),
            ref_path.trim_start_matches(['/', '\\'])
        );
        let Some(content) = remote_read_text(&full).await else {
            continue;
        };
        let body = if content.len() > MAX_FILE_REF_CHARS {
            let mut cut = MAX_FILE_REF_CHARS;
            while !content.is_char_boundary(cut) {
                cut -= 1;
            }
            format!(
                "{}\n… [truncated, {} chars omitted]",
                &content[..cut],
                content.len() - cut
            )
        } else {
            content
        };
        let block = format!("```{ref_path}\n{body}\n```");
        total += block.len();
        blocks.push(block);
    }
    let wants_codebase = refs.iter().any(|r| r == "codebase");
    let wants_graph = refs.iter().any(|r| r == "graph");
    if wants_codebase || wants_graph {
        if let Ok(mirror) = crate::remote::mirror::sync(workspace_root, false).await {
            let mirror = mirror.to_string_lossy().to_string();
            let raw_owned = raw.to_string();
            let extra = tokio::task::spawn_blocking(move || {
                let mut v = Vec::new();
                if wants_codebase {
                    v.extend(codebase_context_block(&raw_owned, &mirror));
                }
                if wants_graph {
                    v.extend(graph_context_block(&raw_owned, &mirror));
                }
                v
            })
            .await
            .unwrap_or_default();
            blocks.extend(extra);
        }
    }
    if blocks.is_empty() {
        return raw.to_string();
    }
    format!(
        "Context from referenced files:\n\n{}\n\n---\n\n{}",
        blocks.join("\n\n"),
        raw
    )
}

pub(super) async fn project_rules_context_remote(workspace_root: &str) -> Option<String> {
    let session = crate::remote::bind(workspace_root).await.ok()?;
    crate::remote::with_session(session, project_rules_context_remote_bound(workspace_root)).await
}

pub(super) async fn project_rules_context_remote_bound(workspace_root: &str) -> Option<String> {
    let root = crate::remote::resolve(workspace_root).await.ok()??;
    for dir in [".agentz/rules", ".cursor/rules"] {
        let dir_path = format!("{}/{dir}", root.trim_end_matches('/'));
        let Ok(entries) =
            crate::remote::call("fs.readDir", serde_json::json!({ "path": dir_path })).await
        else {
            continue;
        };
        let mut names: Vec<String> = entries
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| e.get("name")?.as_str().map(str::to_string))
            .filter(|n| n.ends_with(".md") || n.ends_with(".mdc"))
            .collect();
        names.sort();
        let mut blocks = Vec::new();
        for n in names {
            if let Some(c) = remote_read_text(&format!("{dir_path}/{n}")).await {
                if !c.trim().is_empty() {
                    blocks.push(c.trim().to_string());
                }
            }
        }
        if !blocks.is_empty() {
            return Some(format!("## Project rules\n{}", blocks.join("\n\n")));
        }
    }
    None
}

pub(super) fn expand_file_refs(raw: &str, workspace_root: &str) -> String {
    let refs = collect_at_refs(raw);
    if refs.is_empty() {
        return raw.to_string();
    }
    tracing::info!(
        "expand_file_refs: {} @ref(s) in prompt (raw_len={}): {:?}",
        refs.len(),
        raw.len(),
        refs
    );
    let wants_codebase = refs.iter().any(|r| r == "codebase");
    let wants_graph = refs.iter().any(|r| r == "graph");
    let mut blocks = Vec::new();
    let mut total = 0usize;
    for ref_path in &refs {
        if ref_path == "codebase" || ref_path == "graph" {
            continue;
        }
        if total >= MAX_TOTAL_REF_CHARS {
            blocks.push(format!(
                "[Skipped remaining @file references — total inline limit ({MAX_TOTAL_REF_CHARS} chars) reached.]"
            ));
            break;
        }
        if let Some(block) = read_ref_block(workspace_root, ref_path) {
            tracing::info!(
                "expand_file_refs: inlined @{} → {} chars (total_so_far={})",
                ref_path,
                block.len(),
                total + block.len()
            );
            total += block.len();
            blocks.push(block);
        } else {
            tracing::info!(
                "expand_file_refs: @{} → skipped (not found or unreadable)",
                ref_path
            );
        }
    }
    if wants_codebase {
        if let Some(block) = codebase_context_block(raw, workspace_root) {
            tracing::info!("expand_file_refs: @codebase block → {} chars", block.len());
            blocks.push(block);
        }
    }
    if wants_graph {
        if let Some(block) = graph_context_block(raw, workspace_root) {
            tracing::info!("expand_file_refs: @graph block → {} chars", block.len());
            blocks.push(block);
        }
    }
    if blocks.is_empty() {
        tracing::info!("expand_file_refs: no blocks produced, passing prompt through");
        return raw.to_string();
    }
    let out = format!(
        "Context from referenced files:\n\n{}\n\n---\n\n{}",
        blocks.join("\n\n"),
        raw
    );
    tracing::info!(
        "expand_file_refs: wrapped prompt → {} chars (blocks={})",
        out.len(),
        blocks.len()
    );
    out
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn build_turn_system_prompt(
    app: &tauri::AppHandle,
    config_dir: &Path,
    workspace_root: &str,
    session_id: &str,
    chat_mode: &str,
    prefer_zh: bool,
    allow_outside_workspace: bool,
    extra_system_context: Option<&str>,
    agent: Option<&AgentManifest>,
    enabled_skills: &[String],
    enabled_connectors: Option<&Vec<String>>,
    durable_plan_enabled: bool,
    plan_store: &PlanStore,
    workz_team_id: Option<&str>,
    workz_pool_id: Option<&str>,
) -> String {
    let mut extra_sections: Vec<String> = Vec::new();
    if let Some(existing) = extra_system_context {
        if !existing.trim().is_empty() {
            extra_sections.push(existing.to_string());
        }
    }
    if let Some(agent) = agent {
        let prompt_body = if prefer_zh && !agent.system_prompt_zh.trim().is_empty() {
            agent.system_prompt_zh.trim()
        } else {
            agent.system_prompt.trim()
        };
        if !prompt_body.is_empty() {
            extra_sections.push(format!(
                "## Active agent: {}\nYou are acting as this agent. Follow its role and \
                 instructions:\n{}",
                agent.name, prompt_body
            ));
        }
    }
    if let Some(skills) = skills_context(config_dir, enabled_skills) {
        extra_sections.push(skills);
    }
    if chat_mode != "plan" {
        if let Some(delegable) = crate::commands::delegation::delegable_agents_context(config_dir) {
            extra_sections.push(delegable);
        }
    }
    if let Some(ids) = enabled_connectors {
        if let Some(connectors) =
            crate::commands::connectors::connectors_prompt_context(config_dir, ids)
        {
            extra_sections.push(connectors);
        }
    }
    if let Some(parsed) = crate::remote::vfs::parse(workspace_root) {
        extra_sections.push(format!(
            "## Remote workspace\n\nThe workspace lives on `{}` at `{}` (Linux/POSIX). \
             file_* and shell tools execute on that machine: use POSIX shell syntax, \
             forward-slash paths, and paths relative to the workspace root. \
             Index tools (codebase_search, graph_*, symbol_search, impact) search a \
             local mirror synced from the remote; their paths are workspace-relative.",
            parsed.authority, parsed.path
        ));
    }
    let rules = if crate::remote::is_remote(workspace_root) {
        project_rules_context_remote(workspace_root).await
    } else {
        project_rules_context(workspace_root)
    };
    if let Some(rules) = rules {
        extra_sections.push(rules);
    }
    if let Some(wiki) = wiki_agent_coding_context(workspace_root, chat_mode) {
        extra_sections.push(wiki);
    }
    if chat_mode == "plan" {
        let plan_path = session_plan_rel_path(session_id);
        extra_sections.push(plan_mode_context(&plan_path));
    } else if let Some((plan_path, excerpt)) = active_plan_excerpt(workspace_root, session_id) {
        extra_sections.push(agent_active_plan_context(&plan_path, Some(&excerpt)));
    }
    if chat_mode == "agent" && !durable_plan_enabled {
        // The static Agent prompt tells the model to maintain `plan_todo`. When the
        // durable plan tool was not registered for this turn (non-interactive run,
        // remote workspace, restricted slash-command tool list) that instruction
        // would send the model calling a tool that does not exist.
        extra_sections.push(
            "## Task tracking in this turn\n\
             The `plan_todo` tool is NOT available in this turn. Ignore every instruction about \
             creating or maintaining `plan_todo` / Todos, including the rule that a text-only reply is \
             final only when all Todos are completed. Do not call `plan_todo`. Track progress in the \
             plan file instead (update **状态** and **执行记录** with `plan_write` or `file_edit`) and \
             keep working until the task is done."
                .to_string(),
        );
    }
    if chat_mode == "agent" {
        if let Some(hint) =
            crate::commands::devenv::context_hint_for_turn(std::path::Path::new(workspace_root))
                .await
        {
            extra_sections.push(hint);
        }
        let retained_todos = {
            let plans = plan_store.lock().await;
            plans.get(session_id).cloned().unwrap_or_default()
        };
        if durable_plan_enabled
            && retained_todos
                .iter()
                .any(|item| item.status == "pending" || item.status == "in_progress")
        {
            extra_sections.push(active_todo_context(&retained_todos));
        }
    }
    if let (Some(team_id), Some(_pool_id)) = (
        workz_team_id.filter(|s| !s.is_empty()),
        workz_pool_id.filter(|s| !s.is_empty()),
    ) {
        if let Ok(team) = TeamManifest::load_by_id(app, team_id) {
            let excerpt: String = team.org_spec.chars().take(2000).collect();
            let excerpt = if team.org_spec.chars().count() > 2000 {
                format!("{excerpt}…")
            } else {
                excerpt
            };
            extra_sections.push(swarm_coordinator_append(
                Some(excerpt.as_str()).filter(|s| !s.trim().is_empty()),
                &team.workflow_hint,
            ));
        }
    }
    if let Some(hook_ctx) =
        crate::commands::workbench::run_event_hooks(workspace_root, "beforeAgentTurn").await
    {
        extra_sections.push(hook_ctx);
    }
    let combined_extra = if extra_sections.is_empty() {
        None
    } else {
        Some(extra_sections.join("\n\n"))
    };
    agent_system_prompt(
        workspace_root,
        allow_outside_workspace,
        combined_extra.as_deref(),
    )
}

#[cfg(test)]
mod remote_context_binding_tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn prompt_without_file_refs_needs_no_remote_connection() {
        let prompt = "Explain the architecture";
        assert_eq!(
            expand_file_refs_remote(prompt, "agentz-remote://box/work").await,
            prompt
        );
    }

    #[tokio::test]
    async fn context_rejects_a_workspace_outside_its_bound_authority() {
        let session = crate::remote::RemoteSession {
            target: crate::remote::RemoteTarget::Ssh { host: "a".into() },
            authority: "ssh-remote+a".into(),
            path: "/work".into(),
            generation: 0,
            manager: Arc::new(crate::commands::ext_host::ExtHostManager::new()),
        };
        crate::remote::with_session(session, async {
            let workspace = "agentz-remote://ssh-remote+b/work";
            let prompt = "Explain @file.rs";
            assert_eq!(
                expand_file_refs_remote_bound(prompt, workspace, vec!["file.rs".into()]).await,
                prompt
            );
            assert!(project_rules_context_remote_bound(workspace)
                .await
                .is_none());
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_multibyte_file_ref_truncates_without_panicking() {
        let dir = std::env::temp_dir().join(format!("agentz-ref-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // "a" shifts every 3-byte char off the 12000-byte boundary.
        std::fs::write(dir.join("doc.md"), format!("a{}", "\u{8bbe}\u{8ba1}".repeat(4000))).unwrap();
        let block = read_ref_block(&dir.to_string_lossy(), "doc.md").expect("file inlined");
        assert!(block.contains("[truncated,"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
