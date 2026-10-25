//! Dual-Track Repo Wiki & DeepWiki Engine (M8 + DeepWiki Upgrade).
//!
//! - **Track 1 (`generate` / `repo_wiki_generate` -> `.agentz/REPO_WIKI.md`)**:
//!   Deterministic, zero-token architecture wiki built from `index.db`, `graph.json`,
//!   Tree-sitter AST skeletons, PageRank hub scores, Tauri IPC cross-boundary edges,
//!   and source-anchored `repo_notes`.
//! - **Track 2 (`generate_deep_wiki` / `repo_wiki_generate_deep` -> `.agentz/WIKI_DEEP.md`)**:
//!   Incremental LLM-synthesized DeepWiki with per-module content-hash caching in
//!   `graph.db` (`module_wiki_cache`) and automatic summary writeback into `graph.json`,
//!   `graph.db`, and `.agentz/AGENT_CODING_BRIEF.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use piscis_kernel::llm::{self, LlmMessage, LlmRequest, MessageContent};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::commands::codebase::{build_index, search_index, CodeSearchHit};
use crate::commands::data_scope::{
    open_global_kernel_state, open_project_kernel_state, require_indexable_project_dir,
};
use crate::commands::graph::{self, GraphDoc};
use crate::commands::graph_db::{
    export_repo_memory_markdown, open_graph_db, query_repo_notes, sync_from_graph_doc, RepoNote,
};
use crate::commands::symbols::{extract_file_skeleton, SymbolSkeleton};

fn index_db_path(root: &Path) -> PathBuf {
    root.join(".agentz").join("index.db")
}

/// Per-module aggregate stats accumulated from the index.
#[derive(Default)]
struct ModuleStat {
    files: std::collections::HashSet<String>,
    chunks: usize,
    /// language (extension) -> chunk count
    langs: BTreeMap<String, usize>,
}

fn top_module(path: &str) -> String {
    graph::logical_module(path)
}

fn mermaid_id(name: &str) -> String {
    let mut id: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if id.is_empty() || id.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        id.insert_str(0, "m_");
    }
    // Avoid Mermaid reserved keywords
    if matches!(
        id.as_str(),
        "end" | "graph" | "subgraph" | "flowchart" | "style" | "class" | "click" | "default"
    ) {
        id.push_str("_mod");
    }
    id
}

fn mermaid_safe_label(label: &str) -> String {
    label
        .replace('"', "'")
        .replace('[', "(")
        .replace(']', ")")
        .replace('{', "(")
        .replace('}', ")")
}

fn mermaid_module_deps(doc: &GraphDoc) -> Option<String> {
    let mut nodes: BTreeMap<String, String> = BTreeMap::new();
    let mut edges: BTreeSet<(String, String)> = BTreeSet::new();
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
        if from == to {
            continue;
        }
        let from_id = mermaid_id(from);
        let to_id = mermaid_id(to);
        nodes.insert(from_id.clone(), mermaid_safe_label(from));
        nodes.insert(to_id.clone(), mermaid_safe_label(to));
        edges.insert((from_id, to_id));
    }
    if edges.is_empty() {
        return None;
    }
    let mut md = String::from("```mermaid\nflowchart LR\n");
    for (id, label) in &nodes {
        md.push_str(&format!("  {id}[\"{label}\"]\n"));
    }
    for (from, to) in edges.iter().take(40) {
        md.push_str(&format!("  {from} --> {to}\n"));
    }
    md.push_str("```\n");
    Some(md)
}

