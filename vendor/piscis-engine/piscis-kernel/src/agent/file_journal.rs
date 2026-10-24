//! Shared file-edit journal — per-turn pre-edit snapshots for Undo / replay.
//!
//! This is a host-agnostic [`AgentHooks`] implementation: **before** any
//! `file_write` / `file_edit` runs, the prior file content is captured into a
//! dedicated SQLite database; **after**, the snapshot is marked applied (or
//! dropped on failure). A turn's snapshots can then be reverted ("Undo All").
//!
//! Message-level checkpoints alone cannot recover code — only a content
//! journal can. Both desktop hosts (AgentZ, openpiscis) share this exact
//! implementation and add only a thin command/UI layer, so the capability
//! never forks between products.
//!
//! Storage is a standalone DB (path chosen by the host, e.g.
//! `{project}/.agentz/journal.db`) kept separate from the chat DB so journaling
//! never contends with the kernel's own locking inside the agent loop.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::agent::hooks::{AgentHooks, HookDecision, ToolHookEvent};
use crate::agent::tool::ToolResult;

/// Tools whose effects are snapshotted. Deletions via `shell` are out of scope.
const SNAPSHOT_TOOLS: &[&str] = &["file_write", "file_edit"];

/// One journaled file change, surfaced to a host's review UI.
#[derive(Debug, Clone, Serialize)]
pub struct JournalChange {
    pub id: i64,
    pub rel_path: String,
    pub tool_name: String,
    /// Whether the file existed before the edit (false => agent created it).
    pub existed: bool,
    /// Whether the edit actually applied (tool returned success).
    pub applied: bool,
    /// Net state vs. the pre-edit baseline, reconciled against disk:
    /// `added` | `modified` | `deleted`.
    pub status: String,
    pub turn_id: String,
}

/// Net status of a path relative to its baseline; `None` means no net change
/// (e.g. a temp file the agent created and later removed).
fn net_status(existed: bool, before: Option<&[u8]>, abs: &Path) -> Option<&'static str> {
    match (existed, abs.is_file()) {
        (false, false) => None,
        (false, true) => Some("added"),
        (true, false) => Some("deleted"),
        (true, true) => match (before, std::fs::read(abs)) {
            (Some(b), Ok(now)) if b == now.as_slice() => None,
            _ => Some("modified"),
        },
    }
}

/// Before/after file contents for inline diff cards in the chat stream.
#[derive(Debug, Clone, Serialize)]
pub struct JournalFileDiff {
    pub id: i64,
    pub rel_path: String,
    pub existed: bool,
    pub before: Option<String>,
    pub after: String,
}

/// Workspace-scoped file journal. Cheap to open; backed by a SQLite file.
pub struct FileJournal {
    conn: Mutex<Connection>,
    workspace_root: PathBuf,
    /// Turn id for the in-flight turn (set by the host before `agent.run`).
    current_turn: Mutex<Option<String>>,
}

impl FileJournal {
    /// Open (and migrate) the journal DB. `workspace_root` is the directory
    /// tool paths are resolved against; `db_path` is where snapshots live
    /// (its parent directory is created if missing).
    pub fn open(workspace_root: impl AsRef<Path>, db_path: impl AsRef<Path>) -> Result<Self> {
        let workspace_root = workspace_root.as_ref().to_path_buf();
        let db_path = db_path.as_ref();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create journal dir {}", parent.display()))?;
        }
        let conn = Connection::open(db_path)
            .with_context(|| format!("open journal db {}", db_path.display()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS file_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                turn_id TEXT NOT NULL,
                tool_use_id TEXT,
                tool_name TEXT NOT NULL,
                rel_path TEXT NOT NULL,
                existed INTEGER NOT NULL,
                before_content BLOB,
                applied INTEGER NOT NULL DEFAULT 0,
                undone INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_snap_session_turn
                ON file_snapshots(session_id, turn_id);",
        )
        .context("journal migrate failed")?;
        // Older DBs predate Keep; the column add is a no-op error when present.
        let _ = conn.execute(
            "ALTER TABLE file_snapshots ADD COLUMN kept INTEGER NOT NULL DEFAULT 0",
            [],
        );
        Ok(Self {
            conn: Mutex::new(conn),
            workspace_root,
            current_turn: Mutex::new(None),
        })
    }

