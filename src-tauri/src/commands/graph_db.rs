//! SQLite knowledge graph (`graph.db`) for `graph_explore`, AST symbol call graph,
//! Tauri cross-boundary IPC edges, and source-anchored repository memory (`repo_notes`).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::commands::graph::GraphDoc;

const GRAPH_DB_VERSION: &str = "2.0";
const MAX_SNIPPET_LINES: usize = 120;
const MAX_BLAST_ITEMS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoteCitation {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default)]
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoNote {
    pub id: String,
    /// 'module' | 'file' | 'symbol' | 'architecture'
    pub scope_kind: String,
    /// e.g. 'src-tauri/commands' or 'src-tauri/src/commands/session.rs'
    pub scope_target: String,
    /// 'invariant' | 'pitfall' | 'dataflow' | 'decision'
    pub kind: String,
    pub title: String,
    pub body: String,
    pub citations: Vec<NoteCitation>,
    /// 'agent_auto' | 'agent_tool' | 'deep_wiki' | 'user'
    pub source: String,
    pub stale: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoNoteInput {
    pub scope_kind: String,
    pub scope_target: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub citations: Vec<NoteCitation>,
    #[serde(default = "default_note_source")]
    pub source: String,
}

fn default_note_source() -> String {
    "agent_tool".to_string()
}

fn graph_db_path(root: &Path) -> PathBuf {
    root.join(".agentz").join("graph.db")
}

fn graph_db_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn open_graph_db(root: &Path) -> Result<Connection, String> {
    open_graph_db_inner(root)
}

fn open_graph_db_inner(root: &Path) -> Result<Connection, String> {
    let dir = root.join(".agentz");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create .agentz: {e}"))?;
    let conn = Connection::open(graph_db_path(root)).map_err(|e| format!("open graph.db: {e}"))?;
    conn.busy_timeout(Duration::from_secs(10))
        .map_err(|e| format!("graph.db busy_timeout: {e}"))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("wal: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            path TEXT,
            name TEXT NOT NULL,
            layer TEXT,
            summary TEXT NOT NULL DEFAULT '',
            start_line INTEGER,
            end_line INTEGER,
            signature TEXT NOT NULL DEFAULT '',
            content_hash TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS edges (
            from_id TEXT NOT NULL,
            to_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            weight INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (from_id, to_id, kind)
        );
        CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_id);
        CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_id);
        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            name, path, summary, kind,
            content='nodes', content_rowid='rowid'
        );
        CREATE TABLE IF NOT EXISTS pending_sync (
            path TEXT PRIMARY KEY,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS repo_notes (
            id TEXT PRIMARY KEY,
            scope_kind TEXT NOT NULL,
            scope_target TEXT NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            citations TEXT NOT NULL DEFAULT '[]',
            source TEXT NOT NULL DEFAULT 'agent_tool',
            stale INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_repo_notes_target ON repo_notes(scope_target);
        CREATE TABLE IF NOT EXISTS module_wiki_cache (
            module_name TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            markdown TEXT NOT NULL,
            summary TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        );",
    )
    .map_err(|e| format!("graph.db schema: {e}"))?;
    let _ = conn.execute(
        "ALTER TABLE nodes ADD COLUMN signature TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE nodes ADD COLUMN content_hash TEXT NOT NULL DEFAULT ''",
        [],
    );
    Ok(conn)
}

fn sha256_short(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let full = format!("{digest:x}");
    full.chars().take(16).collect()
}

/// Compute a normalized content hash for a source file or 1-based line range `[start_line..=end_line]`.
pub fn compute_anchor_hash(
    root: &Path,
    rel_path: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Option<String> {
    let norm = rel_path.trim_start_matches('/').replace('\\', "/");
    let content = std::fs::read_to_string(root.join(&norm)).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Some(sha256_short(""));
    }
    let slice = match (start_line, end_line) {
        (Some(s), Some(e)) if s >= 1 && e >= s => {
            let start_idx = (s - 1).min(lines.len().saturating_sub(1));
            let end_idx = (e - 1).min(lines.len().saturating_sub(1));
            if start_idx > end_idx {
                return None;
            }
            &lines[start_idx..=end_idx]
        }
        (Some(s), None) if s >= 1 => {
            let start_idx = (s - 1).min(lines.len().saturating_sub(1));
            let end_idx = (start_idx + 25).min(lines.len().saturating_sub(1));
            &lines[start_idx..=end_idx]
        }
        _ => &lines[..],
    };
    let normalized = slice
        .iter()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n");
    Some(sha256_short(&normalized))
}

fn sync_from_graph_doc_conn(conn: &Connection, root: &Path, doc: &GraphDoc) -> Result<(), String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("tx: {e}"))?;
    tx.execute("DELETE FROM edges", []).ok();
    tx.execute("DELETE FROM nodes", []).ok();
    tx.execute("DELETE FROM nodes_fts", []).ok();
    tx.execute("DELETE FROM meta", []).ok();
    tx.execute("DELETE FROM pending_sync", []).ok();

    let mut sym_name_to_ids: HashMap<String, Vec<String>> = HashMap::new();
    let mut deferred_calls: Vec<(String, String)> = Vec::new();

    for n in &doc.nodes {
        let mut file_summary = n.summary.clone();
        let mut file_hash = String::new();
        let mut file_skeleton = None;

        if n.kind == "file" {
            if let Some(rel) = n.path.as_deref() {
                let abs = root.join(rel);
                if let Ok(raw) = crate::bounded_read::read(&abs, 600_000) {
                    if !raw[..raw.len().min(4096)].contains(&0) {
                        let source = String::from_utf8_lossy(&raw);
                        file_hash = sha256_short(&source);
                        if let Some(skel) =
                            crate::commands::symbols::extract_file_skeleton(&abs, rel, &source)
                        {
                            if file_summary.is_empty() && !skel.file_doc.is_empty() {
                                file_summary = skel.file_doc.clone();
                            }
                            file_skeleton = Some((rel.to_string(), n.layer.clone(), skel));
                        }
                    }
                }
            }
        }

        tx.execute(
            "INSERT INTO nodes (id, kind, path, name, layer, summary, start_line, end_line, signature, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, '', ?7)",
            params![n.id, n.kind, n.path, n.name, n.layer, file_summary, file_hash],
        )
        .map_err(|e| format!("insert node {}: {e}", n.id))?;

        if let Some((rel, layer, skel)) = file_skeleton {
            let mut local_sym_ids: Vec<String> = Vec::with_capacity(skel.symbols.len());
            for sym in &skel.symbols {
                let sym_id = format!("sym:{rel}:{}:{}", sym.start_line, sym.name);
                let summary_text = if sym.doc.is_empty() {
                    sym.signature.clone()
                } else if sym.signature.is_empty() {
                    sym.doc.clone()
                } else {
                    format!("{} — {}", sym.signature, sym.doc)
                };
                tx.execute(
                    "INSERT OR REPLACE INTO nodes (id, kind, path, name, layer, summary, start_line, end_line, signature, content_hash)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '')",
                    params![
                        sym_id,
                        sym.kind,
                        rel,
                        sym.name,
                        layer,
                        summary_text,
                        sym.start_line as i64,
                        sym.end_line as i64,
                        sym.signature
                    ],
                )
                .ok();
                tx.execute(
                    "INSERT OR IGNORE INTO edges (from_id, to_id, kind, weight) VALUES (?1, ?2, 'contains', 1)",
                    params![n.id, sym_id],
                )
                .ok();
                sym_name_to_ids
                    .entry(sym.name.clone())
                    .or_default()
                    .push(sym_id.clone());
                local_sym_ids.push(sym_id);
            }
            for call in skel.calls {
                let from_id = call
                    .caller
                    .and_then(|idx| local_sym_ids.get(idx).cloned())
                    .unwrap_or_else(|| n.id.clone());
                deferred_calls.push((from_id, call.callee));
            }
        }
    }

    for e in &doc.edges {
        tx.execute(
            "INSERT OR IGNORE INTO edges (from_id, to_id, kind, weight) VALUES (?1, ?2, ?3, 1)",
            params![e.from, e.to, e.kind],
        )
        .map_err(|e| format!("insert edge: {e}"))?;
    }

    for (from_id, callee_name) in deferred_calls {
        if let Some(targets) = sym_name_to_ids.get(&callee_name) {
            if targets.len() <= 6 {
                for to_id in targets {
                    if &from_id != to_id {
                        tx.execute(
                            "INSERT OR IGNORE INTO edges (from_id, to_id, kind, weight) VALUES (?1, ?2, 'calls', 1)",
                            params![from_id, to_id],
                        )
                        .ok();
                    }
                }
            }
        }
    }

    tx.execute(
        "INSERT INTO meta (key, value) VALUES ('version', ?1), ('generated_at', ?2)",
        params![GRAPH_DB_VERSION, doc.generated_at],
    )
    .map_err(|e| format!("meta: {e}"))?;
    tx.execute("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')", [])
        .map_err(|e| format!("fts rebuild: {e}"))?;
    tx.commit().map_err(|e| format!("commit: {e}"))?;
    let _ = verify_all_repo_notes_conn(conn, root);
    Ok(())
}