fn mermaid_ipc_bridges(doc: &GraphDoc) -> Option<(String, Vec<(String, String)>)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for e in &doc.edges {
        if e.kind != "ipc_calls" {
            continue;
        }
        let Some(fe) = e.from.strip_prefix("file:") else {
            continue;
        };
        let Some(be) = e.to.strip_prefix("file:") else {
            continue;
        };
        pairs.push((fe.to_string(), be.to_string()));
    }
    pairs.sort();
    pairs.dedup();
    if pairs.is_empty() {
        return None;
    }

    let mut md = String::from("```mermaid\nflowchart LR\n");
    let mut declared = BTreeSet::new();
    for (fe, be) in pairs.iter().take(24) {
        let fe_id = mermaid_id(&format!("fe_{fe}"));
        let be_id = mermaid_id(&format!("be_{be}"));
        let fe_short = fe.rsplit('/').next().unwrap_or(fe);
        let be_short = be.rsplit('/').next().unwrap_or(be);
        if declared.insert(fe_id.clone()) {
            md.push_str(&format!(
                "  {fe_id}[\"FE: {}\"]\n",
                mermaid_safe_label(fe_short)
            ));
        }
        if declared.insert(be_id.clone()) {
            md.push_str(&format!(
                "  {be_id}[\"Rust: {}\"]\n",
                mermaid_safe_label(be_short)
            ));
        }
        md.push_str(&format!("  {fe_id} -.->|invoke| {be_id}\n"));
    }
    md.push_str("```\n");
    Some((md, pairs))
}

fn file_hub_ranking(doc: &GraphDoc) -> Vec<(String, usize, f64)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in &doc.edges {
        if e.kind != "imports" && e.kind != "ipc_calls" {
            continue;
        }
        let Some(to) = e.to.strip_prefix("file:") else {
            continue;
        };
        *counts.entry(to.to_string()).or_default() += 1;
    }
    let pr = graph::compute_pagerank(doc);
    let mut ranked: Vec<(String, usize, f64)> = counts
        .into_iter()
        .map(|(path, deg)| {
            let score = pr.get(&format!("file:{path}")).copied().unwrap_or(1.0);
            (path, deg, score)
        })
        .collect();
    ranked.sort_by(|a, b| {
        let wa = a.2 * (a.1 as f64 + 1.0);
        let wb = b.2 * (b.1 as f64 + 1.0);
        wb.partial_cmp(&wa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.cmp(&a.1))
    });
    ranked
}

fn ensure_graph(root: &Path) -> Option<GraphDoc> {
    if let Some(doc) = graph::load_graph(root) {
        return Some(doc);
    }
    graph::generate(root).ok()
}

fn ext_of(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .filter(|e| e.len() <= 8)
        .unwrap_or_else(|| "—".to_string())
}

/// Extract up to `max_symbols` representative AST symbols across the top files of a module.
fn collect_module_skeleton(
    root: &Path,
    files: &[String],
    max_files: usize,
    max_symbols: usize,
) -> (Vec<(String, String)>, Vec<SymbolSkeleton>) {
    let mut file_docs = Vec::new();
    let mut symbols = Vec::new();
    for rel in files.iter().take(max_files) {
        let abs = root.join(rel);
        let Ok(raw) = crate::bounded_read::read(&abs, 400_000) else {
            continue;
        };
        if raw[..raw.len().min(2048)].contains(&0) {
            continue;
        }
        let source = String::from_utf8_lossy(&raw);
        let Some(skel) = extract_file_skeleton(&abs, rel, &source) else {
            continue;
        };
        if !skel.file_doc.is_empty() {
            file_docs.push((rel.clone(), skel.file_doc));
        }
        for sym in skel.symbols {
            if symbols.len() < max_symbols {
                symbols.push(sym);
            }
        }
    }
    (file_docs, symbols)
}

