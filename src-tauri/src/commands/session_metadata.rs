//! Existing session authorization metadata without kernel initialization or migration.
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Deserialize;
use std::{path::Path, time::Duration};

const MAX_FRAME_BYTES: i64 = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SessionMetadata {
    pub workspace: Option<String>,
    pub workz: bool,
    pub cwd: Option<String>,
}

#[derive(Deserialize)]
struct Frame {
    #[serde(default)]
    agentz_workz: Option<super::session::WorkzSessionMeta>,
}

pub(super) fn read_existing(project: &Path, session_id: &str) -> Result<SessionMetadata, String> {
    if session_id.is_empty()
        || session_id.trim() != session_id
        || session_id.len() > 256
        || session_id.chars().any(char::is_control)
    {
        return Err("existing session identity required".into());
    }
    let project = agentz_task_state::canonical_root(project)?;
    let mut conn = Connection::open_with_flags(
        Path::new(&project).join(".agentz/piscis.db"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("cannot read existing session database: {e}"))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    // No schema-version claim: missing required columns fail at prepare. Do not
    // COALESCE a missing/null source into a privileged session namespace.
    let metadata = tx.query_row(
        "SELECT id,source,workspace_root,pool_session_id,length(CAST(state_frame_json AS BLOB)) FROM sessions WHERE id=?1",
        [session_id],
        |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,Option<String>>(2)?,row.get::<_,Option<String>>(3)?,row.get::<_,Option<i64>>(4)?)),
    ).optional().map_err(|e|format!("cannot read existing session metadata: {e}"))?
        .ok_or("existing session not found")?;
    let (id, source, workspace, pool, frame_bytes) = metadata;
    if id != session_id {
        return Err("existing session identity mismatch".into());
    }
    if frame_bytes.is_some_and(|size| !(0..=MAX_FRAME_BYTES).contains(&size)) {
        return Err("existing session frame exceeds 1 MiB".into());
    }
    // Read the frame strictly even for CodeZ, so malformed SQLite storage types
    // cannot be silently converted to an absent authorization field.
    let raw: Option<String> = tx
        .query_row(
            "SELECT state_frame_json FROM sessions WHERE id=?1",
            [session_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("cannot read existing session frame: {e}"))?;
    if super::session_sources::is_codez_source(&source) {
        return Ok(SessionMetadata {
            workspace: workspace.filter(|s| !s.trim().is_empty()),
            workz: false,
            cwd: None,
        });
    }
    if source != super::session_sources::SOURCE_WORKZ || pool.is_some() {
        return Err("team, pool, remote and background plan sessions are unsupported".into());
    }
    let meta = match raw.filter(|s| !s.trim().is_empty()) {
        None => super::session::WorkzSessionMeta::default(),
        Some(text) => serde_json::from_str::<Frame>(&text)
            .map_err(|e| format!("parse state_frame failed: {e}"))?
            .agentz_workz
            .unwrap_or_default(),
    };
    if meta.team_id.is_some() || meta.pool_id.is_some() {
        return Err("team/pool plan metadata is unsupported".into());
    }
    Ok(SessionMetadata {
        workspace,
        workz: true,
        cwd: meta.cwd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("agentz-session-readonly-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn db(&self) -> std::path::PathBuf {
            self.0.join(".agentz/piscis.db")
        }
        fn initialize(&self) -> piscis_kernel::store::db::Database {
            std::fs::create_dir_all(self.0.join(".agentz")).unwrap();
            piscis_kernel::store::db::Database::open(&self.db()).unwrap()
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn existing_metadata_reads_without_database_mutation_or_missing_creation() {
        let dir = Directory::new();
        assert!(read_existing(&dir.0, "missing").is_err());
        assert!(!dir.0.join(".agentz").exists());
        let db = dir.initialize();
        let session = db.create_session_with_source(None, "workz").unwrap();
        db.update_session_state_frame_json(
            &session.id,
            Some(r#"{"agentz_workz":{"cwd":"C:/workspace"}}"#),
        )
        .unwrap();
        drop(db);
        let before = std::fs::read(dir.db()).unwrap();
        let meta = read_existing(&dir.0, &session.id).unwrap();
        assert!(meta.workz);
        assert_eq!(meta.cwd.as_deref(), Some("C:/workspace"));
        assert!(read_existing(&dir.0, "missing").is_err());
        assert_eq!(std::fs::read(dir.db()).unwrap(), before);
    }
    #[test]
    fn sources_and_workz_metadata_fail_closed() {
        let dir = Directory::new();
        let db = dir.initialize();
        for source in [
            "codez",
            "agentz",
            "workz",
            "workz-team",
            "pool",
            "piscis-heartbeat-pool",
            "remote",
            "chat",
        ] {
            let s = db.create_session_with_source(None, source).unwrap();
            assert_eq!(
                read_existing(&dir.0, &s.id).is_ok(),
                matches!(source, "codez" | "agentz" | "workz"),
                "{source}"
            );
        }
        let s = db.create_session_with_source(None, "workz").unwrap();
        for frame in [
            r#"{"agentz_workz":{"team_id":"team"}}"#,
            r#"{"agentz_workz":{"pool_id":"pool"}}"#,
            "malformed",
            r#"{"agentz_workz":{"cwd":8}}"#,
        ] {
            db.update_session_state_frame_json(&s.id, Some(frame))
                .unwrap();
            assert!(read_existing(&dir.0, &s.id).is_err());
        }
        db.update_session_state_frame_json(&s.id, None).unwrap();
        db.conn
            .execute(
                "UPDATE sessions SET pool_session_id='' WHERE id=?1",
                [&s.id],
            )
            .unwrap();
        assert!(read_existing(&dir.0, &s.id).is_err());
    }
    #[test]
    fn strict_storage_types_oversize_and_legacy_schema_are_rejected() {
        let dir = Directory::new();
        let db = dir.initialize();
        let s = db.create_session_with_source(None, "workz").unwrap();
        for column in [
            "source",
            "workspace_root",
            "pool_session_id",
            "state_frame_json",
        ] {
            db.conn
                .execute(
                    &format!("UPDATE sessions SET {column}=X'00' WHERE id=?1"),
                    [&s.id],
                )
                .unwrap();
            assert!(read_existing(&dir.0, &s.id).is_err(), "{column}");
            db.conn.execute("UPDATE sessions SET source='workz',workspace_root=NULL,pool_session_id=NULL,state_frame_json=NULL WHERE id=?1",[&s.id]).unwrap();
        }
        let huge = "x".repeat(MAX_FRAME_BYTES as usize + 1);
        db.update_session_state_frame_json(&s.id, Some(&huge))
            .unwrap();
        assert!(read_existing(&dir.0, &s.id).is_err());
        drop(db);
        let old = Directory::new();
        std::fs::create_dir_all(old.0.join(".agentz")).unwrap();
        let conn = Connection::open(old.db()).unwrap();
        conn.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,source TEXT); INSERT INTO sessions VALUES('old','codez');").unwrap();
        drop(conn);
        let before = std::fs::read(old.db()).unwrap();
        assert!(read_existing(&old.0, "old").is_err());
        assert_eq!(std::fs::read(old.db()).unwrap(), before);
    }
}