    /// Begin a new turn; returns the generated turn id.
    pub fn begin_turn(&self, _session_id: &str) -> String {
        let turn_id = turn_id();
        *self.current_turn.lock().unwrap() = Some(turn_id.clone());
        turn_id
    }

    /// The turn id currently being journaled, if any.
    pub fn current_turn_id(&self) -> Option<String> {
        self.current_turn.lock().unwrap().clone()
    }

    /// Resolve a tool-supplied path (absolute or relative) to a workspace-
    /// relative path string using forward slashes.
    fn rel_path(&self, raw: &str) -> String {
        let p = Path::new(raw);
        let rel = p
            .strip_prefix(&self.workspace_root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        rel.trim_start_matches('/').to_string()
    }

    fn abs_path(&self, rel: &str) -> PathBuf {
        self.workspace_root.join(rel)
    }

    /// Record the pre-edit state of a file (called from `before_tool`).
    fn snapshot_before(&self, session_id: &str, ev: &ToolHookEvent<'_>) {
        let Some(turn_id) = self.current_turn_id() else {
            return;
        };
        let Some(raw_path) = ev.input.get("path").and_then(|v| v.as_str()) else {
            return;
        };
        let rel = self.rel_path(raw_path);
        let abs = self.abs_path(&rel);
        let (existed, before) = match std::fs::read(&abs) {
            Ok(bytes) => (true, Some(bytes)),
            Err(_) => (false, None),
        };
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO file_snapshots
                (session_id, turn_id, tool_use_id, tool_name, rel_path, existed, before_content, applied)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
            params![
                session_id,
                turn_id,
                ev.tool_use_id,
                ev.tool_name,
                rel,
                existed as i64,
                before,
            ],
        );
    }

    /// Mark the snapshot for a tool call applied/failed (called from `after_tool`).
    fn finalize(&self, ev: &ToolHookEvent<'_>, ok: bool) {
        let conn = self.conn.lock().unwrap();
        if ok {
            let _ = conn.execute(
                "UPDATE file_snapshots SET applied = 1
                 WHERE tool_use_id = ?1 AND applied = 0",
                params![ev.tool_use_id],
            );
        } else {
            // Tool failed: the file was not changed, drop the pending snapshot.
            let _ = conn.execute(
                "DELETE FROM file_snapshots WHERE tool_use_id = ?1 AND applied = 0",
                params![ev.tool_use_id],
            );
        }
    }