/// Build the Track 1 deterministic architecture wiki markdown for `root`.
pub fn generate(root: &Path, project_name: &str) -> Result<String, String> {
    let conn = Connection::open(index_db_path(root)).map_err(|e| format!("open index db: {e}"))?;
    let has_chunks: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
        .unwrap_or(0);
    drop(conn);
    if has_chunks == 0 {
        build_index(root)?;
    }

    let conn = Connection::open(index_db_path(root)).map_err(|e| format!("open index db: {e}"))?;
    let mut stmt = conn
        .prepare("SELECT path, COUNT(*) FROM chunks GROUP BY path")
        .map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| format!("query: {e}"))?;

    let mut modules: BTreeMap<String, ModuleStat> = BTreeMap::new();
    let mut file_chunks: Vec<(String, i64)> = Vec::new();
    let mut total_files = 0usize;
    let mut total_chunks = 0usize;
    for row in rows.flatten() {
        let (path, chunks) = row;
        total_files += 1;
        total_chunks += chunks as usize;
        file_chunks.push((path.clone(), chunks));
        let m = modules.entry(top_module(&path)).or_default();
        m.files.insert(path.clone());
        m.chunks += chunks as usize;
        *m.langs.entry(ext_of(&path)).or_default() += chunks as usize;
    }

    if total_files == 0 {
        return Err("index is empty — nothing to document".to_string());
    }

    let mut lang_totals: BTreeMap<String, usize> = BTreeMap::new();
    for m in modules.values() {
        for (lang, c) in &m.langs {
            *lang_totals.entry(lang.clone()).or_default() += c;
        }
    }
    let mut lang_sorted: Vec<(String, usize)> = lang_totals.into_iter().collect();
    lang_sorted.sort_by_key(|x| std::cmp::Reverse(x.1));

    file_chunks.sort_by_key(|x| std::cmp::Reverse(x.1));

    let mut module_sorted: Vec<(&String, &ModuleStat)> = modules.iter().collect();
    module_sorted.sort_by_key(|x| std::cmp::Reverse(x.1.chunks));

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    let doc_opt = ensure_graph(root);
    let repo_notes = query_repo_notes(root, None, true, 60).unwrap_or_default();

    let mut md = String::new();
    md.push_str(&format!("# {project_name} — Repo Wiki\n\n"));
    md.push_str(&format!(
        "_Auto-generated by AgentZ (AST Graph + PageRank + IPC + Anchored Memory) on {now}._\n\n"
    ));
    md.push_str(&format!(
        "**{total_files}** indexed files · **{total_chunks}** code chunks · \
         **{}** logical subsystems · **{}** anchored memory notes.\n\n",
        module_sorted.len(),
        repo_notes.len()
    ));

    // 1. Start Here: PageRank Guided Tour & Core Hubs
    if let Some(ref doc) = doc_opt {
        let hubs = file_hub_ranking(doc);
        if !hubs.is_empty() {
            md.push_str("## Start Here — Core Architecture Hubs (PageRank)\n\n");
            md.push_str("Highest-centrality files ranked by `PageRank × Blast Radius`:\n\n");
            for (idx, (path, deg, pr)) in hubs.iter().take(12).enumerate() {
                let mod_name = top_module(path);
                md.push_str(&format!(
                    "{}. `{path}:1` (`{mod_name}`) — imported/called by **{deg}** file(s) · PageRank `{pr:.2}`\n",
                    idx + 1
                ));
            }
            md.push('\n');
        }

        if let Some(tour) = graph::load_tour(root) {
            if !tour.stops.is_empty() {
                md.push_str("### Recommended Reading Order\n\n");
                for stop in tour.stops.iter().take(10) {
                    md.push_str(&format!(
                        "- **#{} `{}`**: {}\n",
                        stop.order, stop.module, stop.summary
                    ));
                }
                md.push('\n');
            }
        }
    }

    // 2. C4 Subsystem Dependency & Tauri IPC Diagrams
    if let Some(ref doc) = doc_opt {
        md.push_str("## Subsystem Dependency Graph (C4 Container View)\n\n");
        if let Some(mermaid) = mermaid_module_deps(doc) {
            md.push_str(&mermaid);
            md.push('\n');
        } else {
            md.push_str("_No cross-module dependencies detected._\n\n");
        }

        if let Some((ipc_mermaid, ipc_pairs)) = mermaid_ipc_bridges(doc) {
            md.push_str("## Cross-Boundary Tauri IPC Graph (Frontend TS ↔ Rust Commands)\n\n");
            md.push_str(&ipc_mermaid);
            md.push('\n');
            for (fe, be) in ipc_pairs.iter().take(15) {
                md.push_str(&format!("- `{fe}:1` → invokes Rust handler in `{be}:1`\n"));
            }
            md.push('\n');
        }
    }

    // 3. Languages Breakdown
    md.push_str("## Languages\n\n");
    for (lang, c) in lang_sorted.iter().take(12) {
        let pct = (*c as f64 / total_chunks.max(1) as f64) * 100.0;
        md.push_str(&format!("- `.{lang}` — {c} chunks ({pct:.0}%)\n"));
    }
    md.push('\n');

    // 4. Subsystem Knowledge Cards (AST Symbol Skeletons + Anchored Notes)
    md.push_str("## Subsystem Knowledge Cards (AST Skeletons)\n\n");
    for (name, stat) in module_sorted.iter().take(28) {
        let mut langs: Vec<(&String, &usize)> = stat.langs.iter().collect();
        langs.sort_by(|a, b| b.1.cmp(a.1));
        let lang_str = langs
            .iter()
            .take(3)
            .map(|(l, _)| format!(".{l}"))
            .collect::<Vec<_>>()
            .join(", ");
        md.push_str(&format!(
            "### `{name}`\n- **Scale:** {} files · {} chunks · {}\n",
            stat.files.len(),
            stat.chunks,
            if lang_str.is_empty() {
                "—".into()
            } else {
                lang_str
            }
        ));

        let mut mod_files: Vec<String> = stat.files.iter().cloned().collect();
        mod_files.sort();
        let (file_docs, syms) = collect_module_skeleton(root, &mod_files, 5, 8);
        for (rel, fdoc) in file_docs.iter().take(3) {
            md.push_str(&format!("- **`{rel}:1`**: {fdoc}\n"));
        }
        if !syms.is_empty() {
            md.push_str("- **Key AST Symbols:**\n");
            for s in syms.iter().take(6) {
                let sig = if s.signature.is_empty() {
                    &s.name
                } else {
                    &s.signature
                };
                if s.doc.is_empty() {
                    md.push_str(&format!(
                        "  - `{}:{}-{}` — `{}` ({})\n",
                        s.file, s.start_line, s.end_line, sig, s.kind
                    ));
                } else {
                    md.push_str(&format!(
                        "  - `{}:{}-{}` — `{}`: {}\n",
                        s.file, s.start_line, s.end_line, sig, s.doc
                    ));
                }
            }
        }

        let mod_notes: Vec<&RepoNote> = repo_notes
            .iter()
            .filter(|n| &n.scope_target == *name || n.scope_target.starts_with(&format!("{name}/")))
            .collect();
        if !mod_notes.is_empty() {
            md.push_str("- **Anchored Memory Notes:**\n");
            for n in mod_notes.iter().take(4) {
                let flag = if n.stale { "⚠️ STALE" } else { "✓ ACTIVE" };
                md.push_str(&format!(
                    "  - [{flag} | {}] **{}**: {}\n",
                    n.kind, n.title, n.body
                ));
            }
        }
        md.push('\n');
    }

    // 5. Largest Hotspot Files
    md.push_str("## Largest Hotspot Files\n\n");
    for (path, chunks) in file_chunks.iter().take(20) {
        md.push_str(&format!("- `{path}:1` — {chunks} chunks\n"));
    }
    md.push('\n');

    md.push_str("---\n\n");
    md.push_str(
        "> **Agent coding brief & memory:** `.agentz/AGENT_CODING_BRIEF.md` and `.agentz/REPO_MEMORY.md` \
         are automatically maintained. Use **`graph_explore`** for AST call chains + blast radius, \
         and **`repo_memory`** to record or verify source-anchored module invariants.\n",
    );

    Ok(md)
}

