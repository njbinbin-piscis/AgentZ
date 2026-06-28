//! Agent-facing graph utilities (P2).
//!
//! Turns the structural graph into actionable coding context: compact briefs,
//! subgraph search, validation, and domain hints — not just visualization data.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::commands::graph::{load_graph, load_tour, GraphDoc, GraphEdge, GraphNode};

const MAX_BRIEF_CHARS: usize = 8_000;
const CODING_BRIEF_PATH: &str = ".agentz/AGENT_CODING_BRIEF.md";
const DOMAIN_PATH: &str = ".agentz/domain.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEntry {
    pub name: String,
    pub modules: Vec<String>,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDoc {
    pub version: String,
    pub generated_at: String,
    pub domains: Vec<DomainEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphValidation {
    pub ok: bool,
    pub warnings: Vec<String>,
}


fn module_node_id(name: &str) -> String {
    format!("module:{name}")
}

fn top_module(path: &str) -> String {
    match path.split_once('/') {
        Some((head, _)) => head.to_string(),
        None => "(root)".to_string(),
    }
}

/// Validate graph completeness for agent reliance.
pub fn validate_graph(doc: &GraphDoc) -> GraphValidation {
    let mut warnings = Vec::new();
    let node_ids: BTreeSet<String> = doc.nodes.iter().map(|n| n.id.clone()).collect();
    for e in &doc.edges {
        if !node_ids.contains(&e.from) {
            warnings.push(format!("edge from missing node: {}", e.from));
        }
        if !node_ids.contains(&e.to) {
            warnings.push(format!("edge to missing node: {}", e.to));
        }
    }
    if doc.modules.is_empty() {
        warnings.push("no modules detected".into());
    }
    let orphan_modules: Vec<_> = doc
        .modules
        .iter()
        .filter(|m| {
            m.in_degree == 0
                && !doc.edges.iter().any(|e| {
                    e.kind == "depends" && e.from == module_node_id(&m.name)
                })
        })
        .map(|m| m.name.clone())
        .take(5)
        .collect();
    if orphan_modules.len() > 3 {
        warnings.push(format!(
            "many isolated modules (no cross-deps): {}",
            orphan_modules.join(", ")
        ));
    }
    GraphValidation {
        ok: warnings.is_empty(),
        warnings,
    }
}

fn hub_files(doc: &GraphDoc, limit: usize) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in &doc.edges {
        if e.kind != "imports" {
            continue;
        }
        let Some(to) = e.to.strip_prefix("file:") else {
            continue;
        };
        *counts.entry(to.to_string()).or_default() += 1;
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by_key(|x| std::cmp::Reverse(x.1));
    sorted.truncate(limit);
    sorted
}

fn module_dependencies(doc: &GraphDoc) -> BTreeMap<String, Vec<String>> {
    let mut deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for e in &doc.edges {
        if e.kind != "depends" {
            continue;
        }
        let Some(from) = e.from.strip_prefix("module:") else {
            continue;
        };
        let Some(to) = e.to.strip_prefix("module:") else {
            continue;
        };
        if from != to {
            deps.entry(from.to_string()).or_default().insert(to.to_string());
        }
    }
    deps.into_iter()
        .map(|(k, v)| {
            let mut list: Vec<String> = v.into_iter().collect();
            list.sort();
            (k, list)
        })
        .collect()
}

fn infer_domain(module: &str) -> &'static str {
    let lower = module.to_lowercase();
    if lower.contains("test") || lower.contains("spec") {
        "testing"
    } else if lower.contains("ui") || lower.contains("frontend") || lower == "src" {
        "frontend"
    } else if lower.contains("tauri") || lower.contains("backend") || lower.contains("server") {
        "backend"
    } else if lower.contains("doc") {
        "documentation"
    } else if lower.contains("cmd") || lower.contains("cli") {
        "cli"
    } else {
        "core"
    }
}

