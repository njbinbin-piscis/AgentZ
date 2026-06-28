//! Repo Wiki generation (M8).
//!
//! Produces a module / architecture overview of the workspace by reusing the
//! M5 codebase index (`{project}/.agentz/index.db`). The generation is
//! deterministic and dependency-light — it aggregates the indexed chunks by
//! top-level module and language and highlights the largest files — so it works
//! offline without an LLM. The result is written to `{project}/.agentz/REPO_WIKI.md`
//! and returned to the caller for preview.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::commands::codebase::{build_index, search_index, CodeSearchHit};
use crate::commands::data_scope::require_project_dir;
use crate::commands::graph::{self, GraphDoc};

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
    match path.split_once('/') {
        Some((head, _)) => head.to_string(),
        None => "(root)".to_string(),
    }
}

fn mermaid_module_deps(doc: &GraphDoc) -> Option<String> {
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
        edges.insert((from_id, to_id));
    }
    if edges.is_empty() {
        return None;
    }
    let mut md = String::from("```mermaid\nflowchart LR\n");
    for (from, to) in edges.iter().take(40) {
        md.push_str(&format!("  {from} --> {to}\n"));
    }
    md.push_str("```\n");
    Some(md)
}

fn mermaid_id(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn file_in_degrees(doc: &GraphDoc) -> Vec<(String, usize)> {
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
    sorted
}

fn ensure_graph(root: &Path) -> Option<GraphDoc> {
    if let Some(doc) = graph::load_graph(root) {
        return Some(doc);
    }
    let _ = crate::commands::graph_index::request_rebuild(root.to_path_buf());
    graph::load_graph(root)
}

fn ext_of(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .filter(|e| e.len() <= 8)
        .unwrap_or_else(|| "—".to_string())
}

/// Build the wiki markdown for `root`, (re)building the index if it is empty.
pub fn generate(root: &Path) -> Result<String, String> {
    // Ensure the index exists / is populated.
    let conn = Connection::open(index_db_path(root)).map_err(|e| format!("open index db: {e}"))?;
    let has_chunks: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
        .unwrap_or(0);
    drop(conn);
    if has_chunks == 0 {
        build_index(root)?;
    }

    let conn = Connection::open(index_db_path(root)).map_err(|e| format!("open index db: {e}"))?;

    // Per-file chunk counts (proxy for size / importance).
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

    // Overall language breakdown.
    let mut lang_totals: BTreeMap<String, usize> = BTreeMap::new();
    for m in modules.values() {
        for (lang, c) in &m.langs {
            *lang_totals.entry(lang.clone()).or_default() += c;
        }
    }
    let mut lang_sorted: Vec<(String, usize)> = lang_totals.into_iter().collect();
    lang_sorted.sort_by_key(|x| std::cmp::Reverse(x.1));

    // Largest files overall.
    file_chunks.sort_by_key(|x| std::cmp::Reverse(x.1));

    // Modules sorted by chunk weight.
    let mut module_sorted: Vec<(&String, &ModuleStat)> = modules.iter().collect();
    module_sorted.sort_by_key(|x| std::cmp::Reverse(x.1.chunks));

    let project_name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "workspace".to_string());
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();

    let mut md = String::new();
    md.push_str(&format!("# {project_name} — Repo Wiki\n\n"));
    md.push_str(&format!(
        "_Auto-generated by AgentZ from the codebase index on {now}._\n\n"
    ));
    md.push_str(&format!(
        "**{total_files}** indexed files · **{total_chunks}** code chunks · \
         **{}** top-level modules.\n\n",
        module_sorted.len()
    ));

    md.push_str("## Languages\n\n");
    for (lang, c) in lang_sorted.iter().take(12) {
        let pct = (*c as f64 / total_chunks.max(1) as f64) * 100.0;
        md.push_str(&format!("- `.{lang}` — {c} chunks ({pct:.0}%)\n"));
    }
    md.push('\n');

    md.push_str("## Modules\n\n");
    md.push_str("Top-level directories, ordered by code weight:\n\n");
    for (name, stat) in module_sorted.iter().take(40) {
        let mut langs: Vec<(&String, &usize)> = stat.langs.iter().collect();
        langs.sort_by(|a, b| b.1.cmp(a.1));
        let lang_str = langs
            .iter()
            .take(3)
            .map(|(l, _)| format!(".{l}"))
            .collect::<Vec<_>>()
            .join(", ");
        md.push_str(&format!(
            "### `{name}/`\n- {} files · {} chunks · {}\n\n",
            stat.files.len(),
            stat.chunks,
            if lang_str.is_empty() {
                "—".into()
            } else {
                lang_str
            }
        ));
    }

    md.push('\n');

    if let Some(doc) = ensure_graph(root) {
        md.push_str("## Module dependencies\n\n");
        md.push_str("Cross-module import relationships (from `.agentz/graph.json`):\n\n");
        if let Some(mermaid) = mermaid_module_deps(&doc) {
            md.push_str(&mermaid);
            md.push('\n');
        } else {
            md.push_str("_No cross-module dependencies detected._\n\n");
        }

        md.push_str("## Hub files\n\n");
        md.push_str("Most imported files (likely shared utilities or core types):\n\n");
        for (path, deg) in file_in_degrees(&doc).iter().take(15) {
            md.push_str(&format!("- `{path}` — imported by {deg} file(s)\n"));
        }
        md.push('\n');
    }

    md.push_str("## Representative snippets\n\n");
    for (name, stat) in module_sorted.iter().take(5) {
        let hits = search_index(root, name, 1).unwrap_or_default();
        if let Some(hit) = hits.first() {
            md.push_str(&format!("### `{name}/`\n\n"));
            md.push_str(&format!(
                "`{}` L{}–{}\n\n```\n{}\n```\n\n",
                hit.path,
                hit.start_line,
                hit.end_line,
                hit.snippet.lines().take(8).collect::<Vec<_>>().join("\n")
            ));
        } else if !stat.files.is_empty() {
            let sample = stat.files.iter().next().unwrap();
            md.push_str(&format!("- `{name}/` — see `{sample}`\n"));
        }
    }
    md.push('\n');

    md.push_str("## Largest files\n\n");
    md.push_str("Likely entry points / hotspots (by indexed size):\n\n");
    for (path, chunks) in file_chunks.iter().take(25) {
        md.push_str(&format!("- `{path}` — {chunks} chunks\n"));
    }
    md.push('\n');

    md.push_str("---\n\n");
    md.push_str(
        "> **Agent coding brief:** `.agentz/AGENT_CODING_BRIEF.md` (auto-generated with graph) \
         is injected into agent turns. Use **`graph_search`** + **`@graph`** for structure, \
         **`codebase_search`** + **`@codebase`** for code snippets.\n>\n\
         > Tip: ask the Agent to expand any module here, or use `@codebase` in chat \
         to pull relevant code into a conversation.\n",
    );

    Ok(md)
}