/// Surface representative search hits for richer wiki sections.
pub fn sample_hits(root: &Path, query: &str) -> Vec<CodeSearchHit> {
    search_index(root, query, 5).unwrap_or_default()
}

// ─── Track 2: Incremental LLM DeepWiki with Module Hash Caching ───────────

fn compute_module_content_hash(root: &Path, files: &[String]) -> String {
    let mut hasher = Sha256::new();
    let mut sorted = files.to_vec();
    sorted.sort();
    for rel in sorted {
        hasher.update(rel.as_bytes());
        if let Ok(bytes) = crate::bounded_read::read(&root.join(&rel), 400_000) {
            hasher.update(&bytes);
        }
    }
    let hex = format!("{:x}", hasher.finalize());
    hex.chars().take(20).collect()
}

fn build_module_ast_prompt_context(root: &Path, module_name: &str, files: &[String]) -> String {
    let mut ctx = format!("Module: `{module_name}` ({} files)\n\n", files.len());
    let mut sorted = files.to_vec();
    sorted.sort();
    for rel in sorted.iter().take(12) {
        let abs = root.join(rel);
        let Ok(raw) = crate::bounded_read::read(&abs, 400_000) else {
            continue;
        };
        if raw[..raw.len().min(2048)].contains(&0) {
            continue;
        };
        let source = String::from_utf8_lossy(&raw);
        let line_count = source.lines().count();
        ctx.push_str(&format!("### File `{rel}` ({line_count} lines)\n"));
        if let Some(skel) = extract_file_skeleton(&abs, rel, &source) {
            if !skel.file_doc.is_empty() {
                ctx.push_str(&format!("Header doc: {}\n", skel.file_doc));
            }
            for sym in skel.symbols.iter().take(18) {
                ctx.push_str(&format!(
                    "- L{}-{}: [{}] `{}`",
                    sym.start_line,
                    sym.end_line,
                    sym.kind,
                    if sym.signature.is_empty() {
                        &sym.name
                    } else {
                        &sym.signature
                    }
                ));
                if !sym.doc.is_empty() {
                    ctx.push_str(&format!(" // {}", sym.doc));
                }
                ctx.push('\n');
            }
        } else {
            for (i, line) in source.lines().take(20).enumerate() {
                ctx.push_str(&format!("{}: {}\n", i + 1, line));
            }
        }
        ctx.push('\n');
    }
    ctx
}

