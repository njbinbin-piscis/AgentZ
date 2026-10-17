//! Persisted task notifications. Posting is explicit and never polls or sends externally.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    MeaningfulChange,
    Completed,
    Failed,
    ActionRequired,
}

impl NoticeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::MeaningfulChange => "meaningful_change",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::ActionRequired => "action_required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    pub event_key: String,
    pub task_id: String,
    pub kind: NoticeKind,
    pub summary: String,
    pub created_ms: i64,
    pub read_ms: Option<i64>,
}

pub struct InboxStore {
    conn: Connection,
}

impl InboxStore {
    pub fn open(project: &Path) -> Result<Self, String> {
        let dir = project.join(".agentz");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let conn = Connection::open(dir.join("inbox.db")).map_err(|e| e.to_string())?;
        conn.busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS notices (
                event_key TEXT PRIMARY KEY NOT NULL, task_id TEXT NOT NULL,
                kind TEXT NOT NULL, summary TEXT NOT NULL, created_ms INTEGER NOT NULL,
                read_ms INTEGER);
             CREATE INDEX IF NOT EXISTS notices_recent ON notices(created_ms DESC,event_key);
             CREATE TABLE IF NOT EXISTS notice_source_states (
                 source_key TEXT PRIMARY KEY NOT NULL, task_id TEXT NOT NULL,
                 fingerprint TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS inbox_reconcile_cursors (
                 source_key TEXT PRIMARY KEY NOT NULL, cursor_event_key TEXT NOT NULL);",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }

    /// Post only when a source's semantic state changes. The caller supplies a
    /// stable source identity and a digest of the user-visible state, not a poll
    /// sequence. The state transition and notice are committed together.
    pub fn post_change(
        &mut self,
        source_key: &str,
        fingerprint: &str,
        notice: &Notice,
    ) -> Result<bool, String> {
        if notice.kind != NoticeKind::MeaningfulChange
            || !valid_identity(source_key, 256)
            || !valid_identity(fingerprint, 128)
        {
            return Err("invalid meaningful-change source".into());
        }
        validate_notice(notice)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let prior: Option<(String, String)> = tx
            .query_row(
                "SELECT task_id,fingerprint FROM notice_source_states WHERE source_key=?1",
                [source_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if prior
            .as_ref()
            .is_some_and(|(task, _)| task != &notice.task_id)
        {
            return Err("meaningful-change source task conflict".into());
        }
        if prior
            .as_ref()
            .is_some_and(|(_, state)| state == fingerprint)
        {
            return Ok(false);
        }
        // A new transition must have a new event identity. Reusing an older
        // notice key must not move the semantic state without a new notice.
        let existing: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM notices WHERE event_key=?1",
                [&notice.event_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if existing.is_some() {
            return Err("meaningful-change event key already used".into());
        }
        tx.execute(
            "INSERT INTO notices(event_key,task_id,kind,summary,created_ms,read_ms) VALUES(?1,?2,?3,?4,?5,NULL)",
            params![notice.event_key, notice.task_id, notice.kind.as_str(), notice.summary, notice.created_ms],
        ).map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO notice_source_states(source_key,task_id,fingerprint) VALUES(?1,?2,?3)
             ON CONFLICT(source_key) DO UPDATE SET fingerprint=excluded.fingerprint",
            params![source_key, notice.task_id, fingerprint],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(true)
    }

    /// A source must supply a durable event key, e.g. run ID plus event sequence.
    /// Reposting the same event preserves read state; a changed payload is rejected.
    pub fn post(&mut self, notice: &Notice) -> Result<bool, String> {
        validate_notice(notice)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let existing: Option<(String, String, String, i64)> = tx
            .query_row(
                "SELECT task_id,kind,summary,created_ms FROM notices WHERE event_key=?1",
                [&notice.event_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((task_id, kind, summary, created_ms)) = existing {
            if task_id != notice.task_id
                || kind != notice.kind.as_str()
                || summary != notice.summary
                || created_ms != notice.created_ms
            {
                return Err("notification event key payload conflict".into());
            }
            return Ok(false);
        }
        tx.execute(
            "INSERT INTO notices(event_key,task_id,kind,summary,created_ms,read_ms) VALUES(?1,?2,?3,?4,?5,NULL)",
            params![notice.event_key, notice.task_id, notice.kind.as_str(), notice.summary, notice.created_ms],
        ).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub fn mark_read(&mut self, event_key: &str, read_ms: i64) -> Result<bool, String> {
        if read_ms < 0 {
            return Err("invalid read timestamp".into());
        }
        let changed = self
            .conn
            .execute(
                "UPDATE notices SET read_ms=?2 WHERE event_key=?1 AND read_ms IS NULL",
                params![event_key, read_ms],
            )
            .map_err(|e| e.to_string())?;
        Ok(changed == 1)
    }

    /// Rotate through unread approval notices in bounded pages. The durable
    /// cursor prevents live notices in the first page from starving older keys.
    pub fn unread_approval_candidates(
        &mut self,
        limit: usize,
    ) -> Result<Vec<(String, String)>, String> {
        let limit = limit.clamp(1, 1000) as i64;
        let mut cursor: String = self.conn.query_row(
            "SELECT cursor_event_key FROM inbox_reconcile_cursors WHERE source_key='approval_v1'",
            [], |row| row.get(0),
        ).optional().map_err(|e| e.to_string())?.unwrap_or_default();
        let mut page = self.approval_candidates_after(&cursor, limit)?;
        if page.is_empty() && !cursor.is_empty() {
            cursor.clear();
            page = self.approval_candidates_after(&cursor, limit)?;
        }
        let next_cursor = page.last().map(|(key, _)| key.as_str()).unwrap_or("");
        self.conn.execute(
            "INSERT INTO inbox_reconcile_cursors(source_key,cursor_event_key) VALUES('approval_v1',?1)
             ON CONFLICT(source_key) DO UPDATE SET cursor_event_key=excluded.cursor_event_key",
            [next_cursor],
        ).map_err(|e| e.to_string())?;
        Ok(page)
    }

    fn approval_candidates_after(
        &self,
        cursor: &str,
        limit: i64,
    ) -> Result<Vec<(String, String)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT event_key,task_id FROM notices
             WHERE read_ms IS NULL AND kind='action_required' AND summary='Approval required'
             AND event_key LIKE 'approval:approval:%' AND event_key > ?1
             ORDER BY event_key LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![cursor, limit], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.map(|row| row.map_err(|e| e.to_string())).collect()
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<Notice>, String> {
        let limit = limit.clamp(1, 100) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT event_key,task_id,kind,summary,created_ms,read_ms FROM notices ORDER BY created_ms DESC,event_key LIMIT ?1",
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([limit], |row| {
                let kind: String = row.get(2)?;
                let kind = match kind.as_str() {
                    "meaningful_change" => NoticeKind::MeaningfulChange,
                    "completed" => NoticeKind::Completed,
                    "failed" => NoticeKind::Failed,
                    "action_required" => NoticeKind::ActionRequired,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                Ok(Notice {
                    event_key: row.get(0)?,
                    task_id: row.get(1)?,
                    kind,
                    summary: row.get(3)?,
                    created_ms: row.get(4)?,
                    read_ms: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn unread_count(&self) -> Result<u64, String> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM notices WHERE read_ms IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        u64::try_from(count).map_err(|e| e.to_string())
    }
}

fn valid_identity(value: &str, max_len: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_len && !value.chars().any(char::is_control)
}

fn validate_notice(notice: &Notice) -> Result<(), String> {
    if !valid_identity(&notice.event_key, 512)
        || !valid_identity(&notice.task_id, 256)
        || notice.summary.trim().is_empty()
        || notice.summary.chars().count() > 240
        || notice.created_ms < 0
        || notice.read_ms.is_some()
    {
        return Err("invalid notification".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_event_preserves_read_state_across_reopen() {
        let path = std::env::temp_dir().join(format!("agentz-inbox-{}", uuid::Uuid::new_v4()));
        let notice = Notice {
            event_key: "run-1:7".into(),
            task_id: "task-1".into(),
            kind: NoticeKind::ActionRequired,
            summary: "Approval needed".into(),
            created_ms: 100,
            read_ms: None,
        };
        let mut store = InboxStore::open(&path).unwrap();
        assert!(store.post(&notice).unwrap());
        assert_eq!(store.unread_count().unwrap(), 1);
        assert!(store.mark_read(&notice.event_key, 200).unwrap());
        assert_eq!(store.unread_count().unwrap(), 0);
        drop(store);
        let mut reopened = InboxStore::open(&path).unwrap();
        assert!(!reopened.post(&notice).unwrap());
        assert!(!reopened.mark_read(&notice.event_key, 300).unwrap());
        let row = &reopened.recent(10).unwrap()[0];
        assert_eq!(row.task_id, "task-1");
        assert_eq!(row.read_ms, Some(200));
        let mut changed = notice.clone();
        changed.summary = "Different".into();
        assert!(reopened.post(&changed).is_err());
        drop(reopened);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn two_connections_cannot_create_duplicate_notification() {
        let path = std::env::temp_dir().join(format!("agentz-inbox-{}", uuid::Uuid::new_v4()));
        let notice = Notice {
            event_key: "run-2:1".into(),
            task_id: "task-2".into(),
            kind: NoticeKind::Completed,
            summary: "Completed".into(),
            created_ms: 300,
            read_ms: None,
        };
        let mut first = InboxStore::open(&path).unwrap();
        let mut second = InboxStore::open(&path).unwrap();
        assert!(first.post(&notice).unwrap());
        assert!(!second.post(&notice).unwrap());
        assert_eq!(second.unread_count().unwrap(), 1);
        drop(first);
        drop(second);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn unchanged_semantic_state_is_quiet_across_poll_and_reopen() {
        let path = std::env::temp_dir().join(format!("agentz-inbox-{}", uuid::Uuid::new_v4()));
        let mut notice = Notice {
            event_key: "source:1".into(),
            task_id: "task-3".into(),
            kind: NoticeKind::MeaningfulChange,
            summary: "Needs review".into(),
            created_ms: 100,
            read_ms: None,
        };
        let mut first = InboxStore::open(&path).unwrap();
        assert!(first
            .post_change("review:task-3", "digest-a", &notice)
            .unwrap());
        notice.event_key = "source:2".into();
        notice.created_ms = 200;
        assert!(!first
            .post_change("review:task-3", "digest-a", &notice)
            .unwrap());
        drop(first);
        let mut reopened = InboxStore::open(&path).unwrap();
        assert!(!reopened
            .post_change("review:task-3", "digest-a", &notice)
            .unwrap());
        let mut wrong_task = notice.clone();
        wrong_task.task_id = "task-4".into();
        assert!(reopened
            .post_change("review:task-3", "digest-a", &wrong_task)
            .is_err());
        assert!(reopened
            .post_change("review:task-3", "digest-b", &notice)
            .unwrap());
        assert_eq!(reopened.recent(10).unwrap().len(), 2);
        notice.event_key = "source:1".into();
        assert!(reopened
            .post_change("review:task-3", "digest-c", &notice)
            .is_err());
        notice.event_key = "source:3".into();
        assert!(reopened
            .post_change("review:task-3", "digest-c", &notice)
            .unwrap());
        assert_eq!(reopened.recent(10).unwrap().len(), 3);
        drop(reopened);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn approval_reconciliation_cursor_reaches_beyond_first_thousand_across_reopen() {
        let path = std::env::temp_dir().join(format!("agentz-inbox-{}", uuid::Uuid::new_v4()));
        let mut store = InboxStore::open(&path).unwrap();
        for index in 0..1001 {
            store
                .post(&Notice {
                    event_key: format!("approval:approval:{index:04}"),
                    task_id: "task".into(),
                    kind: NoticeKind::ActionRequired,
                    summary: "Approval required".into(),
                    created_ms: index,
                    read_ms: None,
                })
                .unwrap();
        }
        let first = store.unread_approval_candidates(1000).unwrap();
        assert_eq!(first.len(), 1000);
        assert_eq!(first[0].0, "approval:approval:0000");
        drop(store);
        let mut reopened = InboxStore::open(&path).unwrap();
        let second = reopened.unread_approval_candidates(1000).unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].0, "approval:approval:1000");
        reopened.mark_read(&second[0].0, 2000).unwrap();
        let wrapped = reopened.unread_approval_candidates(1000).unwrap();
        assert_eq!(wrapped.len(), 1000);
        assert_eq!(wrapped[0].0, "approval:approval:0000");
        drop(reopened);
        let _ = std::fs::remove_dir_all(path);
    }
}