/// Surface representative search hits for richer wiki sections.
pub fn sample_hits(root: &Path, query: &str) -> Vec<CodeSearchHit> {
    search_index(root, query, 5).unwrap_or_default()
}

// ─── Tauri command ────────────────────────────────────────────────────────

/// Generate the Repo Wiki, write it to `.agentz/REPO_WIKI.md`, and return the
/// markdown plus the relative path it was written to.
#[tauri::command]
pub async fn repo_wiki_generate(project_dir: Option<String>) -> Result<RepoWikiResult, String> {
    let project = require_project_dir(project_dir.as_deref())?;
    let root = PathBuf::from(project);
    tokio::task::spawn_blocking(move || {
        let md = generate(&root)?;
        let out_dir = root.join(".agentz");
        std::fs::create_dir_all(&out_dir).map_err(|e| format!("create .agentz: {e}"))?;
        let out = out_dir.join("REPO_WIKI.md");
        std::fs::write(&out, &md).map_err(|e| format!("write wiki: {e}"))?;
        Ok(RepoWikiResult {
            path: ".agentz/REPO_WIKI.md".to_string(),
            markdown: md,
        })
    })
    .await
    .map_err(|e| format!("wiki task failed: {e}"))?
}

#[derive(Debug, serde::Serialize)]
pub struct RepoWikiResult {
    pub path: String,
    pub markdown: String,
}