pub fn build_domain_doc(doc: &GraphDoc) -> DomainDoc {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in &doc.modules {
        let domain = infer_domain(&m.name);
        grouped.entry(domain.to_string()).or_default().push(m.name.clone());
    }
    let domains: Vec<DomainEntry> = grouped
        .into_iter()
        .map(|(name, modules)| {
            let hint = match name.as_str() {
                "frontend" => "UI components, workspaces, React/Monaco integration",
                "backend" => "Rust/Tauri commands, kernel, persistence",
                "testing" => "tests and fixtures",
                "documentation" => "docs and specs",
                "cli" => "command-line tools",
                _ => "shared/core modules",
            };
            DomainEntry {
                name,
                modules,
                hint: hint.into(),
            }
        })
        .collect();
    DomainDoc {
        version: "1.0".into(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        domains,
    }
}

/// Compact markdown brief optimized for agent coding turns.
pub fn build_coding_brief(root: &Path, doc: &GraphDoc) -> String {
    let project = doc.project.clone();
    let deps = module_dependencies(doc);
    let hubs = hub_files(doc, 12);
    let tour = load_tour(root);
    let validation = validate_graph(doc);

    let mut md = String::new();
    md.push_str("# Agent Coding Brief\n\n");
    md.push_str(&format!(
        "_Auto-generated from `.agentz/graph.json` on {}._\n\n",
        doc.generated_at
    ));
    md.push_str("Use **`graph_explore`** first for structure (source + blast radius), then **`codebase_search`** for snippets. \
Legacy **`graph_search`** remains for module ids. Mention **`@graph <topic>`** in chat for inline subgraph context.\n\n");

    md.push_str("## Repository snapshot\n\n");
    md.push_str(&format!(
        "- **Project:** {project}\n- **Files:** {} · **Modules:** {} · **Edges:** {}\n\n",
        doc.stats.files,
        doc.modules.len(),
        doc.stats.edges
    ));

    if !validation.warnings.is_empty() {
        md.push_str("### Graph warnings\n\n");
        for w in validation.warnings.iter().take(5) {
            md.push_str(&format!("- {w}\n"));
        }
        md.push('\n');
    }

    let module_summaries: Vec<_> = doc
        .nodes
        .iter()
        .filter(|n| n.kind == "module" && !n.summary.is_empty())
        .take(8)
        .collect();
    if !module_summaries.is_empty() {
        md.push_str("## Module summaries (from WIKI_DEEP)\n\n");
        for n in module_summaries {
            md.push_str(&format!("- **`{}/`**: {}\n", n.name, n.summary));
        }
        md.push('\n');
    }

    md.push_str("## Module map (dependencies)\n\n");
    for m in doc.modules.iter().take(25) {
        let dep_list = deps.get(&m.name).cloned().unwrap_or_default();
        let dep_str = if dep_list.is_empty() {
            "—".into()
        } else {
            dep_list.iter().take(6).cloned().collect::<Vec<_>>().join(", ")
        };
        md.push_str(&format!(
            "- **`{}/`** — {} files, in-degree {} → depends on: {dep_str}\n",
            m.name, m.file_count, m.in_degree
        ));
    }
    md.push('\n');

    if let Some(tour) = tour {
        if !tour.stops.is_empty() {
            md.push_str("## Suggested exploration order\n\n");
            for stop in tour.stops.iter().take(12) {
                md.push_str(&format!(
                    "{}. **`{}/`** — {} files\n",
                    stop.order, stop.module, stop.file_count
                ));
            }
            md.push('\n');
        }
    }

    let domain = build_domain_doc(doc);
    if !domain.domains.is_empty() {
        md.push_str("## Inferred business domains\n\n");
        for d in domain.domains.iter().take(8) {
            let mods: Vec<String> = d.modules.iter().take(5).cloned().collect();
            md.push_str(&format!(
                "- **{}** ({}) — {}\n",
                d.name,
                mods.join(", "),
                d.hint
            ));
        }
        md.push('\n');
    }

    md.push_str("## Hub files (high fan-in — change with care)\n\n");
    for (path, deg) in &hubs {
        md.push_str(&format!("- `{path}` — imported by {deg} file(s)\n"));
    }
    md.push('\n');

    md.push_str("## Layer hints (by path)\n\n");
    let mut layers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in &doc.nodes {
        if n.kind != "file" {
            continue;
        }
        let Some(path) = &n.path else { continue };
        layers
            .entry(n.layer.clone())
            .or_default()
            .push(top_module(path));
    }
    for (layer, mods) in layers {
        let uniq: BTreeSet<String> = mods.into_iter().collect();
        let sample: Vec<String> = uniq.iter().take(4).cloned().collect();
        md.push_str(&format!("- **{layer}**: {}\n", sample.join(", ")));
    }
    md.push('\n');

    md.push_str("## Coding playbook\n\n");
    md.push_str("| Goal | First steps |\n|------|-------------|\n");
    md.push_str("| Find where X is implemented | `codebase_search(\"X\")` then `graph_search(\"X\")` |\n");
    md.push_str("| Understand module boundaries | Read this brief → `file_read` hub files |\n");
    md.push_str("| Assess change impact | `graph_search` importers of target file/module |\n");
    md.push_str("| Add a new feature | Follow tour order; match layer of similar code |\n");
    md.push('\n');

    if md.len() > MAX_BRIEF_CHARS {
        md.truncate(MAX_BRIEF_CHARS);
        md.push_str("\n\n… [truncated — use `file_read .agentz/AGENT_CODING_BRIEF.md` for full brief]\n");
    }
    md
}

pub fn write_agent_artifacts(root: &Path, doc: &GraphDoc) -> Result<(), String> {
    let dir = root.join(".agentz");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create .agentz: {e}"))?;
    let brief = build_coding_brief(root, doc);
    std::fs::write(root.join(CODING_BRIEF_PATH), &brief)
        .map_err(|e| format!("write coding brief: {e}"))?;
    let domain = build_domain_doc(doc);
    let domain_json = serde_json::to_string_pretty(&domain).map_err(|e| format!("serialize domain: {e}"))?;
    std::fs::write(root.join(DOMAIN_PATH), domain_json).map_err(|e| format!("write domain: {e}"))?;
    Ok(())
}

pub fn coding_brief_path() -> &'static str {
    CODING_BRIEF_PATH
}