/// Replace graph.db contents from a GraphDoc (including AST symbols and call edges).
pub fn sync_from_graph_doc(root: &Path, doc: &GraphDoc) -> Result<(), String> {
    let _guard = graph_db_lock();
    let conn = open_graph_db_inner(root)?;
    sync_from_graph_doc_conn(&conn, root, doc)
}

fn ensure_synced(root: &Path) -> Result<Connection, String> {
    let conn = open_graph_db_inner(root)?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .unwrap_or(0);
    if count > 0 {
        return Ok(conn);
    }
    let doc = crate::commands::graph::load_graph(root)
        .ok_or("graph index empty — run Wiki → Rebuild graph first".to_string())?;
    sync_from_graph_doc_conn(&conn, root, &doc)?;
    Ok(conn)
}

#[derive(Debug, Clone)]
struct NodeRow {
    id: String,
    kind: String,
    path: Option<String>,
    name: String,
    layer: Option<String>,
    start_line: Option<usize>,
    end_line: Option<usize>,
    signature: String,
}

const SELECT_NODE_COLS: &str =
    "id, kind, path, name, layer, start_line, end_line, COALESCE(signature, '')";

fn search_nodes(
    conn: &Connection,
    root: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<NodeRow>, String> {
    let q = query.trim();
    if q.is_empty() {
        return list_top_files(conn, limit);
    }

    let fts_query = q
        .split_whitespace()
        .filter(|w| !w.starts_with('@'))
        .map(|w| {
            let cleaned: String = w.chars().filter(|c| *c != '"').collect();
            format!("\"{cleaned}\"")
        })
        .collect::<Vec<_>>()
        .join(" OR ");

    let mut rows: Vec<NodeRow> = Vec::new();
    if !fts_query.is_empty() {
        let mut stmt = conn
            .prepare(
                "SELECT n.id, n.kind, n.path, n.name, n.layer, n.start_line, n.end_line, COALESCE(n.signature, '')
                 FROM nodes_fts f
                 JOIN nodes n ON n.rowid = f.rowid
                 WHERE nodes_fts MATCH ?1
                   AND (n.path IS NULL OR n.path NOT LIKE 'bundled/preinstall/%')
                 ORDER BY CASE WHEN lower(n.name) = lower(?3) THEN 0 ELSE 1 END, bm25(nodes_fts)
                 LIMIT ?2",
            )
            .map_err(|e| format!("fts prepare: {e}"))?;
        rows.extend(
            stmt.query_map(params![fts_query, limit as i64, q], map_node_row)
                .map_err(|e| format!("fts query: {e}"))?
                .filter_map(|r| r.ok()),
        );
    }

    if rows.len() < limit {
        let like = format!("%{}%", q.to_lowercase());
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {SELECT_NODE_COLS} FROM nodes
                 WHERE (lower(name) LIKE ?1 OR lower(COALESCE(path,'')) LIKE ?1)
                   AND (path IS NULL OR path NOT LIKE 'bundled/preinstall/%')
                 ORDER BY CASE WHEN lower(name) = lower(?3) THEN 0 WHEN id LIKE 'sym:%' THEN 1 ELSE 2 END
                 LIMIT ?2"
            ))
            .map_err(|e| format!("like prepare: {e}"))?;
        for row in stmt
            .query_map(params![like, limit as i64, q], map_node_row)
            .map_err(|e| format!("like query: {e}"))?
            .flatten()
        {
            if !rows.iter().any(|n| n.id == row.id) {
                rows.push(row);
            }
            if rows.len() >= limit {
                break;
            }
        }
    }

    if rows.is_empty() {
        rows = search_nodes_by_content(conn, root, q, limit)?;
    }

    Ok(rows.into_iter().take(limit).collect())
}

fn query_tokens(q: &str) -> Vec<String> {
    q.to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '_' || c == '-' || c == '/' || c == '.')
        .filter(|t| t.len() >= 2)
        .map(String::from)
        .collect()
}

fn path_prefilter_score(path: &str, tokens: &[String], full_q: &str) -> i32 {
    let p = path.to_lowercase();
    let name = path.rsplit('/').next().unwrap_or("").to_lowercase();
    let mut score = 0;
    if p.contains(full_q) {
        score += 30;
    }
    for t in tokens {
        if name.contains(t.as_str()) {
            score += 8;
        } else if p.contains(t.as_str()) {
            score += 4;
        }
    }
    if full_q.contains('_') && p.ends_with(".rs") {
        score += 3;
    }
    score
}

