//! SQLite knowledge graph for `graph_explore` (Phase 0).
//!
//! Phase 0: import file/import edges from `graph.json`, FTS on paths/names,
//! explore returns numbered source + blast radius. Phase 1 adds tree-sitter symbols.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::commands::graph::GraphDoc;

const GRAPH_DB_VERSION: &str = "1.0";
const MAX_SNIPPET_LINES: usize = 120;
const MAX_BLAST_ITEMS: usize = 8;

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
            end_line INTEGER
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
        );",
    )
    .map_err(|e| format!("graph.db schema: {e}"))?;
    Ok(conn)
}

fn sync_from_graph_doc_conn(conn: &Connection, doc: &GraphDoc) -> Result<(), String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("tx: {e}"))?;
    tx.execute("DELETE FROM edges", []).ok();
    tx.execute("DELETE FROM nodes", []).ok();
    tx.execute("DELETE FROM nodes_fts", []).ok();
    tx.execute("DELETE FROM meta", []).ok();
    tx.execute("DELETE FROM pending_sync", []).ok();

    for n in &doc.nodes {
        tx.execute(
            "INSERT INTO nodes (id, kind, path, name, layer, summary, start_line, end_line)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL)",
            params![n.id, n.kind, n.path, n.name, n.layer, n.summary],
        )
        .map_err(|e| format!("insert node {}: {e}", n.id))?;
    }
    for e in &doc.edges {
        tx.execute(
            "INSERT OR IGNORE INTO edges (from_id, to_id, kind, weight) VALUES (?1, ?2, ?3, 1)",
            params![e.from, e.to, e.kind],
        )
        .map_err(|e| format!("insert edge: {e}"))?;
    }
    tx.execute(
        "INSERT INTO meta (key, value) VALUES ('version', ?1), ('generated_at', ?2)",
        params![GRAPH_DB_VERSION, doc.generated_at],
    )
    .map_err(|e| format!("meta: {e}"))?;
    tx.execute(
        "INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')",
        [],
    )
    .map_err(|e| format!("fts rebuild: {e}"))?;
    tx.commit().map_err(|e| format!("commit: {e}"))?;
    Ok(())
}

/// Replace graph.db contents from a GraphDoc (Phase 0 bridge from graph.json).
pub fn sync_from_graph_doc(root: &Path, doc: &GraphDoc) -> Result<(), String> {
    let _guard = graph_db_lock();
    let conn = open_graph_db_inner(root)?;
    sync_from_graph_doc_conn(&conn, doc)
}

fn ensure_synced(root: &Path) -> Result<Connection, String> {
    let conn = open_graph_db_inner(root)?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .unwrap_or(0);
    if count > 0 {
        return Ok(conn);
    }
    let doc = crate::commands::graph::load_graph(root).ok_or(
        "graph index empty — run Wiki → Rebuild graph first".to_string(),
    )?;
    sync_from_graph_doc_conn(&conn, &doc)?;
    Ok(conn)
}