/// Read inline excerpt for agent system prompt (bounded).
pub fn coding_brief_excerpt(root: &Path, max_chars: usize) -> Option<String> {
    let path = root.join(CODING_BRIEF_PATH);
    let content = std::fs::read_to_string(path).ok()?;
    if content.trim().is_empty() {
        return None;
    }
    if content.len() <= max_chars {
        Some(content)
    } else {
        Some(format!(
            "{}\n… [truncated — full brief at `{CODING_BRIEF_PATH}`]",
            &content[..max_chars]
        ))
    }
}

fn node_matches_query(n: &GraphNode, q: &str) -> bool {
    let q = q.to_lowercase();
    if n.name.to_lowercase().contains(&q) {
        return true;
    }
    if let Some(p) = &n.path {
        if p.to_lowercase().contains(&q) {
            return true;
        }
    }
    n.layer.to_lowercase().contains(&q) || n.summary.to_lowercase().contains(&q)
}

fn collect_related_edges(doc: &GraphDoc, seed_ids: &BTreeSet<String>) -> Vec<GraphEdge> {
    doc.edges
        .iter()
        .filter(|e| {
            (e.kind == "imports" || e.kind == "depends")
                && (seed_ids.contains(&e.from) || seed_ids.contains(&e.to))
        })
        .take(40)
        .cloned()
        .collect()
}