    /// The most recent turn id that still has applied, not-yet-undone changes
    /// for a session. Lets event-driven hosts (whose turn handler returns before
    /// the agent finishes) resolve "the last turn" without threading a turn id.
    pub fn latest_turn_with_changes(&self, session_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let id = conn
            .query_row(
                "SELECT turn_id FROM file_snapshots
                 WHERE session_id = ?1 AND applied = 1 AND undone = 0
                 ORDER BY id DESC LIMIT 1",
                params![session_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        Ok(id)
    }

    /// Net per-path changes from the snapshots matching `filter`. The earliest
    /// snapshot per path is the baseline; paths with no net change on disk are
    /// dropped. Newest-touched first.
    fn net_changes(&self, filter: &str, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<JournalChange>> {
        struct Row {
            id: i64,
            rel: String,
            tool: String,
            existed: bool,
            before: Option<Vec<u8>>,
            turn: String,
        }
        let rows: Vec<Row> = {
            let conn = self.conn.lock().unwrap();
            let sql = format!(
                "SELECT id, rel_path, tool_name, existed, before_content, turn_id
                 FROM file_snapshots WHERE applied = 1 AND undone = 0 AND {filter}
                 ORDER BY id ASC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map(args, |r| {
                Ok(Row {
                    id: r.get(0)?,
                    rel: r.get(1)?,
                    tool: r.get(2)?,
                    existed: r.get::<_, i64>(3)? != 0,
                    before: r.get(4)?,
                    turn: r.get(5)?,
                })
            })?;
            mapped.collect::<rusqlite::Result<_>>()?
        };
        let mut order: Vec<String> = Vec::new();
        let mut by_path: std::collections::HashMap<String, (Row, i64, String)> =
            std::collections::HashMap::new();
        for row in rows {
            match by_path.get_mut(&row.rel) {
                Some(entry) => {
                    entry.1 = row.id;
                    entry.2 = row.tool;
                }
                None => {
                    order.push(row.rel.clone());
                    let (last, tool) = (row.id, row.tool.clone());
                    by_path.insert(row.rel.clone(), (row, last, tool));
                }
            }
        }
        let mut out = Vec::new();
        for rel in order {
            let (base, last_id, last_tool) = by_path.remove(&rel).unwrap();
            let Some(status) = net_status(base.existed, base.before.as_deref(), &self.abs_path(&rel))
            else {
                continue;
            };
            out.push(JournalChange {
                id: last_id,
                rel_path: rel,
                tool_name: last_tool,
                existed: base.existed,
                applied: true,
                status: status.to_string(),
                turn_id: base.turn,
            });
        }
        out.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(out)
    }

    /// Net changes for one turn (most recent first, one entry per path).
    pub fn list_changes(&self, session_id: &str, turn_id: &str) -> Result<Vec<JournalChange>> {
        self.net_changes("session_id = ?1 AND turn_id = ?2", &[&session_id, &turn_id])
    }

    /// Changes awaiting review across every turn of a session: applied, not
    /// undone, not kept — reconciled against disk.
    pub fn list_pending(&self, session_id: &str) -> Result<Vec<JournalChange>> {
        self.net_changes("session_id = ?1 AND kept = 0", &[&session_id])
    }

    /// Accept pending changes (`None` = all) so they leave the review list.
    pub fn keep_paths(&self, session_id: &str, paths: Option<&[String]>) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let n = match paths {
            None => conn.execute(
                "UPDATE file_snapshots SET kept = 1
                 WHERE session_id = ?1 AND applied = 1 AND undone = 0 AND kept = 0",
                params![session_id],
            )?,
            Some(list) => {
                let mut n = 0;
                for p in list {
                    n += conn.execute(
                        "UPDATE file_snapshots SET kept = 1
                         WHERE session_id = ?1 AND rel_path = ?2
                           AND applied = 1 AND undone = 0 AND kept = 0",
                        params![session_id, p],
                    )?;
                }
                n
            }
        };
        Ok(n)
    }

    /// Revert pending changes (`None` = all) to their pre-review baseline.
    /// Returns the restored relative paths.
    pub fn undo_paths(&self, session_id: &str, paths: Option<&[String]>) -> Result<Vec<String>> {
        let targets: Vec<(String, bool, Option<Vec<u8>>)> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT rel_path, existed, before_content FROM file_snapshots
                 WHERE session_id = ?1 AND applied = 1 AND undone = 0 AND kept = 0
                 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map(params![session_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)? != 0,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                ))
            })?;
            let mut seen = std::collections::HashSet::new();
            let mut v = Vec::new();
            for row in rows {
                let row = row?;
                let wanted = paths.map_or(true, |l| l.iter().any(|p| p == &row.0));
                if wanted && seen.insert(row.0.clone()) {
                    v.push(row);
                }
            }
            v
        };
        let mut restored = Vec::new();
        for (rel, existed, before) in &targets {
            self.restore(rel, *existed, before.as_deref())?;
            restored.push(rel.clone());
        }
        let conn = self.conn.lock().unwrap();
        for rel in &restored {
            conn.execute(
                "UPDATE file_snapshots SET undone = 1
                 WHERE session_id = ?1 AND rel_path = ?2 AND applied = 1 AND kept = 0",
                params![session_id, rel],
            )?;
        }
        Ok(restored)
    }

    fn restore(&self, rel: &str, existed: bool, before: Option<&[u8]>) -> Result<()> {
        let abs = self.abs_path(rel);
        if existed {
            if let Some(bytes) = before {
                if let Some(parent) = abs.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&abs, bytes).with_context(|| format!("restore {rel} failed"))?;
            }
        } else {
            let _ = std::fs::remove_file(&abs);
        }
        Ok(())
    }

    /// Load before/after text for each applied change in a turn (for inline diffs).
    pub fn get_turn_file_diffs(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> Result<Vec<JournalFileDiff>> {
        let changes = self.list_changes(session_id, turn_id)?;
        let mut out = Vec::with_capacity(changes.len());
        for c in changes {
            let (before, after) = self.read_change_contents(c.id)?;
            out.push(JournalFileDiff {
                id: c.id,
                rel_path: c.rel_path,
                existed: c.existed,
                before,
                after,
            });
        }
        Ok(out)
    }

    fn read_change_contents(&self, change_id: i64) -> Result<(Option<String>, String)> {
        let conn = self.conn.lock().unwrap();
        let (rel, before_blob): (String, Option<Vec<u8>>) = conn.query_row(
            "SELECT rel_path, before_content FROM file_snapshots WHERE id = ?1",
            params![change_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        drop(conn);
        let before = before_blob.map(|b| String::from_utf8_lossy(&b).into_owned());
        let abs = self.abs_path(&rel);
        let after = std::fs::read_to_string(&abs).unwrap_or_default();
        Ok((before, after))
    }

    /// Undo all applied changes in a turn, restoring pre-edit content.
    /// Returns the list of restored relative paths.
    pub fn undo_turn(&self, session_id: &str, turn_id: &str) -> Result<Vec<String>> {
        // Restore each path once from its earliest still-applied snapshot.
        // Selecting the latest snapshot would leave earlier edits in place.
        let snapshots: Vec<(i64, String, bool, Option<Vec<u8>>)> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, rel_path, existed, before_content
                 FROM file_snapshots
                 WHERE session_id = ?1 AND turn_id = ?2 AND applied = 1 AND undone = 0
                 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map(params![session_id, turn_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)? != 0,
                    r.get::<_, Option<Vec<u8>>>(3)?,
                ))
            })?;
            let mut v = Vec::new();
            for row in rows {
                v.push(row?);
            }
            v
        };

        let mut restored = Vec::new();
        let mut restored_paths = std::collections::HashSet::new();
        for (_id, rel, existed, before) in &snapshots {
            // Only act once per path (the earliest eligible snapshot wins).
            if !restored_paths.insert(rel.clone()) {
                continue;
            }
            self.restore(rel, *existed, before.as_deref())?;
            restored.push(rel.clone());
        }

        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "UPDATE file_snapshots SET undone = 1
             WHERE session_id = ?1 AND turn_id = ?2 AND applied = 1",
            params![session_id, turn_id],
        );
        Ok(restored)
    }
}