fn list_indexed_files(conn: &Connection) -> Result<Vec<NodeRow>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_NODE_COLS} FROM nodes
             WHERE kind = 'file' AND path IS NOT NULL
               AND path NOT LIKE 'bundled/preinstall/%'"
        ))
        .map_err(|e| format!("list files prepare: {e}"))?;
    let rows: Vec<NodeRow> = stmt
        .query_map([], map_node_row)
        .map_err(|e| format!("list files query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Content fallback: grep indexed source when FTS/path miss a symbol name.
fn search_nodes_by_content(
    conn: &Connection,
    root: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<NodeRow>, String> {
    let q = query.to_lowercase();
    if q.len() < 2 {
        return Ok(Vec::new());
    }
    let tokens = query_tokens(&q);
    let files = list_indexed_files(conn)?;
    let symbol_query = q.contains('_') || query.chars().any(|c| c.is_ascii_uppercase());

    let mut scored: Vec<(i32, NodeRow)> = Vec::new();
    for node in files {
        let Some(rel) = node.path.as_ref() else {
            continue;
        };
        let pre = path_prefilter_score(rel, &tokens, &q);
        if pre == 0 && !symbol_query && tokens.len() > 2 {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(root.join(rel)) else {
            if pre > 0 {
                scored.push((pre, node));
            }
            continue;
        };
        let lower = content.to_lowercase();
        let mut score = pre;
        if lower.contains(&q) {
            score += 100;
        }
        for line in content.lines() {
            let trimmed = line.trim().to_lowercase();
            if (trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("function ")
                || trimmed.starts_with("export function ")
                || trimmed.starts_with("export async function ")
                || trimmed.starts_with("def "))
                && trimmed.contains(&q)
            {
                score += 250;
                break;
            }
        }
        for t in &tokens {
            if lower.contains(t.as_str()) {
                score += 2;
            }
        }
        if rel.contains("/examples/") || rel.contains("/tests/") {
            score -= 50;
        }
        if score > 0 {
            scored.push((score, node));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    Ok(scored.into_iter().take(limit).map(|(_, n)| n).collect())
}

fn grep_symbol_callers(
    root: &Path,
    conn: &Connection,
    symbol: &str,
    def_paths: &BTreeSet<String>,
) -> Vec<String> {
    let sym = symbol.trim();
    // Support both snake_case (Rust/Python) and camelCase/PascalCase (TS/JS/Go) symbols.
    let valid_ident = sym.len() >= 3
        && !sym.contains('/')
        && !sym.contains('.')
        && !sym.contains(' ')
        && sym.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid_ident {
        return Vec::new();
    }
    let mut lines = Vec::new();

    // 1. AST call graph edges from `edges` table (`calls` and `ipc_calls`).
    if let Ok(mut stmt) = conn.prepare(
        "SELECT fn_from.name, fn_from.path, fn_from.start_line, fn_to.name, fn_to.path, e.kind
         FROM edges e
         JOIN nodes fn_from ON fn_from.id = e.from_id
         JOIN nodes fn_to ON fn_to.id = e.to_id
         WHERE e.kind IN ('calls', 'ipc_calls')
           AND (lower(fn_to.name) = lower(?1) OR lower(fn_from.name) = lower(?1))
         LIMIT ?2",
    ) {
        if let Ok(rows) = stmt.query_map(params![sym, MAX_BLAST_ITEMS as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
            ))
        }) {
            for (from_name, from_path, from_line, to_name, to_path, kind) in rows.flatten() {
                let from_loc = match (from_path.as_deref(), from_line) {
                    (Some(p), Some(l)) => format!("{p}:{l}"),
                    (Some(p), None) => p.to_string(),
                    _ => from_name.clone(),
                };
                let to_loc = to_path.unwrap_or_else(|| to_name.clone());
                lines.push(format!(
                    "- `{from_name}` (`{from_loc}`) --[{kind}]--> `{to_name}` (`{to_loc}`)"
                ));
            }
        }
    }

    if lines.len() >= MAX_BLAST_ITEMS {
        lines.truncate(MAX_BLAST_ITEMS);
        return lines;
    }

    // 2. Supplement with direct source call-site scan (covers dynamic calls, JSX, or Tauri invoke("sym")).
    let call_needle = format!("{sym}(");
    let ipc_needle_dq = format!("\"{sym}\"");
    let ipc_needle_sq = format!("'{sym}'");
    let files = list_indexed_files(conn).unwrap_or_default();

    for node in files {
        let Some(rel) = node.path.as_ref() else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(root.join(rel)) else {
            continue;
        };
        for (i, line) in content.lines().enumerate() {
            let is_ipc = line.contains("invoke")
                && (line.contains(&ipc_needle_dq) || line.contains(&ipc_needle_sq));
            let is_call = line.contains(&call_needle)
                && !line.contains(&format!("\"{call_needle}"))
                && !line.contains(&format!("'{call_needle}"));
            if !is_call && !is_ipc {
                continue;
            }
            if line.contains("std::fs::write") || line.contains("assert!(out.contains") {
                continue;
            }
            let t = line.trim_start();
            if (t.starts_with("fn ")
                || t.starts_with("pub fn ")
                || t.starts_with("pub async fn ")
                || t.starts_with("async fn ")
                || t.starts_with("function ")
                || t.starts_with("export function "))
                && line.contains(sym)
            {
                continue;
            }
            let loc = format!("{rel}:{}", i + 1);
            let role = if is_ipc {
                "ipc invoke"
            } else if def_paths.contains(rel) {
                "caller"
            } else {
                "reference"
            };
            let entry = format!("- `{sym}` ({role}) at `{loc}`");
            if !lines.contains(&entry) {
                lines.push(entry);
            }
            if lines.len() >= MAX_BLAST_ITEMS {
                return lines;
            }
        }
    }
    lines
}

fn map_node_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<NodeRow> {
    Ok(NodeRow {
        id: row.get(0)?,
        kind: row.get(1)?,
        path: row.get(2)?,
        name: row.get(3)?,
        layer: row.get(4)?,
        start_line: row.get::<_, Option<i64>>(5)?.map(|v| v.max(1) as usize),
        end_line: row.get::<_, Option<i64>>(6)?.map(|v| v.max(1) as usize),
        signature: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
    })
}

fn list_top_files(conn: &Connection, limit: usize) -> Result<Vec<NodeRow>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_NODE_COLS} FROM nodes
             WHERE kind = 'file'
             ORDER BY path
             LIMIT ?1"
        ))
        .map_err(|e| format!("list files: {e}"))?;
    let rows: Vec<NodeRow> = stmt
        .query_map(params![limit as i64], map_node_row)
        .map_err(|e| format!("list query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

fn read_numbered_file(root: &Path, rel: &str, max_lines: usize) -> Option<String> {
    read_numbered_range(root, rel, 1, max_lines)
}

fn read_numbered_range(
    root: &Path,
    rel: &str,
    start_line_1based: usize,
    max_lines: usize,
) -> Option<String> {
    let abs = root.join(rel);
    let content = std::fs::read_to_string(abs).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let start_idx = start_line_1based.saturating_sub(1).min(lines.len());
    let mut out = String::new();
    for (offset, line) in lines[start_idx..].iter().enumerate().take(max_lines) {
        out.push_str(&format!("{}\t{line}\n", start_idx + offset + 1));
    }
    if start_idx + max_lines < lines.len() {
        out.push_str("\n… [truncated]\n");
    }
    Some(out)
}

fn blast_radius(conn: &Connection, node_id: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut stmt = match conn.prepare(
        "SELECT e.from_id, n.path, n.name, e.kind, n.start_line
         FROM edges e
         JOIN nodes n ON n.id = e.from_id
         WHERE e.to_id = ?1 AND e.kind IN ('imports', 'calls', 'ipc_calls', 'depends')
         LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(_) => return lines,
    };
    let Ok(rows) = stmt.query_map(params![node_id, MAX_BLAST_ITEMS as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<i64>>(4)?,
        ))
    }) else {
        return lines;
    };
    for row in rows.flatten() {
        let (from_id, path, name, kind, start_line) = row;
        let loc = match (path, start_line) {
            (Some(p), Some(l)) => format!("{p}:{l}"),
            (Some(p), None) => p,
            _ => from_id.clone(),
        };
        lines.push(format!("- `{name}` (`{loc}`) --[{kind}]--> this"));
    }
    lines
}

fn one_hop_edges(conn: &Connection, seed_ids: &HashSet<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut stmt = match conn.prepare(
        "SELECT from_id, to_id, kind FROM edges
         WHERE kind IN ('imports', 'depends', 'calls', 'ipc_calls')
         LIMIT 60",
    ) {
        Ok(s) => s,
        Err(_) => return out,
    };
    let Ok(rows) = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    }) else {
        return out;
    };
    for row in rows.flatten() {
        let (from, to, kind) = row;
        if seed_ids.contains(from.as_str()) || seed_ids.contains(to.as_str()) {
            out.push(format!("- `{from}` --[{kind}]--> `{to}`"));
        }
        if out.len() >= 20 {
            break;
        }
    }
    out
}