/// Search graph and format as agent-readable text (tool + @graph mention).
pub fn search_graph(root: &Path, query: &str, limit: usize) -> Result<String, String> {
    let doc = load_graph(root).ok_or(
        "graph.json not found — run Wiki → Rebuild graph first".to_string(),
    )?;
    let q = query.trim();
    let limit = limit.clamp(1, 30);

    if q.is_empty() {
        return Ok(format!(
            "graph_search overview: {} modules, {} files\n\nTop modules:\n{}",
            doc.modules.len(),
            doc.stats.files,
            doc.modules
                .iter()
                .take(limit)
                .map(|m| format!("- {}/ ({} files, in-degree {})", m.name, m.file_count, m.in_degree))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    let mut matched: Vec<GraphNode> = doc
        .nodes
        .iter()
        .filter(|n| node_matches_query(n, q))
        .take(limit)
        .cloned()
        .collect();

    if matched.is_empty() {
        for m in doc
            .modules
            .iter()
            .filter(|m| m.name.to_lowercase().contains(&q.to_lowercase()))
            .take(limit)
        {
            if let Some(n) = doc
                .nodes
                .iter()
                .find(|n| n.id == module_node_id(&m.name))
            {
                matched.push(n.clone());
            } else {
                matched.push(GraphNode {
                    id: module_node_id(&m.name),
                    kind: "module".into(),
                    path: None,
                    name: m.name.clone(),
                    layer: "unknown".into(),
                    summary: String::new(),
                });
            }
        }
    }

    if matched.is_empty() {
        return Ok(format!("graph_search: no nodes matching \"{q}\""));
    }

    let seed_ids: BTreeSet<String> = matched.iter().map(|n| n.id.clone()).collect();
    let edges = collect_related_edges(&doc, &seed_ids);

    let mut out = format!("graph_search: {} match(es) for \"{q}\"\n", matched.len());
    for n in &matched {
        out.push_str(&format!(
            "\n── {} ({}) layer={}{}\n",
            n.id,
            n.kind,
            n.layer,
            n.path
                .as_ref()
                .map(|p| format!(" path={p}"))
                .unwrap_or_default()
        ));
    }
    if !edges.is_empty() {
        out.push_str("\nRelated import/dependency edges:\n");
        for e in edges {
            out.push_str(&format!("  {} --[{}]--> {}\n", e.from, e.kind, e.to));
        }
    }

    // Actionable: list importers for file nodes
    for n in matched.iter().filter(|n| n.kind == "file") {
        let importers: Vec<String> = doc
            .edges
            .iter()
            .filter(|e| e.kind == "imports" && e.to == n.id)
            .map(|e| e.from.clone())
            .take(8)
            .collect();
        if !importers.is_empty() {
            out.push_str(&format!("\nImporters of {}:\n", n.id));
            for imp in importers {
                out.push_str(&format!("  - {imp}\n"));
            }
        }
    }

    Ok(out)
}

pub fn load_domain(root: &Path) -> Option<DomainDoc> {
    let path = root.join(DOMAIN_PATH);
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[tauri::command]
pub fn graph_domain_read(project_dir: String) -> Result<Option<DomainDoc>, String> {
    let root = Path::new(project_dir.trim());
    if project_dir.trim().is_empty() {
        return Err("project_dir is empty".into());
    }
    Ok(load_domain(root))
}

const DEEP_WIKI_PATH: &str = ".agentz/WIKI_DEEP.md";

/// Parse `###`-level module sections from WIKI_DEEP (ralph-loop output).
pub fn parse_deep_wiki_module_summaries(content: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut current_title: Option<String> = None;
    let mut paragraph = String::new();

    let flush = |title: &mut Option<String>, para: &mut String, map: &mut BTreeMap<String, String>| {
        let Some(t) = title.take() else {
            para.clear();
            return;
        };
        let summary: String = para
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("```"))
            .take(4)
            .collect::<Vec<_>>()
            .join(" ");
        if summary.len() >= 20 {
            map.insert(normalize_module_title(&t), summary);
        }
        para.clear();
    };

    for line in content.lines() {
        if line.starts_with("### ") {
            flush(&mut current_title, &mut paragraph, &mut out);
            current_title = Some(line.trim_start_matches("### ").trim().to_string());
        } else if line.starts_with("## ") {
            flush(&mut current_title, &mut paragraph, &mut out);
        } else if current_title.is_some() {
            paragraph.push_str(line);
            paragraph.push('\n');
        }
    }
    flush(&mut current_title, &mut paragraph, &mut out);
    out
}

fn normalize_module_title(raw: &str) -> String {
    let t = raw.trim();
    // Strip leading "2.1 " numbering from wiki headings.
    let t = t
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ')
        .trim();
    t.to_string()
}

fn match_module_summary<'a>(
    summaries: &'a BTreeMap<String, String>,
    module_name: &str,
) -> Option<&'a String> {
    let name = module_name.to_lowercase();
    summaries
        .iter()
        .find(|(k, _)| {
            let k = k.to_lowercase();
            k == name || k.contains(&name) || name.contains(&k)
        })
        .map(|(_, v)| v)
}

/// Apply WIKI_DEEP module summaries onto graph module nodes (in-place).
pub fn merge_deep_wiki_summaries(doc: &mut GraphDoc, root: &Path) -> usize {
    let path = root.join(DEEP_WIKI_PATH);
    let Ok(content) = std::fs::read_to_string(path) else {
        return 0;
    };
    let summaries = parse_deep_wiki_module_summaries(&content);
    if summaries.is_empty() {
        return 0;
    }
    let mut applied = 0usize;
    for node in &mut doc.nodes {
        if node.kind != "module" {
            continue;
        }
        if let Some(summary) = match_module_summary(&summaries, &node.name) {
            node.summary = summary.clone();
            applied += 1;
        }
    }
    applied
}

/// Short excerpt from WIKI_DEEP for agent system context.
pub fn deep_wiki_excerpt(root: &Path, max_chars: usize) -> Option<String> {
    let path = root.join(DEEP_WIKI_PATH);
    let content = std::fs::read_to_string(path).ok()?;
    if content.trim().is_empty() {
        return None;
    }
    let excerpt: String = content.chars().take(max_chars).collect();
    let suffix = if content.chars().count() > max_chars {
        format!("\n… [full doc at `{DEEP_WIKI_PATH}`]")
    } else {
        String::new()
    };
    Some(format!("{excerpt}{suffix}"))
}

/// Compact domain list for agent injection.
pub fn domain_context_excerpt(root: &Path, max_domains: usize) -> Option<String> {
    let doc = load_domain(root)?;
    if doc.domains.is_empty() {
        return None;
    }
    let mut lines = vec!["Inferred business domains:".to_string()];
    for d in doc.domains.iter().take(max_domains) {
        lines.push(format!(
            "- **{}** ({}): {} — modules: {}",
            d.name,
            d.modules.len(),
            d.hint,
            d.modules.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    Some(lines.join("\n"))
}

#[tauri::command]
pub fn graph_validate(project_dir: String) -> Result<GraphValidation, String> {
    let root = Path::new(project_dir.trim());
    if project_dir.trim().is_empty() {
        return Err("project_dir is empty".into());
    }
    let doc = load_graph(root).ok_or(
        "graph.json not found — run Wiki → Rebuild graph first".to_string(),
    )?;
    Ok(validate_graph(&doc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::graph::{GraphStats, ModuleStat};

    fn sample_doc() -> GraphDoc {
        GraphDoc {
            version: "1.0".into(),
            generated_at: "2026-01-01T00:00:00Z".into(),
            project: "test".into(),
            nodes: vec![
                GraphNode {
                    id: "file:src/auth.rs".into(),
                    kind: "file".into(),
                    path: Some("src/auth.rs".into()),
                    name: "auth.rs".into(),
                    layer: "api".into(),
                    summary: String::new(),
                },
                GraphNode {
                    id: "module:src".into(),
                    kind: "module".into(),
                    path: None,
                    name: "src".into(),
                    layer: "unknown".into(),
                    summary: String::new(),
                },
            ],
            edges: vec![],
            modules: vec![ModuleStat {
                name: "src".into(),
                file_count: 1,
                in_degree: 0,
            }],
            stats: GraphStats {
                files: 1,
                nodes: 2,
                edges: 0,
            },
        }
    }

    #[test]
    fn coding_brief_contains_modules() {
        let doc = sample_doc();
        let brief = build_coding_brief(std::path::Path::new("."), &doc);
        assert!(brief.contains("Agent Coding Brief"));
        assert!(brief.contains("src/"));
    }

    #[test]
    fn parse_deep_wiki_extracts_module_summary() {
        let md = r#"
## 2. 核心模块详解

### 2.1 src-tauri

**功能概述**：
Rust 后端，提供 Tauri 命令与 Agent 运行时。

### 2.2 src

前端 React 应用。
"#;
        let map = parse_deep_wiki_module_summaries(md);
        assert!(map.values().any(|s| s.contains("Tauri")));
    }

    #[test]
    fn merge_deep_wiki_applies_to_module_nodes() {
        let dir = std::env::temp_dir().join(format!(
            "agentz-graph-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".agentz")).unwrap();
        std::fs::write(
            dir.join(".agentz/WIKI_DEEP.md"),
            "### src\n\nCore source tree for application logic.\n",
        )
        .unwrap();
        let mut doc = sample_doc();
        let n = merge_deep_wiki_summaries(&mut doc, &dir);
        assert_eq!(n, 1);
        assert!(doc.nodes.iter().any(|node| node.name == "src" && !node.summary.is_empty()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_graph_finds_module() {
        let dir = std::env::temp_dir().join(format!(
            "agentz-graph-search-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".agentz")).unwrap();
        let doc = sample_doc();
        let json = serde_json::to_string_pretty(&doc).unwrap();
        std::fs::write(dir.join(".agentz/graph.json"), json).unwrap();
        let out = search_graph(&dir, "auth", 5).unwrap();
        assert!(out.contains("auth"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