#[derive(Debug, Clone)]
struct NodeRow {
    id: String,
    kind: String,
    path: Option<String>,
    name: String,
    layer: Option<String>,
}

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
        .map(|w| format!("\"{w}\""))
        .collect::<Vec<_>>()
        .join(" OR ");

    let mut rows: Vec<NodeRow> = Vec::new();
    if !fts_query.is_empty() {
        let mut stmt = conn
            .prepare(
                "SELECT n.id, n.kind, n.path, n.name, n.layer
                 FROM nodes_fts f
                 JOIN nodes n ON n.rowid = f.rowid
                 WHERE nodes_fts MATCH ?1
                   AND (n.path IS NULL OR n.path NOT LIKE 'bundled/preinstall/%')
                 ORDER BY bm25(nodes_fts)
                 LIMIT ?2",
            )
            .map_err(|e| format!("fts prepare: {e}"))?;
        rows.extend(
            stmt.query_map(params![fts_query, limit as i64], map_node_row)
                .map_err(|e| format!("fts query: {e}"))?
                .filter_map(|r| r.ok()),
        );
    }

    if rows.len() < limit {
        let like = format!("%{}%", q.to_lowercase());
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, path, name, layer FROM nodes
                 WHERE (lower(name) LIKE ?1 OR lower(COALESCE(path,'')) LIKE ?1)
                   AND (path IS NULL OR path NOT LIKE 'bundled/preinstall/%')
                 LIMIT ?2",
            )
            .map_err(|e| format!("like prepare: {e}"))?;
        for row in stmt
            .query_map(params![like, limit as i64], map_node_row)
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
        .prepare(
            "SELECT id, kind, path, name, layer FROM nodes
             WHERE kind = 'file' AND path IS NOT NULL
               AND path NOT LIKE 'bundled/preinstall/%'",
        )
        .map_err(|e| format!("list files prepare: {e}"))?;
    let rows: Vec<NodeRow> = stmt
        .query_map([], map_node_row)
        .map_err(|e| format!("list files query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Phase 0 fallback: grep indexed source when FTS/path miss a symbol name.
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
    let symbol_query = q.contains('_');

    let mut scored: Vec<(i32, NodeRow)> = Vec::new();
    for node in files {
        let Some(rel) = node.path.as_ref() else {
            continue;
        };
        if symbol_query && !rel.ends_with(".rs") && !rel.ends_with(".ts") && !rel.ends_with(".tsx") {
            continue;
        }
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
            let trimmed = line.trim();
            if trimmed.starts_with("fn ") && trimmed.contains(&q) {
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
    if sym.len() < 3 || !sym.contains('_') {
        return Vec::new();
    }
    let call_needle = format!("{sym}(");
    let files = list_indexed_files(conn).unwrap_or_default();
    let mut lines = Vec::new();

    for node in files {
        let Some(rel) = node.path.as_ref() else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(root.join(rel)) else {
            continue;
        };
        for (i, line) in content.lines().enumerate() {
            if !line.contains(&call_needle) {
                continue;
            }
            if line.contains(&format!("\"{call_needle}")) || line.contains(&format!("'{call_needle}"))
            {
                continue;
            }
            if line.contains("std::fs::write") || line.contains("assert!(out.contains") {
                continue;
            }
            if line.trim_start().starts_with("fn ") && line.contains(sym) {
                continue;
            }
            let loc = format!("{rel}:{}", i + 1);
            let role = if def_paths.contains(rel) {
                "caller"
            } else {
                "reference"
            };
            lines.push(format!("- `{sym}` ({role}) at `{loc}`"));
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
    })
}

fn list_top_files(conn: &Connection, limit: usize) -> Result<Vec<NodeRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, path, name, layer FROM nodes
             WHERE kind = 'file'
             ORDER BY path
             LIMIT ?1",
        )
        .map_err(|e| format!("list files: {e}"))?;
    let rows: Vec<NodeRow> = stmt
        .query_map(params![limit as i64], map_node_row)
        .map_err(|e| format!("list query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

fn read_numbered_file(root: &Path, rel: &str, max_lines: usize) -> Option<String> {
    let abs = root.join(rel);
    let content = std::fs::read_to_string(abs).ok()?;
    let mut out = String::new();
    for (i, line) in content.lines().enumerate().take(max_lines) {
        out.push_str(&format!("{}\t{line}\n", i + 1));
    }
    if content.lines().count() > max_lines {
        out.push_str("\n… [truncated]\n");
    }
    Some(out)
}

fn blast_radius(conn: &Connection, node_id: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut stmt = match conn.prepare(
        "SELECT e.from_id, n.path, n.name
         FROM edges e
         JOIN nodes n ON n.id = e.from_id
         WHERE e.to_id = ?1 AND e.kind IN ('imports', 'calls', 'depends')
         LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(_) => return lines,
    };
    let Ok(rows) = stmt.query_map(params![node_id, MAX_BLAST_ITEMS as i64], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?))
    }) else {
        return lines;
    };
    for row in rows.flatten() {
        let (from_id, path, name) = row;
        let loc = path.unwrap_or_else(|| from_id.clone());
        lines.push(format!("- `{name}` ({loc}) imports/calls this"));
    }
    lines
}

fn one_hop_edges(conn: &Connection, seed_ids: &HashSet<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut stmt = match conn.prepare(
        "SELECT from_id, to_id, kind FROM edges
         WHERE kind IN ('imports', 'depends', 'calls')
         LIMIT 40",
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

/// Explore the codebase graph (CodeGraph-style markdown for agents).
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

    let file_paths: BTreeSet<String> = nodes
        .iter()
        .filter_map(|n| n.path.clone())
        .collect();
    let mut files_ordered: Vec<String> = Vec::new();
    for n in &nodes {
        if let Some(p) = &n.path {
            if !files_ordered.iter().any(|f| f == p) {
                files_ordered.push(p.clone());
            }
        }
    }
    let seed_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

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

    out.push_str("**Blast radius — what depends on these (update/verify before editing)**\n\n");
    let mut any_blast = false;
    for n in &nodes {
        for line in blast_radius(&conn, &n.id) {
            any_blast = true;
            out.push_str(&line);
            out.push('\n');
        }
    }
    if !any_blast {
        out.push_str("_No indexed importers in 1-hop (Phase 0: import edges only)._\n");
    }
    out.push('\n');

    let caller_lines = grep_symbol_callers(root, &conn, query.trim(), &file_paths);
    if !caller_lines.is_empty() {
        out.push_str("**Call / reference sites (content grep, Phase 0)**\n\n");
        for line in caller_lines {
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    let flow = one_hop_edges(&conn, &seed_ids);
    if !flow.is_empty() {
        out.push_str("**Import / dependency flow (1-hop)**\n\n");
        for line in flow {
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str("**Source Code**\n\n");
    out.push_str(
        "> Verbatim on-disk source with line numbers — treat as already Read.\n\n",
    );

    for rel in files_ordered.iter().take(8) {
        let node = nodes.iter().find(|n| n.path.as_deref() == Some(rel.as_str()));
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

/// Mark a file as pending re-index (Phase 1 staleness).
pub fn mark_pending_sync(root: &Path, rel: &str) -> Result<(), String> {
    let conn = open_graph_db(root)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO pending_sync (path, updated_at) VALUES (?1, ?2)
         ON CONFLICT(path) DO UPDATE SET updated_at = excluded.updated_at",
        params![rel, now],
    )
    .map_err(|e| format!("pending_sync: {e}"))?;
    Ok(())
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
}