#[async_trait]
impl AgentHooks for FileJournal {
    async fn before_tool(&self, ev: &ToolHookEvent<'_>) -> HookDecision {
        if SNAPSHOT_TOOLS.contains(&ev.tool_name) {
            self.snapshot_before(ev.session_id, ev);
        }
        HookDecision::Continue
    }

    async fn after_tool(&self, ev: &ToolHookEvent<'_>, result: &ToolResult) {
        if SNAPSHOT_TOOLS.contains(&ev.tool_name) {
            self.finalize(ev, !result.is_error);
        }
    }
}

#[cfg(test)]
mod undo_tests {
    use super::*;
    fn change(journal: &FileJournal, session: &str, id: &str, path: &str, bytes: &[u8], ok: bool) {
        let input = serde_json::json!({"path":path});
        let event = ToolHookEvent {
            session_id: session,
            tool_use_id: id,
            tool_name: "file_write",
            input: &input,
            workspace_root: &journal.workspace_root,
        };
        journal.snapshot_before(session, &event);
        if ok {
            std::fs::write(journal.abs_path(path), bytes).unwrap();
        }
        journal.finalize(&event, ok);
    }
    #[test]
    fn repeated_edits_restore_original_bytes_once_and_do_not_replay() {
        let dir = tempfile::tempdir().unwrap();
        let journal = FileJournal::open(dir.path(), dir.path().join("journal.db")).unwrap();
        std::fs::write(dir.path().join("a.bin"), [0, 255, 13, 10]).unwrap();
        let turn = journal.begin_turn("a");
        change(&journal, "a", "one", "a.bin", b"first", true);
        change(&journal, "a", "two", "a.bin", b"second", true);
        assert_eq!(journal.undo_turn("a", &turn).unwrap(), vec!["a.bin"]);
        assert_eq!(
            std::fs::read(dir.path().join("a.bin")).unwrap(),
            [0, 255, 13, 10]
        );
        assert!(journal.undo_turn("a", &turn).unwrap().is_empty());
    }
    #[test]
    fn previously_undone_snapshot_is_not_selected_for_restore() {
        let dir = tempfile::tempdir().unwrap();
        let journal = FileJournal::open(dir.path(), dir.path().join("journal.db")).unwrap();
        std::fs::write(dir.path().join("a.txt"), b"original").unwrap();
        let turn = journal.begin_turn("a");
        change(&journal, "a", "one", "a.txt", b"first", true);
        change(&journal, "a", "two", "a.txt", b"second", true);
        journal
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE file_snapshots SET undone=1 WHERE tool_use_id='one'",
                [],
            )
            .unwrap();
        journal.undo_turn("a", &turn).unwrap();
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), b"first");
        assert!(journal.undo_turn("a", &turn).unwrap().is_empty());
    }
    #[test]
    fn new_file_then_edit_restores_absence() {
        let dir = tempfile::tempdir().unwrap();
        let journal = FileJournal::open(dir.path(), dir.path().join("journal.db")).unwrap();
        let turn = journal.begin_turn("a");
        change(&journal, "a", "one", "new.txt", b"first", true);
        change(&journal, "a", "two", "new.txt", b"second", true);
        journal.undo_turn("a", &turn).unwrap();
        assert!(!dir.path().join("new.txt").exists());
    }
    #[test]
    fn failed_changes_and_other_sessions_do_not_replace_original() {
        let dir = tempfile::tempdir().unwrap();
        let journal = FileJournal::open(dir.path(), dir.path().join("journal.db")).unwrap();
        std::fs::write(dir.path().join("a.txt"), b"original").unwrap();
        let turn = journal.begin_turn("a");
        change(&journal, "a", "failed", "a.txt", b"never", false);
        change(&journal, "a", "one", "a.txt", b"first", true);
        change(&journal, "b", "other", "b.txt", b"other-session", true);
        change(&journal, "a", "two", "a.txt", b"second", true);
        journal.undo_turn("a", &turn).unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("a.txt")).unwrap(),
            b"original"
        );
        assert_eq!(
            std::fs::read(dir.path().join("b.txt")).unwrap(),
            b"other-session"
        );
    }
    #[test]
    fn pending_reconciles_with_disk() {
        let dir = tempfile::tempdir().unwrap();
        let journal = FileJournal::open(dir.path(), dir.path().join("journal.db")).unwrap();
        std::fs::write(dir.path().join("same.txt"), b"x").unwrap();
        std::fs::write(dir.path().join("gone.txt"), b"keep me").unwrap();
        journal.begin_turn("a");
        change(&journal, "a", "1", "tmp_test.py", b"print(1)", true);
        std::fs::remove_file(dir.path().join("tmp_test.py")).unwrap();
        change(&journal, "a", "2", "same.txt", b"y", true);
        change(&journal, "a", "3", "same.txt", b"x", true);
        change(&journal, "a", "4", "gone.txt", b"edited", true);
        std::fs::remove_file(dir.path().join("gone.txt")).unwrap();
        journal.begin_turn("a");
        change(&journal, "a", "5", "new.txt", b"n", true);
        let pending = journal.list_pending("a").unwrap();
        let got: Vec<(&str, &str)> = pending
            .iter()
            .map(|c| (c.rel_path.as_str(), c.status.as_str()))
            .collect();
        assert_eq!(got, vec![("new.txt", "added"), ("gone.txt", "deleted")]);
    }
    #[test]
    fn keep_and_undo_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let journal = FileJournal::open(dir.path(), dir.path().join("journal.db")).unwrap();
        std::fs::write(dir.path().join("a.txt"), b"orig").unwrap();
        journal.begin_turn("s");
        change(&journal, "s", "1", "a.txt", b"one", true);
        change(&journal, "s", "2", "b.txt", b"new", true);
        journal.keep_paths("s", Some(&["a.txt".to_string()])).unwrap();
        journal.begin_turn("s");
        change(&journal, "s", "3", "a.txt", b"two", true);
        assert_eq!(journal.undo_paths("s", None).unwrap().len(), 2);
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), b"one");
        assert!(!dir.path().join("b.txt").exists());
        assert!(journal.list_pending("s").unwrap().is_empty());
    }
}

/// Lightweight unique turn id (hyphen-free, time + address entropy).
fn turn_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let salt = std::ptr::addr_of!(nanos) as usize;
    format!("turn_{nanos:x}{salt:x}")
}