fn deterministic_module_deep_section(root: &Path, module_name: &str, files: &[String]) -> (String, String) {
    let (file_docs, syms) = collect_module_skeleton(root, files, 8, 12);
    let summary = if let Some((_, d)) = file_docs.first() {
        d.clone()
    } else if !syms.is_empty() {
        format!(
            "Subsystem `{module_name}` ({} files) defining {}.",
            files.len(),
            syms.iter()
                .take(4)
                .map(|s| format!("`{}`", s.name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        format!("Subsystem `{module_name}` containing {} files.", files.len())
    };

    let mut section = format!("### `{module_name}`\n\n{summary}\n\n");
    if !file_docs.is_empty() {
        section.push_str("**Core Files & Responsibilities:**\n");
        for (rel, doc) in file_docs.iter().take(5) {
            section.push_str(&format!("- `{rel}:1` — {doc}\n"));
        }
        section.push('\n');
    }
    if !syms.is_empty() {
        section.push_str("**Key Symbols & Entry Points:**\n");
        for s in syms.iter().take(8) {
            section.push_str(&format!(
                "- `{}:{}-{}` — `{}` ({})\n",
                s.file,
                s.start_line,
                s.end_line,
                if s.signature.is_empty() {
                    &s.name
                } else {
                    &s.signature
                },
                s.kind
            ));
        }
        section.push('\n');
    }
    (summary, section)
}

fn read_cached_module_wiki(
    root: &Path,
    module_name: &str,
    expected_hash: &str,
) -> Option<(String, String)> {
    let conn = open_graph_db(root).ok()?;
    let (hash, markdown, summary): (String, String, String) = conn
        .query_row(
            "SELECT content_hash, markdown, summary FROM module_wiki_cache WHERE module_name = ?1",
            params![module_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok()?;
    if hash == expected_hash && !markdown.trim().is_empty() {
        Some((summary, markdown))
    } else {
        None
    }
}

fn write_cached_module_wiki(
    root: &Path,
    module_name: &str,
    content_hash: &str,
    summary: &str,
    markdown: &str,
) {
    if let Ok(conn) = open_graph_db(root) {
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT INTO module_wiki_cache (module_name, content_hash, markdown, summary, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(module_name) DO UPDATE SET
               content_hash = excluded.content_hash,
               markdown = excluded.markdown,
               summary = excluded.summary,
               updated_at = excluded.updated_at",
            params![module_name, content_hash, markdown, summary, now],
        );
    }
}

// ─── Tauri commands ───────────────────────────────────────────────────────

/// Generate the fast deterministic Repo Wiki, write it to `.agentz/REPO_WIKI.md`,
/// and return the markdown plus path.
#[tauri::command]
pub async fn repo_wiki_generate(project_dir: Option<String>) -> Result<RepoWikiResult, String> {
    let (project, root) = wiki_root(project_dir.as_deref()).await?;
    let name = crate::remote::mirror::workspace_name(&project, &root);
    let build_root = root.clone();
    let result = tokio::task::spawn_blocking(move || {
        let md = generate(&build_root, &name)?;
        let out_dir = build_root.join(".agentz");
        std::fs::create_dir_all(&out_dir).map_err(|e| format!("create .agentz: {e}"))?;
        let out = out_dir.join("REPO_WIKI.md");
        std::fs::write(&out, &md).map_err(|e| format!("write wiki: {e}"))?;
        Ok::<_, String>(RepoWikiResult {
            path: ".agentz/REPO_WIKI.md".to_string(),
            markdown: md,
        })
    })
    .await
    .map_err(|e| format!("wiki task failed: {e}"))??;
    publish_outputs(&project, &root, &[".agentz/REPO_WIKI.md"]).await?;
    Ok(result)
}

/// Project id plus the local root the index code reads: the project itself,
/// or the synced source mirror for `agentz-remote://` workspaces.
async fn wiki_root(project_dir: Option<&str>) -> Result<(String, PathBuf), String> {
    let project = require_indexable_project_dir(project_dir)?;
    let root = crate::remote::mirror::index_root(&project, true).await?;
    Ok((project, root))
}

/// Remote projects: copy generated documents to the remote `.agentz/`.
async fn publish_outputs(project: &str, root: &Path, rels: &[&str]) -> Result<(), String> {
    if crate::remote::is_remote(project) {
        crate::remote::mirror::publish(project, root, rels).await
    } else {
        Ok(())
    }
}

/// Generate the Track 2 Incremental DeepWiki (`.agentz/WIKI_DEEP.md`) with per-module
/// SHA-256 caching and write module summaries back into `graph.json`, `graph.db`, and
/// `.agentz/AGENT_CODING_BRIEF.md`.
#[tauri::command]
pub async fn repo_wiki_generate_deep(
    app: AppHandle,
    project_dir: Option<String>,
) -> Result<RepoWikiResult, String> {
    let (project, root) = wiki_root(project_dir.as_deref()).await?;
    let project_name = crate::remote::mirror::workspace_name(&project, &root);

    // LLM settings are global; remote projects have no local session store.
    let kernel_state = if crate::remote::is_remote(&project) {
        open_global_kernel_state(&app)
    } else {
        open_project_kernel_state(&app, &project)
    };
    let llm_cfg = if let Ok((_, settings_arc)) = kernel_state {
        let s = settings_arc.lock().await;
        s.llm_providers
            .iter()
            .find(|cfg| !cfg.effective_api_key().trim().is_empty())
            .cloned()
    } else {
        None
    };

    // Group project files by logical module
    let root_for_scan = root.clone();
    let (mut doc, module_files) = tokio::task::spawn_blocking(move || -> Result<(GraphDoc, BTreeMap<String, Vec<String>>), String> {
        let doc = ensure_graph(&root_for_scan)
            .ok_or_else(|| "Failed to load or build repository graph".to_string())?;
        let mut by_mod: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for n in &doc.nodes {
            if n.kind == "file" {
                if let Some(ref p) = n.path {
                    by_mod.entry(top_module(p)).or_default().push(p.clone());
                }
            }
        }
        Ok((doc, by_mod))
    })
    .await
    .map_err(|e| format!("deep wiki scan failed: {e}"))??;

    let mut ordered_mods: Vec<(String, Vec<String>)> = module_files.into_iter().collect();
    ordered_mods.sort_by_key(|b| std::cmp::Reverse(b.1.len()));

    let mut summaries: Vec<(String, String)> = Vec::new();
    let mut sections: Vec<String> = Vec::new();
    let mut cache_hits = 0usize;
    let mut synthesized = 0usize;

    for (mod_name, files) in ordered_mods.into_iter().take(10) {
        let mod_hash = compute_module_content_hash(&root, &files);
        if let Some((cached_sum, cached_md)) = read_cached_module_wiki(&root, &mod_name, &mod_hash) {
            cache_hits += 1;
            summaries.push((mod_name, cached_sum));
            sections.push(cached_md);
            continue;
        }

        let mut built: Option<(String, String)> = None;
        if let Some(ref cfg) = llm_cfg {
            let ast_ctx = build_module_ast_prompt_context(&root, &mod_name, &files);
            let prompt = format!(
                "You are generating a concise DeepWiki architecture section for subsystem `{mod_name}`.\n\
                 Output format:\n\
                 Line 1: SUMMARY: <one sentence summary under 160 chars>\n\
                 Line 2+: Markdown under `### \\`{mod_name}\\`` covering:\n\
                 - Core responsibility & architectural role\n\
                 - Key control/data flow across functions (cite exact files and line ranges as `path/to/file.ext:start-end`)\n\
                 - Important invariants or edge cases\n\n\
                 AST Skeleton Input:\n{ast_ctx}"
            );
            let base_url = if cfg.base_url.trim().is_empty() {
                None
            } else {
                Some(cfg.base_url.as_str())
            };
            let client = llm::build_client_with_timeout(
                &cfg.provider,
                cfg.effective_api_key(),
                base_url,
                60,
            );
            let req = LlmRequest {
                messages: vec![LlmMessage {
                    role: "user".into(),
                    content: MessageContent::text(&prompt),
                }],
                system: Some(
                    "You are a principal software architect writing source-anchored DeepWiki documentation."
                        .into(),
                ),
                tools: vec![],
                model: cfg.model.clone(),
                max_tokens: 900,
                stream: false,
                vision_override: None,
            };
            if let Ok(resp) = client.complete(req).await {
                let text = resp.content.trim();
                if !text.is_empty() {
                    let mut lines = text.lines();
                    let first = lines.next().unwrap_or("").trim();
                    let summary = first
                        .strip_prefix("SUMMARY:")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("Subsystem `{mod_name}` ({} files).", files.len()));
                    let rest: String = lines.collect::<Vec<_>>().join("\n").trim().to_string();
                    let md_sec = if rest.starts_with("###") {
                        format!("{rest}\n\n")
                    } else {
                        format!("### `{mod_name}`\n\n{rest}\n\n")
                    };
                    built = Some((summary, md_sec));
                    synthesized += 1;
                }
            }
        }

        let (summary, md_sec) =
            built.unwrap_or_else(|| deterministic_module_deep_section(&root, &mod_name, &files));
        write_cached_module_wiki(&root, &mod_name, &mod_hash, &summary, &md_sec);
        summaries.push((mod_name, summary));
        sections.push(md_sec);
    }

    let root_finalize = root.clone();
    let result = tokio::task::spawn_blocking(move || {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();

        let mut deep_md = String::new();
        deep_md.push_str(&format!("# {project_name} — DeepWiki (Incremental AST + AI)\n\n"));
        deep_md.push_str(&format!(
            "_Generated on {now} · Module cache hits: **{cache_hits}** · Newly synthesized: **{synthesized}**._\n\n"
        ));

        if let Some(mermaid) = mermaid_module_deps(&doc) {
            deep_md.push_str("## Architecture & Subsystem Topology\n\n");
            deep_md.push_str(&mermaid);
            deep_md.push('\n');
        }

        // `## Module Summaries` is parsed by `graph_agent::merge_deep_wiki_summaries`
        deep_md.push_str("## Module Summaries\n\n");
        for (m, sum) in &summaries {
            deep_md.push_str(&format!("- `{m}`: {sum}\n"));
        }
        deep_md.push('\n');

        deep_md.push_str("## Deep Subsystem Guides\n\n");
        for sec in &sections {
            deep_md.push_str(sec);
        }

        let out_dir = root_finalize.join(".agentz");
        std::fs::create_dir_all(&out_dir).map_err(|e| format!("create .agentz: {e}"))?;
        let out_path = out_dir.join("WIKI_DEEP.md");
        std::fs::write(&out_path, &deep_md).map_err(|e| format!("write WIKI_DEEP.md: {e}"))?;

        // Backfill summaries into graph.json, graph.db, and AGENT_CODING_BRIEF.md
        if crate::commands::graph_agent::merge_deep_wiki_summaries(&mut doc, &root_finalize) > 0 {
            if let Ok(json) = serde_json::to_string_pretty(&doc) {
                let _ = std::fs::write(out_dir.join("graph.json"), json);
            }
            let _ = sync_from_graph_doc(&root_finalize, &doc);
            let _ = crate::commands::graph_agent::write_agent_artifacts(&root_finalize, &doc);
        }

        Ok::<_, String>(RepoWikiResult {
            path: ".agentz/WIKI_DEEP.md".to_string(),
            markdown: deep_md,
        })
    })
    .await
    .map_err(|e| format!("deep wiki finalize failed: {e}"))??;
    publish_outputs(
        &project,
        &root,
        &[
            ".agentz/WIKI_DEEP.md",
            crate::commands::graph_agent::coding_brief_path(),
        ],
    )
    .await?;
    Ok(result)
}

/// Export and return `.agentz/REPO_MEMORY.md` (source-anchored repository memory notes).
#[tauri::command]
pub async fn repo_memory_read(project_dir: Option<String>) -> Result<RepoWikiResult, String> {
    let project = require_indexable_project_dir(project_dir.as_deref())?;
    let root = crate::remote::mirror::index_root(&project, false).await?;
    let export_root = root.clone();
    let result = tokio::task::spawn_blocking(move || {
        let out = export_repo_memory_markdown(&export_root)?;
        let md = std::fs::read_to_string(&out).map_err(|e| format!("read REPO_MEMORY.md: {e}"))?;
        Ok::<_, String>(RepoWikiResult {
            path: ".agentz/REPO_MEMORY.md".to_string(),
            markdown: md,
        })
    })
    .await
    .map_err(|e| format!("repo memory task failed: {e}"))??;
    publish_outputs(&project, &root, &[".agentz/REPO_MEMORY.md"]).await?;
    Ok(result)
}

#[derive(Debug, serde::Serialize)]
pub struct RepoWikiResult {
    pub path: String,
    pub markdown: String,
}