fn pending_banner(conn: &Connection) -> Option<String> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM pending_sync", [], |r| r.get(0))
        .unwrap_or(0);
    if count == 0 {
        return None;
    }
    Some(format!(
        "⚠️ **Pending re-index:** {count} file(s) changed recently — Read edited files directly for live content.\n"
    ))
}

/// Explore the codebase graph (CodeGraph-style markdown for agents, enriched with AST symbols, IPC edges & Repo Memory).
pub fn explore_graph(root: &Path, query: &str, limit: usize) -> Result<String, String> {
    let _guard = graph_db_lock();
    let limit = limit.clamp(1, 25);
    let conn = ensure_synced(root)?;
    let nodes = search_nodes(&conn, root, query, limit)?;

    if nodes.is_empty() {
        return Ok(format!(
            "**Exploration: {query}**\n\nNo matches in graph index. Try `codebase_search` or rebuild the graph."
        ));
    }

    let file_paths: BTreeSet<String> = nodes.iter().filter_map(|n| n.path.clone()).collect();
    let mut files_ordered: Vec<String> = Vec::new();
    for n in &nodes {
        if let Some(p) = &n.path {
            if !files_ordered.iter().any(|f| f == p) {
                files_ordered.push(p.clone());
            }
        }
    }
    let mut seed_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    for rel in &file_paths {
        seed_ids.insert(format!("file:{rel}"));
    }
    let seed_refs: HashSet<&str> = seed_ids.iter().map(|s| s.as_str()).collect();

    let mut out = String::new();
    if let Some(banner) = pending_banner(&conn) {
        out.push_str(&banner);
        out.push('\n');
    }
    out.push_str(&format!("**Exploration: {query}**\n\n"));
    out.push_str(&format!(
        "Found {} node(s) across {} file(s).\n\n",
        nodes.len(),
        file_paths.len().max(1)
    ));

    // Attach matching Repository Memory notes (invariants / pitfalls / dataflow)
    let matched_notes = find_notes_for_exploration(&conn, query, &file_paths);
    if !matched_notes.is_empty() {
        out.push_str("**Repository Memory Notes (Source-Anchored)**\n\n");
        for note in &matched_notes {
            let status_tag = if note.stale {
                "⚠️ [需核实 / STALE: 关联源码已变更]"
            } else {
                "✓ [ACTIVE]"
            };
            let cites = note
                .citations
                .iter()
                .map(format_citation_tag)
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "- {status_tag} **[{}] {}** (`{}`): {}",
                note.kind, note.title, note.scope_target, note.body
            ));
            if !cites.is_empty() {
                out.push_str(&format!(" — refs: {cites}"));
            }
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str("**Blast radius — what depends on these (update/verify before editing)**\n\n");
    let mut blast_seen = BTreeSet::new();
    for id in &seed_ids {
        for line in blast_radius(&conn, id) {
            if blast_seen.insert(line.clone()) {
                out.push_str(&line);
                out.push('\n');
            }
        }
    }
    if blast_seen.is_empty() {
        out.push_str("_No indexed importers or callers in 1-hop._\n");
    }
    out.push('\n');

    let caller_lines = grep_symbol_callers(root, &conn, query.trim(), &file_paths);
    if !caller_lines.is_empty() {
        out.push_str("**Call / IPC reference sites**\n\n");
        for line in caller_lines {
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    let flow = one_hop_edges(&conn, &seed_refs);
    if !flow.is_empty() {
        out.push_str("**Import / call / IPC flow (1-hop)**\n\n");
        for line in flow {
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str("**Source Code**\n\n");
    out.push_str("> Verbatim on-disk source with line numbers — treat as already Read.\n\n");

    // First emit focused symbol definitions when AST symbol nodes matched.
    let mut rendered_files = BTreeSet::new();
    for sym_node in nodes
        .iter()
        .filter(|n| n.id.starts_with("sym:") && n.start_line.is_some())
        .take(6)
    {
        let Some(rel) = sym_node.path.as_deref() else {
            continue;
        };
        let s_line = sym_node.start_line.unwrap_or(1);
        let e_line = sym_node.end_line.unwrap_or(s_line + 30);
        let context_start = s_line.saturating_sub(3).max(1);
        let span = (e_line.saturating_sub(context_start) + 4).clamp(12, MAX_SNIPPET_LINES);
        out.push_str(&format!(
            "**`{rel}:{s_line}-{e_line}`** — `{}` ({})\n\n",
            if sym_node.signature.is_empty() {
                &sym_node.name
            } else {
                &sym_node.signature
            },
            sym_node.kind
        ));
        out.push_str("```\n");
        if let Some(body) = read_numbered_range(root, rel, context_start, span) {
            out.push_str(&body);
        } else {
            out.push_str("(unable to read file)\n");
        }
        out.push_str("```\n\n");
        rendered_files.insert(rel.to_string());
    }

    for rel in files_ordered.iter().take(8) {
        if rendered_files.contains(rel) {
            continue;
        }
        let node = nodes
            .iter()
            .find(|n| n.path.as_deref() == Some(rel.as_str()));
        let label = node
            .map(|n| {
                let mut s = format!("{} ({})", n.name, n.kind);
                if let Some(ref layer) = n.layer {
                    s.push_str(&format!(", {layer}"));
                }
                s
            })
            .unwrap_or_else(|| rel.clone());
        out.push_str(&format!("**`{rel}`** — {label}\n\n"));
        out.push_str("```\n");
        if let Some(body) = read_numbered_file(root, rel, MAX_SNIPPET_LINES) {
            out.push_str(&body);
        } else {
            out.push_str("(unable to read file)\n");
        }
        out.push_str("```\n\n");
    }

    Ok(out)
}

/// Clear pending staleness after a successful full sync.
pub fn clear_pending_sync(root: &Path) -> Result<(), String> {
    let conn = open_graph_db(root)?;
    conn.execute("DELETE FROM pending_sync", [])
        .map_err(|e| format!("clear pending_sync: {e}"))?;
    Ok(())
}

/// Incrementally re-sync a single file's AST symbols, call edges, and anchored `repo_notes` staleness.
pub fn sync_file_in_graph_db(root: &Path, rel: &str) -> Result<(), String> {
    let _guard = graph_db_lock();
    let conn = open_graph_db_inner(root)?;
    let norm = rel.trim_start_matches('/').replace('\\', "/");
    let file_id = format!("file:{norm}");
    let sym_prefix = format!("sym:{norm}:%");

    // Remove previous symbol nodes and outgoing edges for this file.
    conn.execute(
        "DELETE FROM edges WHERE from_id LIKE ?1 OR to_id LIKE ?1 OR (from_id = ?2 AND kind = 'contains')",
        params![sym_prefix, file_id],
    )
    .ok();
    conn.execute("DELETE FROM nodes WHERE id LIKE ?1", params![sym_prefix])
        .ok();

    let abs = root.join(&norm);
    if abs.is_file() {
        if let Ok(raw) = crate::bounded_read::read(&abs, 600_000) {
            if !raw[..raw.len().min(4096)].contains(&0) {
                let source = String::from_utf8_lossy(&raw);
                let file_hash = sha256_short(&source);
                conn.execute(
                    "UPDATE nodes SET content_hash = ?1 WHERE id = ?2",
                    params![file_hash, file_id],
                )
                .ok();
                if let Some(skel) =
                    crate::commands::symbols::extract_file_skeleton(&abs, &norm, &source)
                {
                    let mut local_sym_ids = Vec::with_capacity(skel.symbols.len());
                    for sym in &skel.symbols {
                        let sym_id = format!("sym:{norm}:{}:{}", sym.start_line, sym.name);
                        let summary_text = if sym.doc.is_empty() {
                            sym.signature.clone()
                        } else if sym.signature.is_empty() {
                            sym.doc.clone()
                        } else {
                            format!("{} — {}", sym.signature, sym.doc)
                        };
                        conn.execute(
                            "INSERT OR REPLACE INTO nodes (id, kind, path, name, layer, summary, start_line, end_line, signature, content_hash)
                             VALUES (?1, ?2, ?3, ?4, 'unknown', ?5, ?6, ?7, ?8, '')",
                            params![
                                sym_id,
                                sym.kind,
                                norm,
                                sym.name,
                                summary_text,
                                sym.start_line as i64,
                                sym.end_line as i64,
                                sym.signature
                            ],
                        )
                        .ok();
                        conn.execute(
                            "INSERT OR IGNORE INTO edges (from_id, to_id, kind, weight) VALUES (?1, ?2, 'contains', 1)",
                            params![file_id, sym_id],
                        )
                        .ok();
                        local_sym_ids.push(sym_id);
                    }
                    for call in skel.calls {
                        let from_id = call
                            .caller
                            .and_then(|idx| local_sym_ids.get(idx).cloned())
                            .unwrap_or_else(|| file_id.clone());
                        if let Ok(mut stmt) = conn.prepare(
                            "SELECT id FROM nodes WHERE id LIKE 'sym:%' AND name = ?1 LIMIT 6",
                        ) {
                            if let Ok(rows) = stmt.query_map(params![call.callee], |r| r.get::<_, String>(0)) {
                                for to_id in rows.flatten() {
                                    if to_id != from_id {
                                        conn.execute(
                                            "INSERT OR IGNORE INTO edges (from_id, to_id, kind, weight) VALUES (?1, ?2, 'calls', 1)",
                                            params![from_id, to_id],
                                        )
                                        .ok();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    conn.execute("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')", [])
        .ok();
    let changed = verify_repo_notes_for_file_conn(&conn, root, &norm).unwrap_or(0);
    drop(_guard);
    if changed > 0 || root.join(".agentz").join("REPO_MEMORY.md").is_file() {
        let _ = export_repo_memory_markdown(root);
    }
    Ok(())
}

/// Mark a file as pending re-index and immediately verify anchored repo notes + incremental symbols.
pub fn mark_pending_sync(root: &Path, rel: &str) -> Result<(), String> {
    let norm = rel.trim_start_matches('/').replace('\\', "/");
    {
        let _guard = graph_db_lock();
        let conn = open_graph_db_inner(root)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO pending_sync (path, updated_at) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET updated_at = excluded.updated_at",
            params![norm, now],
        )
        .map_err(|e| format!("pending_sync: {e}"))?;
    }
    let _ = sync_file_in_graph_db(root, &norm);
    Ok(())
}

// ---------------------------------------------------------------------------
// Anchored Repository Memory (`repo_notes` + `.agentz/REPO_MEMORY.md`)
// ---------------------------------------------------------------------------

fn format_citation_tag(c: &NoteCitation) -> String {
    match (c.start_line, c.end_line) {
        (Some(s), Some(e)) if s != e => format!("`{}:{s}-{e}`", c.path),
        (Some(s), _) => format!("`{}:{s}`", c.path),
        _ => format!("`{}`", c.path),
    }
}

fn map_repo_note_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RepoNote> {
    let raw_cites: String = row.get(6)?;
    let citations: Vec<NoteCitation> = serde_json::from_str(&raw_cites).unwrap_or_default();
    let stale_int: i64 = row.get(8)?;
    Ok(RepoNote {
        id: row.get(0)?,
        scope_kind: row.get(1)?,
        scope_target: row.get(2)?,
        kind: row.get(3)?,
        title: row.get(4)?,
        body: row.get(5)?,
        citations,
        source: row.get(7)?,
        stale: stale_int != 0,
        updated_at: row.get(9)?,
    })
}

fn find_notes_for_exploration(
    conn: &Connection,
    query: &str,
    file_paths: &BTreeSet<String>,
) -> Vec<RepoNote> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, scope_kind, scope_target, kind, title, body, citations, source, stale, updated_at
         FROM repo_notes
         ORDER BY stale ASC, updated_at DESC
         LIMIT 50",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], map_repo_note_row) else {
        return Vec::new();
    };
    let q_lower = query.trim().to_lowercase();
    let modules: BTreeSet<String> = file_paths
        .iter()
        .map(|p| crate::commands::graph::logical_module(p))
        .collect();
    let mut matched = Vec::new();
    for note in rows.flatten() {
        let target_norm = note.scope_target.replace('\\', "/");
        let hits_file = file_paths
            .iter()
            .any(|p| target_norm == *p || target_norm.starts_with(&format!("{p}#")));
        let hits_module = modules.contains(&target_norm);
        let hits_cite = note
            .citations
            .iter()
            .any(|c| file_paths.contains(&c.path.replace('\\', "/")));
        let hits_query = !q_lower.is_empty()
            && (note.title.to_lowercase().contains(&q_lower)
                || note.scope_target.to_lowercase().contains(&q_lower)
                || note.body.to_lowercase().contains(&q_lower));
        if hits_file || hits_module || hits_cite || hits_query {
            matched.push(note);
            if matched.len() >= 6 {
                break;
            }
        }
    }
    matched
}

fn verify_citation_against_disk(root: &Path, conn: &Connection, c: &NoteCitation) -> bool {
    let norm = c.path.trim_start_matches('/').replace('\\', "/");
    if !root.join(&norm).is_file() {
        return false;
    }
    if let Some(ref sym_name) = c.symbol {
        if !sym_name.is_empty() {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM nodes WHERE path = ?1 AND name = ?2",
                    params![norm, sym_name],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if exists == 0 {
                // Fallback: check raw file text for symbol name
                if let Ok(text) = std::fs::read_to_string(root.join(&norm)) {
                    if !text.contains(sym_name) {
                        return false;
                    }
                }
            }
        }
    }
    if !c.hash.is_empty() {
        let Some(current_hash) = compute_anchor_hash(root, &norm, c.start_line, c.end_line) else {
            return false;
        };
        if current_hash != c.hash {
            return false;
        }
    }
    true
}

fn verify_repo_notes_for_file_conn(
    conn: &Connection,
    root: &Path,
    rel_path: &str,
) -> Result<usize, String> {
    let norm = rel_path.trim_start_matches('/').replace('\\', "/");
    let mut stmt = conn
        .prepare(
            "SELECT id, scope_kind, scope_target, kind, title, body, citations, source, stale, updated_at
             FROM repo_notes",
        )
        .map_err(|e| format!("prepare notes: {e}"))?;
    let notes: Vec<RepoNote> = stmt
        .query_map([], map_repo_note_row)
        .map_err(|e| format!("query notes: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let mut updated = 0usize;
    for note in notes {
        let touches_file = note.scope_target.replace('\\', "/").starts_with(&norm)
            || note
                .citations
                .iter()
                .any(|c| c.path.trim_start_matches('/').replace('\\', "/") == norm);
        if !touches_file {
            continue;
        }
        let all_valid = if note.citations.is_empty() {
            root.join(&norm).is_file()
        } else {
            note.citations
                .iter()
                .all(|c| verify_citation_against_disk(root, conn, c))
        };
        let new_stale = !all_valid;
        if new_stale != note.stale {
            conn.execute(
                "UPDATE repo_notes SET stale = ?1 WHERE id = ?2",
                params![if new_stale { 1 } else { 0 }, note.id],
            )
            .ok();
            updated += 1;
        }
    }
    Ok(updated)
}

fn verify_all_repo_notes_conn(conn: &Connection, root: &Path) -> Result<usize, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, scope_kind, scope_target, kind, title, body, citations, source, stale, updated_at
             FROM repo_notes",
        )
        .map_err(|e| format!("prepare notes: {e}"))?;
    let notes: Vec<RepoNote> = stmt
        .query_map([], map_repo_note_row)
        .map_err(|e| format!("query notes: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let mut updated = 0usize;
    for note in notes {
        if note.citations.is_empty() {
            continue;
        }
        let all_valid = note
            .citations
            .iter()
            .all(|c| verify_citation_against_disk(root, conn, c));
        let new_stale = !all_valid;
        if new_stale != note.stale {
            conn.execute(
                "UPDATE repo_notes SET stale = ?1 WHERE id = ?2",
                params![if new_stale { 1 } else { 0 }, note.id],
            )
            .ok();
            updated += 1;
        }
    }
    Ok(updated)
}

/// Record or update an anchored repository note and refresh `.agentz/REPO_MEMORY.md`.
pub fn record_repo_note(root: &Path, mut input: RepoNoteInput) -> Result<RepoNote, String> {
    let title = input.title.trim().to_string();
    let body = input.body.trim().to_string();
    if title.is_empty() || body.is_empty() {
        return Err("repo_note requires non-empty title and body".into());
    }
    let scope_kind = match input.scope_kind.trim() {
        "module" | "file" | "symbol" | "architecture" => input.scope_kind.trim().to_string(),
        _ => "module".to_string(),
    };
    let kind = match input.kind.trim() {
        "invariant" | "pitfall" | "dataflow" | "decision" => input.kind.trim().to_string(),
        _ => "invariant".to_string(),
    };
    let scope_target = input
        .scope_target
        .trim()
        .trim_start_matches('/')
        .replace('\\', "/");

    // Compute current source hashes for each citation anchor.
    for cite in &mut input.citations {
        cite.path = cite
            .path
            .trim()
            .trim_start_matches('/')
            .replace('\\', "/");
        if let Some(h) = compute_anchor_hash(root, &cite.path, cite.start_line, cite.end_line) {
            cite.hash = h;
        }
    }

    let id = format!(
        "note_{}",
        sha256_short(&format!("{scope_kind}:{scope_target}:{kind}:{}", title.to_lowercase()))
    );
    let now = chrono::Utc::now().to_rfc3339();
    let citations_json =
        serde_json::to_string(&input.citations).unwrap_or_else(|_| "[]".to_string());

    let note = RepoNote {
        id: id.clone(),
        scope_kind: scope_kind.clone(),
        scope_target: scope_target.clone(),
        kind: kind.clone(),
        title: title.clone(),
        body: body.clone(),
        citations: input.citations,
        source: input.source.clone(),
        stale: false,
        updated_at: now.clone(),
    };

    {
        let _guard = graph_db_lock();
        let conn = open_graph_db_inner(root)?;
        conn.execute(
            "INSERT INTO repo_notes (id, scope_kind, scope_target, kind, title, body, citations, source, stale, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)
             ON CONFLICT(id) DO UPDATE SET
               scope_kind = excluded.scope_kind,
               scope_target = excluded.scope_target,
               kind = excluded.kind,
               title = excluded.title,
               body = excluded.body,
               citations = excluded.citations,
               source = excluded.source,
               stale = 0,
               updated_at = excluded.updated_at",
            params![
                id,
                scope_kind,
                scope_target,
                kind,
                title,
                body,
                citations_json,
                input.source,
                now
            ],
        )
        .map_err(|e| format!("insert repo_note: {e}"))?;
    }
    let _ = export_repo_memory_markdown(root);
    Ok(note)
}

/// Query repository memory notes by optional keyword/path/module filter.
pub fn query_repo_notes(
    root: &Path,
    query: Option<&str>,
    include_stale: bool,
    limit: usize,
) -> Result<Vec<RepoNote>, String> {
    if !graph_db_path(root).is_file() {
        return Ok(Vec::new());
    }
    let _guard = graph_db_lock();
    let conn = open_graph_db_inner(root)?;
    let _ = verify_all_repo_notes_conn(&conn, root);
    let mut stmt = conn
        .prepare(
            "SELECT id, scope_kind, scope_target, kind, title, body, citations, source, stale, updated_at
             FROM repo_notes
             ORDER BY stale ASC, updated_at DESC",
        )
        .map_err(|e| format!("query repo_notes: {e}"))?;
    let all: Vec<RepoNote> = stmt
        .query_map([], map_repo_note_row)
        .map_err(|e| format!("map repo_notes: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let q = query.unwrap_or("").trim().to_lowercase();
    let filtered: Vec<RepoNote> = all
        .into_iter()
        .filter(|n| include_stale || !n.stale)
        .filter(|n| {
            if q.is_empty() {
                return true;
            }
            n.id.to_lowercase() == q
                || n.scope_target.to_lowercase().contains(&q)
                || n.title.to_lowercase().contains(&q)
                || n.body.to_lowercase().contains(&q)
                || n.kind.to_lowercase().contains(&q)
                || n.citations
                    .iter()
                    .any(|c| c.path.to_lowercase().contains(&q))
        })
        .take(limit.clamp(1, 100))
        .collect();
    Ok(filtered)
}

/// Re-anchor a note against current on-disk source and clear its `stale` flag.
pub fn verify_repo_note(root: &Path, note_id: &str) -> Result<RepoNote, String> {
    let mut note = {
        let _guard = graph_db_lock();
        let conn = open_graph_db_inner(root)?;
        conn.query_row(
            "SELECT id, scope_kind, scope_target, kind, title, body, citations, source, stale, updated_at
             FROM repo_notes WHERE id = ?1",
            params![note_id.trim()],
            map_repo_note_row,
        )
        .map_err(|e| format!("repo_note `{note_id}` not found: {e}"))?
    };

    for cite in &mut note.citations {
        if let Some(h) = compute_anchor_hash(root, &cite.path, cite.start_line, cite.end_line) {
            cite.hash = h;
        } else {
            return Err(format!(
                "cannot verify `{note_id}`: cited file `{}` does not exist on disk",
                cite.path
            ));
        }
    }
    note.stale = false;
    note.updated_at = chrono::Utc::now().to_rfc3339();
    let citations_json =
        serde_json::to_string(&note.citations).unwrap_or_else(|_| "[]".to_string());
    {
        let _guard = graph_db_lock();
        let conn = open_graph_db_inner(root)?;
        conn.execute(
            "UPDATE repo_notes SET citations = ?1, stale = 0, updated_at = ?2 WHERE id = ?3",
            params![citations_json, note.updated_at, note.id],
        )
        .map_err(|e| format!("update repo_note: {e}"))?;
    }
    let _ = export_repo_memory_markdown(root);
    Ok(note)
}

/// Delete a repository note by ID and refresh `.agentz/REPO_MEMORY.md`.
pub fn delete_repo_note(root: &Path, note_id: &str) -> Result<bool, String> {
    let deleted = {
        let _guard = graph_db_lock();
        let conn = open_graph_db_inner(root)?;
        conn.execute(
            "DELETE FROM repo_notes WHERE id = ?1",
            params![note_id.trim()],
        )
        .map_err(|e| format!("delete repo_note: {e}"))?
    };
    let _ = export_repo_memory_markdown(root);
    Ok(deleted > 0)
}

/// Export all anchored repository notes to `.agentz/REPO_MEMORY.md` for human review & agent context.
pub fn export_repo_memory_markdown(root: &Path) -> Result<PathBuf, String> {
    let notes = query_repo_notes(root, None, true, 100)?;
    let dir = root.join(".agentz");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create .agentz: {e}"))?;
    let out_path = dir.join("REPO_MEMORY.md");

    let mut md = String::new();
    md.push_str("# Repository Memory (Source-Anchored Notes)\n\n");
    md.push_str(
        "> Maintained automatically by AgentZ (`repo_memory` tool & post-turn reflection).\n",
    );
    md.push_str("> Each note is anchored to source file hashes (`path:start-end`). When cited code changes, the note is automatically marked **STALE** until re-verified.\n\n");

    if notes.is_empty() {
        md.push_str("_No repository memory notes recorded yet. Use the `repo_memory` tool (`action: \"record\"`) to capture module invariants, pitfalls, or cross-boundary contracts._\n");
        std::fs::write(&out_path, md).map_err(|e| format!("write REPO_MEMORY.md: {e}"))?;
        return Ok(out_path);
    }

    let active_count = notes.iter().filter(|n| !n.stale).count();
    let stale_count = notes.len().saturating_sub(active_count);
    md.push_str(&format!(
        "- **Active notes:** {active_count} | **Stale (needs verification):** {stale_count}\n\n"
    ));

    let mut by_scope: BTreeMap<String, Vec<&RepoNote>> = BTreeMap::new();
    for note in &notes {
        by_scope
            .entry(note.scope_target.clone())
            .or_default()
            .push(note);
    }

    for (scope, list) in by_scope {
        md.push_str(&format!("## `{scope}`\n\n"));
        for n in list {
            let badge = if n.stale {
                "⚠️ **STALE (source modified — verify before relying)**"
            } else {
                "✓ **ACTIVE**"
            };
            md.push_str(&format!(
                "### [{}] {} (`{}`)\n\n",
                n.kind.to_uppercase(),
                n.title,
                n.id
            ));
            md.push_str(&format!("- **Status:** {badge} _(source: `{}`)_\n", n.source));
            if !n.citations.is_empty() {
                let cites = n
                    .citations
                    .iter()
                    .map(|c| {
                        let tag = format_citation_tag(c);
                        match &c.symbol {
                            Some(sym) if !sym.is_empty() => format!("{tag} (`{sym}`)"),
                            _ => tag,
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                md.push_str(&format!("- **Anchors:** {cites}\n"));
            }
            md.push_str(&format!("\n{}\n\n", n.body));
        }
    }

    std::fs::write(&out_path, md).map_err(|e| format!("write REPO_MEMORY.md: {e}"))?;
    Ok(out_path)
}

#[derive(Debug, Serialize)]
pub struct GraphDbStatus {
    pub nodes: usize,
    pub edges: usize,
    pub generated_at: Option<String>,
}

pub fn graph_db_status(root: &Path) -> Result<Option<GraphDbStatus>, String> {
    if !graph_db_path(root).is_file() {
        return Ok(None);
    }
    let _guard = graph_db_lock();
    let conn = open_graph_db_inner(root)?;
    let nodes: usize = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .unwrap_or(0);
    let edges: usize = conn
        .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))
        .unwrap_or(0);
    let generated_at: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'generated_at'",
            [],
            |r| r.get(0),
        )
        .ok();
    Ok(Some(GraphDbStatus {
        nodes,
        edges,
        generated_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::graph::{GraphEdge, GraphNode, GraphStats, ModuleStat};

    fn sample_doc() -> GraphDoc {
        GraphDoc {
            version: "1.0".into(),
            generated_at: "2026-01-01T00:00:00Z".into(),
            project: "demo".into(),
            nodes: vec![
                GraphNode {
                    id: "file:src/a.rs".into(),
                    kind: "file".into(),
                    path: Some("src/a.rs".into()),
                    name: "a.rs".into(),
                    layer: "service".into(),
                    summary: String::new(),
                },
                GraphNode {
                    id: "file:src/b.rs".into(),
                    kind: "file".into(),
                    path: Some("src/b.rs".into()),
                    name: "b.rs".into(),
                    layer: "api".into(),
                    summary: String::new(),
                },
            ],
            edges: vec![GraphEdge {
                from: "file:src/b.rs".into(),
                to: "file:src/a.rs".into(),
                kind: "imports".into(),
            }],
            modules: vec![ModuleStat {
                name: "src".into(),
                file_count: 2,
                in_degree: 0,
            }],
            stats: GraphStats {
                files: 2,
                nodes: 2,
                edges: 1,
            },
        }
    }

    #[test]
    fn sync_and_explore() {
        let dir = std::env::temp_dir().join(format!("agentz-graph-db-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.rs"), "pub fn hub() {}\n").unwrap();
        std::fs::write(dir.join("src/b.rs"), "use super::a;\n").unwrap();
        std::fs::create_dir_all(dir.join(".agentz")).unwrap();
        let doc = sample_doc();
        let json = serde_json::to_string_pretty(&doc).unwrap();
        std::fs::write(dir.join(".agentz/graph.json"), json).unwrap();

        sync_from_graph_doc(&dir, &doc).unwrap();
        let out = explore_graph(&dir, "a.rs", 5).unwrap();
        assert!(out.contains("**Exploration:"));
        assert!(out.contains("src/a.rs"));
        assert!(out.contains("Blast radius"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn content_fallback_finds_symbol_in_file() {
        let dir = std::env::temp_dir().join(format!("agentz-graph-grep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src-tauri/src/commands")).unwrap();
        std::fs::write(
            dir.join("src-tauri/src/commands/chat_turn.rs"),
            "fn graph_context_block() {}\n",
        )
        .unwrap();
        let doc = GraphDoc {
            version: "1.0".into(),
            generated_at: "2026-01-01T00:00:00Z".into(),
            project: "demo".into(),
            nodes: vec![GraphNode {
                id: "file:src-tauri/src/commands/chat_turn.rs".into(),
                kind: "file".into(),
                path: Some("src-tauri/src/commands/chat_turn.rs".into()),
                name: "chat_turn.rs".into(),
                layer: "service".into(),
                summary: String::new(),
            }],
            edges: vec![],
            modules: vec![],
            stats: GraphStats {
                files: 1,
                nodes: 1,
                edges: 0,
            },
        };
        sync_from_graph_doc(&dir, &doc).unwrap();
        let out = explore_graph(&dir, "graph_context_block", 5).unwrap();
        assert!(out.contains("chat_turn.rs"));
        assert!(out.contains("graph_context_block"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn camel_case_ts_ast_calls_and_anchored_repo_notes_staleness() {
        let dir =
            std::env::temp_dir().join(format!("agentz-ast-memory-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src/workspaces/codez")).unwrap();
        std::fs::write(
            dir.join("src/workspaces/codez/mermaidSafe.ts"),
            "// Mermaid SVG sanitizer\nexport function sanitizeMermaidSvg(raw: string): string {\n  return raw.trim();\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/workspaces/codez/Markdown.tsx"),
            "import { sanitizeMermaidSvg } from './mermaidSafe';\nexport function renderDiagram(svg: string) {\n  return sanitizeMermaidSvg(svg);\n}\n",
        )
        .unwrap();

        let _doc = crate::commands::graph::generate(&dir).unwrap();
        let out = explore_graph(&dir, "sanitizeMermaidSvg", 8).unwrap();
        assert!(
            out.contains("sanitizeMermaidSvg"),
            "explore_graph should find camelCase TS function"
        );
        assert!(
            out.contains("Markdown.tsx"),
            "explore_graph should surface caller file Markdown.tsx: {out}"
        );

        // Record an anchored note bound to lines 2..=4 of mermaidSafe.ts
        let note = record_repo_note(
            &dir,
            RepoNoteInput {
                scope_kind: "module".into(),
                scope_target: "src/workspaces/codez".into(),
                kind: "invariant".into(),
                title: "Mermaid SVG must be sanitized before innerHTML".into(),
                body: "Always pass Mermaid SVG output through sanitizeMermaidSvg.".into(),
                citations: vec![NoteCitation {
                    path: "src/workspaces/codez/mermaidSafe.ts".into(),
                    start_line: Some(2),
                    end_line: Some(4),
                    symbol: Some("sanitizeMermaidSvg".into()),
                    hash: String::new(),
                }],
                source: "agent_tool".into(),
            },
        )
        .unwrap();
        assert!(!note.stale);
        assert!(!note.citations[0].hash.is_empty());
        assert!(dir.join(".agentz/REPO_MEMORY.md").is_file());

        // Modify the cited lines in mermaidSafe.ts and trigger incremental file sync
        std::fs::write(
            dir.join("src/workspaces/codez/mermaidSafe.ts"),
            "// Mermaid SVG sanitizer\nexport function sanitizeMermaidSvg(raw: string): string {\n  return raw.replace(/<script/gi, '');\n}\n",
        )
        .unwrap();
        sync_file_in_graph_db(&dir, "src/workspaces/codez/mermaidSafe.ts").unwrap();

        let notes_after = query_repo_notes(&dir, Some("Mermaid"), true, 10).unwrap();
        assert_eq!(notes_after.len(), 1);
        assert!(
            notes_after[0].stale,
            "note should automatically become stale when cited lines change"
        );
        let mem_md = std::fs::read_to_string(dir.join(".agentz/REPO_MEMORY.md")).unwrap();
        assert!(mem_md.contains("STALE"));

        // Verify the note to re-anchor it to the new source hash
        let verified = verify_repo_note(&dir, &note.id).unwrap();
        assert!(!verified.stale);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
