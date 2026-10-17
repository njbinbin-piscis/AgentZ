//! Immutable bounded artifact payload snapshots. No live-path execution or automatic initialization.
use crate::plan::PlanBinding;
use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
const MAX_PAYLOAD: usize = 1_048_576;
const MAX_SCOPE_BYTES: i64 = 33_554_432;
const CREATE:&str="CREATE TABLE artifact_schema(singleton INTEGER NOT NULL PRIMARY KEY CHECK(singleton=1),version INTEGER NOT NULL);INSERT INTO artifact_schema VALUES(1,1);CREATE TABLE artifact_versions(task_id TEXT NOT NULL,environment_id TEXT NOT NULL,artifact_id TEXT NOT NULL,version INTEGER NOT NULL,metadata TEXT NOT NULL,payload BLOB NOT NULL,deleted INTEGER NOT NULL,PRIMARY KEY(task_id,environment_id,artifact_id,version));";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    ChangeSet,
    Document,
    Report,
    Webpage,
    Validation,
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("artifacts-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn scope(&self) -> PlanBinding {
            PlanBinding {
                project_id: crate::canonical_root(&self.0).unwrap(),
                task_id: "task".into(),
                environment_id: "environment".into(),
                binding_version: "a".repeat(64),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn input(version: i64, kind: Kind) -> CaptureInput {
        CaptureInput {
            artifact_id: "artifact".into(),
            version,
            kind,
            title: "Synthetic artifact".into(),
            source_relative_path: Some("out.txt".into()),
            media_type: "text/plain".into(),
            created_ms: version,
        }
    }
    #[test]
    fn capture_next_numbers_inside_insert_and_dedups_newest_only() {
        let f = Fixture::new();
        let scope = f.scope();
        let mut s = ArtifactStore::initialize(&f.0).unwrap();
        let created = |c: Capture| match c {
            Capture::Created(m) => m,
            other => panic!("expected created, got {other:?}"),
        };
        assert_eq!(created(s.capture_next(&scope, input(0, Kind::Document), b"one").unwrap()).version, 1);
        // Same digest, kind and title as the newest version: returned as existing, nothing written.
        assert!(matches!(
            s.capture_next(&scope, input(0, Kind::Document), b"one").unwrap(),
            Capture::Existing(m) if m.version == 1
        ));
        assert_eq!(created(s.capture_next(&scope, input(0, Kind::Document), b"two").unwrap()).version, 2);
        // Matching an older version is not a duplicate when the newest differs: a new version.
        assert_eq!(created(s.capture_next(&scope, input(0, Kind::Document), b"one").unwrap()).version, 3);
        assert_eq!(s.next_version(&scope, "artifact").unwrap(), 4);
        assert_eq!(s.list_versions(&scope, None, 200).unwrap().versions.len(), 3);
    }

    #[test]
    fn human_revision_requires_current_head_and_preserves_history() {
        let f = Fixture::new();
        let scope = f.scope();
        let mut s = ArtifactStore::initialize(&f.0).unwrap();
        s.capture_next(&scope, input(0, Kind::Document), b"original").unwrap();
        let edited = s.capture_next_if_latest(&scope, input(0, Kind::Document), b"edited", 1).unwrap();
        assert!(matches!(edited, Capture::Created(m) if m.version == 2));
        assert!(s.capture_next_if_latest(&scope, input(0, Kind::Document), b"stale", 1).is_err());
        let restored = s.capture_next_if_latest(&scope, input(0, Kind::Document), b"original", 2).unwrap();
        assert!(matches!(restored, Capture::Created(m) if m.version == 3));
        drop(s);
        let mut s = ArtifactStore::open_existing(&f.0, true).unwrap().unwrap();
        assert_eq!(s.read_version(&scope, "artifact", 1).unwrap().unwrap().payload, b"original");
        assert_eq!(s.read_version(&scope, "artifact", 2).unwrap().unwrap().payload, b"edited");
        assert_eq!(s.read_version(&scope, "artifact", 3).unwrap().unwrap().payload, b"original");
    }

    #[test]
    fn immutable_old_contents_survive_reopen_live_file_change_and_new_binding_revision() {
        let f = Fixture::new();
        let scope = f.scope();
        let mut s = ArtifactStore::initialize(&f.0).unwrap();
        std::fs::write(f.0.join("out.txt"), b"old").unwrap();
        s.capture(&scope, input(1, Kind::Document), b"old").unwrap();
        std::fs::write(f.0.join("out.txt"), b"new").unwrap();
        s.capture(&scope, input(2, Kind::Document), b"new").unwrap();
        drop(s);
        let mut fresh = scope.clone();
        fresh.binding_version = "b".repeat(64);
        let mut s = ArtifactStore::open_existing(&f.0, true).unwrap().unwrap();
        let old = s.read_version(&fresh, "artifact", 1).unwrap().unwrap();
        assert_eq!(old.payload, b"old");
        assert_eq!(old.metadata.binding, scope);
        assert_eq!(
            s.read_version(&fresh, "artifact", 2)
                .unwrap()
                .unwrap()
                .payload,
            b"new"
        );
    }
    #[test]
    fn code_and_noncode_payload_versions_are_idempotent_and_conflict_closed() {
        let f = Fixture::new();
        let scope = f.scope();
        let mut s = ArtifactStore::initialize(&f.0).unwrap();
        for (version, kind) in [
            (1, Kind::ChangeSet),
            (2, Kind::Document),
            (3, Kind::Report),
            (4, Kind::Webpage),
            (5, Kind::Validation),
        ] {
            assert!(matches!(
                s.capture(&scope, input(version, kind.clone()), b"fixture")
                    .unwrap(),
                Capture::Created(_)
            ));
            assert!(matches!(
                s.capture(&scope, input(version, kind.clone()), b"fixture")
                    .unwrap(),
                Capture::Existing(_)
            ));
            assert!(s
                .capture(&scope, input(version, kind), b"different")
                .is_err());
        }
        let page = s.list_versions(&scope, None, 2).unwrap();
        assert_eq!(page.versions.len(), 2);
        assert!(page.has_more);
        let next = s.list_versions(&scope, page.next_cursor, 200).unwrap();
        assert_eq!(next.versions.len(), 3);
        assert!(!next.has_more);
    }
    #[test]
    fn deleted_payload_removed_identity_never_reused_and_failure_is_not_success() {
        let f = Fixture::new();
        let scope = f.scope();
        let mut s = ArtifactStore::initialize(&f.0).unwrap();
        s.capture(&scope, input(1, Kind::Report), b"original")
            .unwrap();
        s.capture(&scope, input(2, Kind::Report), b"keep").unwrap();
        let c = Connection::open(f.0.join(".agentz/artifacts.db")).unwrap();
        c.execute_batch("CREATE TRIGGER reject_deletion BEFORE UPDATE ON artifact_versions BEGIN SELECT RAISE(ABORT,'deletion fixture'); END;").unwrap();
        assert!(s.delete_version(&scope, "artifact", 1).is_err());
        assert!(s.read_version(&scope, "artifact", 1).is_err());
        c.execute_batch("DROP TRIGGER reject_deletion;").unwrap();
        assert_eq!(
            s.read_version(&scope, "artifact", 1)
                .unwrap()
                .unwrap()
                .payload,
            b"original"
        );
        assert_eq!(
            s.delete_version(&scope, "artifact", 1).unwrap(),
            Delete::Deleted
        );
        assert_eq!(
            s.delete_version(&scope, "artifact", 1).unwrap(),
            Delete::AlreadyDeleted
        );
        assert!(s.read_version(&scope, "artifact", 1).unwrap().is_none());
        assert!(s
            .capture(&scope, input(1, Kind::Report), b"replacement")
            .is_err());
        assert_eq!(
            s.delete_version(&scope, "missing", 1).unwrap(),
            Delete::NotFound
        );
        assert_eq!(
            s.list_versions(&scope, None, 200).unwrap().versions.len(),
            1
        );
        let len: i64 = c
            .query_row(
                "SELECT length(payload) FROM artifact_versions WHERE version=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(len, 0);
    }
    #[test]
    fn missing_readonly_no_create_and_future_partial_schema_no_repair() {
        let f = Fixture::new();
        assert!(ArtifactStore::open_existing(&f.0, true).unwrap().is_none());
        assert!(!f.0.join(".agentz").exists());
        let s = ArtifactStore::initialize(&f.0).unwrap();
        drop(s);
        let file = f.0.join(".agentz/artifacts.db");
        let before = std::fs::read(&file).unwrap();
        let mut s = ArtifactStore::open_existing(&f.0, true).unwrap().unwrap();
        assert!(s.read_version(&f.scope(), "missing", 1).unwrap().is_none());
        assert!(s
            .capture(&f.scope(), input(1, Kind::Report), b"cannot write")
            .is_err());
        drop(s);
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let c = Connection::open(&file).unwrap();
        c.execute_batch("UPDATE artifact_schema SET version=2;")
            .unwrap();
        drop(c);
        let before = std::fs::read(&file).unwrap();
        assert!(ArtifactStore::initialize(&f.0).is_err());
        assert!(ArtifactStore::open_existing(&f.0, true).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let c = Connection::open(&file).unwrap();
        c.execute_batch("DROP TABLE artifact_versions;").unwrap();
        drop(c);
        assert!(ArtifactStore::initialize(&f.0).is_err());
    }
    #[test]
    fn wrong_scope_payload_corruption_and_budget_reject() {
        let f = Fixture::new();
        let scope = f.scope();
        let mut s = ArtifactStore::initialize(&f.0).unwrap();
        s.capture(&scope, input(1, Kind::Report), b"valid").unwrap();
        let mut wrong = scope.clone();
        wrong.environment_id = "other".into();
        assert!(s.read_version(&wrong, "artifact", 1).unwrap().is_none());
        wrong.project_id = "forged".into();
        assert!(s.read_version(&wrong, "artifact", 1).is_err());
        assert!(s
            .capture(&scope, input(2, Kind::Report), &vec![0u8; MAX_PAYLOAD + 1])
            .is_err());
        let c = Connection::open(f.0.join(".agentz/artifacts.db")).unwrap();
        c.execute(
            "UPDATE artifact_versions SET payload=?1",
            [b"corrupt".as_slice()],
        )
        .unwrap();
        assert!(s.read_version(&scope, "artifact", 1).is_err());
        assert!(s.capture(&scope, input(2, Kind::Report), b"new").is_err());
        assert!(s.delete_version(&scope, "artifact", 1).is_err());
    }
    #[test]
    fn after_triggers_cannot_return_false_capture_or_delete_success() {
        let f = Fixture::new();
        let scope = f.scope();
        let mut s = ArtifactStore::initialize(&f.0).unwrap();
        let file = f.0.join(".agentz/artifacts.db");
        let c = Connection::open(&file).unwrap();
        c.execute_batch("CREATE TRIGGER erase_capture AFTER INSERT ON artifact_versions BEGIN DELETE FROM artifact_versions WHERE task_id=NEW.task_id AND environment_id=NEW.environment_id AND artifact_id=NEW.artifact_id AND version=NEW.version; END;").unwrap();
        let before = std::fs::read(&file).unwrap();
        assert!(s.capture(&scope, input(1, Kind::Report), b"new").is_err());
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let count: i64 = c
            .query_row("SELECT COUNT(*) FROM artifact_versions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        c.execute_batch("DROP TRIGGER erase_capture;").unwrap();
        s.capture(&scope, input(1, Kind::Report), b"original")
            .unwrap();
        c.execute_batch("CREATE TRIGGER resurrect_delete AFTER UPDATE ON artifact_versions BEGIN UPDATE artifact_versions SET payload=OLD.payload,deleted=OLD.deleted WHERE task_id=NEW.task_id AND environment_id=NEW.environment_id AND artifact_id=NEW.artifact_id AND version=NEW.version; END;").unwrap();
        let before = std::fs::read(&file).unwrap();
        assert!(s.delete_version(&scope, "artifact", 1).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), before);
        assert!(ArtifactStore::open_existing(&f.0, true).is_err());
        assert!(ArtifactStore::initialize(&f.0).is_err());
        c.execute_batch("DROP TRIGGER resurrect_delete;").unwrap();
        assert_eq!(
            s.read_version(&scope, "artifact", 1)
                .unwrap()
                .unwrap()
                .payload,
            b"original"
        );
    }
    #[test]
    fn empty_page_after_deleted_last_cursor_has_no_next_cursor() {
        let f = Fixture::new();
        let scope = f.scope();
        let mut s = ArtifactStore::initialize(&f.0).unwrap();
        s.capture(&scope, input(1, Kind::Report), b"last").unwrap();
        let page = s.list_versions(&scope, None, 1).unwrap();
        let after = page.next_cursor.unwrap();
        assert_eq!(
            s.delete_version(&scope, "artifact", 1).unwrap(),
            Delete::Deleted
        );
        let empty = s.list_versions(&scope, Some(after), 1).unwrap();
        assert!(empty.versions.is_empty());
        assert!(empty.next_cursor.is_none());
        assert!(!empty.has_more);
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub schema_version: u32,
    pub binding: PlanBinding,
    pub artifact_id: String,
    pub version: i64,
    pub kind: Kind,
    pub title: String,
    pub source_relative_path: Option<String>,
    pub media_type: String,
    pub payload_sha256: String,
    pub bytes: i64,
    pub created_ms: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub metadata: Metadata,
    pub payload: Vec<u8>,
}
pub struct CaptureInput {
    pub artifact_id: String,
    pub version: i64,
    pub kind: Kind,
    pub title: String,
    pub source_relative_path: Option<String>,
    pub media_type: String,
    pub created_ms: i64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Capture {
    Created(Metadata),
    Existing(Metadata),
}
#[derive(Debug, PartialEq, Eq)]
pub enum Delete {
    Deleted,
    NotFound,
    AlreadyDeleted,
}
#[derive(Debug, Serialize)]
pub struct Page {
    pub versions: Vec<Metadata>,
    pub next_cursor: Option<(String, i64)>,
    pub has_more: bool,
}
pub struct ArtifactStore {
    conn: Connection,
    project: String,
}
fn err(e: impl std::fmt::Display) -> String {
    format!("artifact store: {e}")
}
fn text(s: &str, max: usize) -> Result<(), String> {
    if s.trim().is_empty() || s.trim() != s || s.len() > max || s.chars().any(char::is_control) {
        return Err(err("invalid artifact identity/text"));
    }
    Ok(())
}
fn binding(b: &PlanBinding, project: &str) -> Result<(), String> {
    text(&b.task_id, 256)?;
    text(&b.environment_id, 256)?;
    if b.project_id != project
        || b.binding_version.len() != 64
        || !b.binding_version.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(err("invalid artifact binding"));
    }
    Ok(())
}
fn metadata(m: &Metadata, project: &str) -> Result<(), String> {
    binding(&m.binding, project)?;
    text(&m.artifact_id, 256)?;
    text(&m.title, 1024)?;
    text(&m.media_type, 128)?;
    if m.schema_version != 1
        || m.version < 1
        || !(0..=MAX_PAYLOAD as i64).contains(&m.bytes)
        || m.created_ms < 0
        || m.payload_sha256.len() != 64
        || !m
            .payload_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(err("invalid artifact version/metadata"));
    }
    if let Some(p) = &m.source_relative_path {
        if p.is_empty()
            || p.len() > 4096
            || p.contains(':')
            || p.contains('\\')
            || p.starts_with('/')
            || p.chars().any(char::is_control)
            || p.split('/').any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(err("invalid artifact source reference"));
        }
    }
    Ok(())
}
fn layout(tx: &Transaction<'_>) -> Result<bool, String> {
    let triggers:i64=tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND tbl_name IN ('artifact_schema','artifact_versions')",[],|r|r.get(0)).map_err(err)?;
    if triggers != 0 {
        return Err(err("artifact namespace triggers are unsupported"));
    }
    let user_version: i64 = tx
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(err)?;
    if user_version != 0 {
        return Err(err("unsupported artifact database version"));
    }
    let n:i64=tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name IN ('artifact_schema','artifact_versions')",[],|r|r.get(0)).map_err(err)?;
    if n == 0 {
        return Ok(false);
    }
    if n != 2 {
        return Err(err("partial artifact namespace"));
    }
    let n:i64=tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('artifact_schema','artifact_versions')",[],|r|r.get(0)).map_err(err)?;
    if n != 2 {
        return Err(err("artifact namespace requires tables"));
    }
    for (table, want) in [
        (
            "artifact_schema",
            vec![("singleton", "INTEGER", 1, 1), ("version", "INTEGER", 1, 0)],
        ),
        (
            "artifact_versions",
            vec![
                ("task_id", "TEXT", 1, 1),
                ("environment_id", "TEXT", 1, 2),
                ("artifact_id", "TEXT", 1, 3),
                ("version", "INTEGER", 1, 4),
                ("metadata", "TEXT", 1, 0),
                ("payload", "BLOB", 1, 0),
                ("deleted", "INTEGER", 1, 0),
            ],
        ),
    ] {
        let mut q = tx
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(err)?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        if rows
            != want
                .into_iter()
                .map(|(a, b, c, d)| (a.into(), b.into(), c, d))
                .collect::<Vec<_>>()
        {
            return Err(err("unsupported artifact layout"));
        }
    }
    let mut q = tx
        .prepare("SELECT singleton,version FROM artifact_schema")
        .map_err(err)?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    if rows != vec![(1, 1)] {
        return Err(err("unsupported artifact schema"));
    }
    Ok(true)
}
fn validate_scope(tx: &Transaction<'_>, scope: &PlanBinding, project: &str) -> Result<(), String> {
    let (count,total):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(length(payload)),0) FROM artifact_versions WHERE task_id=?1 AND environment_id=?2",params![scope.task_id,scope.environment_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
    if count > 1000 || !(0..=MAX_SCOPE_BYTES).contains(&total) {
        return Err(err("artifact scope storage budget"));
    }
    let mut q=tx.prepare("SELECT artifact_id,version FROM artifact_versions WHERE task_id=?1 AND environment_id=?2").map_err(err)?;
    let keys = q
        .query_map(params![scope.task_id, scope.environment_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    drop(q);
    for (id, version) in keys {
        read(tx, scope, &id, version, project)?;
    }
    Ok(())
}
fn read(
    tx: &Transaction<'_>,
    scope: &PlanBinding,
    id: &str,
    version: i64,
    project: &str,
) -> Result<Option<Artifact>, String> {
    let row:Option<(String,i64,i64,i64)>=tx.query_row("SELECT metadata,length(payload),typeof(payload)='blob',deleted FROM artifact_versions WHERE task_id=?1 AND environment_id=?2 AND artifact_id=?3 AND version=?4",params![scope.task_id,scope.environment_id,id,version],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
    let Some((raw, len, blob, deleted)) = row else {
        return Ok(None);
    };
    if raw.len() > 16384 || !(0..=MAX_PAYLOAD as i64).contains(&len) || blob != 1 {
        return Err(err("artifact payload budget/storage type"));
    }
    let m: Metadata = serde_json::from_str(&raw).map_err(err)?;
    metadata(&m, project)?;
    if m.binding.task_id != scope.task_id
        || m.binding.environment_id != scope.environment_id
        || m.artifact_id != id
        || m.version != version
        || !matches!(deleted, 0 | 1)
        || (deleted == 0 && m.bytes != len)
        || (deleted == 1 && len != 0)
    {
        return Err(err("artifact row scope mismatch"));
    }
    if deleted == 1 {
        return Ok(None);
    }
    let payload:Vec<u8>=tx.query_row("SELECT payload FROM artifact_versions WHERE task_id=?1 AND environment_id=?2 AND artifact_id=?3 AND version=?4",params![scope.task_id,scope.environment_id,id,version],|r|r.get(0)).map_err(err)?;
    if format!("{:x}", Sha256::digest(&payload)) != m.payload_sha256 {
        return Err(err("artifact content digest mismatch"));
    }
    Ok(Some(Artifact {
        metadata: m,
        payload,
    }))
}
impl ArtifactStore {
    /// Explicit creation only; existing partial/unsupported databases are never repaired.
    pub fn initialize(project: &Path) -> Result<Self, String> {
        let project = crate::canonical_root(project)?;
        std::fs::create_dir_all(Path::new(&project).join(".agentz")).map_err(err)?;
        let mut s = Self {
            conn: Connection::open(Path::new(&project).join(".agentz/artifacts.db"))
                .map_err(err)?,
            project,
        };
        s.conn.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        {
            let tx = s
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(err)?;
            if !layout(&tx)? {
                let count:i64=tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0)).map_err(err)?;
                if count != 0 {
                    return Err(err("foreign artifact database"));
                }
                tx.execute_batch(CREATE).map_err(err)?;
            }
            layout(&tx)?;
            tx.commit().map_err(err)?;
        }
        Ok(s)
    }
    pub fn open_existing(project: &Path, readonly: bool) -> Result<Option<Self>, String> {
        let project = crate::canonical_root(project)?;
        let path = Path::new(&project).join(".agentz/artifacts.db");
        if !path.try_exists().map_err(err)? {
            return Ok(None);
        }
        let mut conn = Connection::open_with_flags(
            path,
            if readonly {
                OpenFlags::SQLITE_OPEN_READ_ONLY
            } else {
                OpenFlags::SQLITE_OPEN_READ_WRITE
            },
        )
        .map_err(err)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        {
            let tx = conn.transaction().map_err(err)?;
            if !layout(&tx)? {
                return Err(err("artifact namespace is uninitialized"));
            }
        }
        Ok(Some(Self { conn, project }))
    }
    /// Capture `payload` at exactly `input.version`. Immutable: the same identity with other
    /// content is a conflict.
    pub fn capture(
        &mut self,
        scope: &PlanBinding,
        input: CaptureInput,
        payload: &[u8],
    ) -> Result<Capture, String> {
        self.capture_inner(scope, input, payload, false, None)
    }

    /// Capture as the next version of `input.artifact_id`. `input.version` is ignored.
    /// The number is taken as `MAX(version) + 1` inside the same immediate transaction as the
    /// insert, so concurrent captures cannot pick the same number. If the newest version already
    /// has the same payload digest, kind and title, it is returned as `Existing` and nothing is
    /// written.
    pub fn capture_next(
        &mut self,
        scope: &PlanBinding,
        input: CaptureInput,
        payload: &[u8],
    ) -> Result<Capture, String> {
        self.capture_inner(scope, input, payload, true, None)
    }

    /// Save an explicit human revision only if the caller saw the current newest version.
    /// A stale editor must refresh and reconcile rather than append over another edit.
    pub fn capture_next_if_latest(
        &mut self,
        scope: &PlanBinding,
        input: CaptureInput,
        payload: &[u8],
        expected_latest: i64,
    ) -> Result<Capture, String> {
        if expected_latest < 0 { return Err(err("invalid expected artifact version")); }
        self.capture_inner(scope, input, payload, true, Some(expected_latest))
    }

    fn capture_inner(
        &mut self,
        scope: &PlanBinding,
        input: CaptureInput,
        payload: &[u8],
        next: bool,
        expected_latest: Option<i64>,
    ) -> Result<Capture, String> {
        if payload.len() > MAX_PAYLOAD {
            return Err(err("artifact exceeds 1 MiB"));
        }
        let CaptureInput {
            artifact_id,
            version: requested_version,
            kind,
            title,
            source_relative_path,
            media_type,
            created_ms,
        } = input;
        let id = artifact_id.as_str();
        let payload_sha256 = format!("{:x}", Sha256::digest(payload));
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        if !layout(&tx)? {
            return Err(err("artifact namespace missing"));
        }
        validate_scope(&tx, scope, &self.project)?;
        let version = if next {
            let latest: i64 = tx
                .query_row(
                    "SELECT COALESCE(MAX(version),0) FROM artifact_versions WHERE task_id=?1 AND environment_id=?2 AND artifact_id=?3",
                    params![scope.task_id, scope.environment_id, id],
                    |r| r.get(0),
                )
                .map_err(err)?;
            if expected_latest.is_some_and(|expected| expected != latest) {
                return Err(err("stale artifact edit; refresh before saving"));
            }
            if latest >= 1 {
                if let Some(newest) = read(&tx, scope, id, latest, &self.project)? {
                    if newest.metadata.payload_sha256 == payload_sha256
                        && newest.metadata.kind == kind
                        && newest.metadata.title == title
                    {
                        return Ok(Capture::Existing(newest.metadata));
                    }
                }
            }
            latest
                .checked_add(1)
                .ok_or(err("artifact version exhausted"))?
        } else {
            requested_version
        };
        let m = Metadata {
            schema_version: 1,
            binding: scope.clone(),
            artifact_id: id.into(),
            version,
            kind,
            title,
            source_relative_path,
            media_type,
            payload_sha256,
            bytes: payload.len() as i64,
            created_ms,
        };
        metadata(&m, &self.project)?;
        if let Some(old) = read(&tx, scope, id, version, &self.project)? {
            if old.metadata != m || old.payload != payload {
                return Err(err("immutable artifact version conflict"));
            }
            return Ok(Capture::Existing(old.metadata));
        }
        let existing:i64=tx.query_row("SELECT COUNT(*) FROM artifact_versions WHERE task_id=?1 AND environment_id=?2 AND artifact_id=?3 AND version=?4",params![scope.task_id,scope.environment_id,id,version],|r|r.get(0)).map_err(err)?;
        if existing != 0 {
            return Err(err("deleted artifact version identity cannot be reused"));
        }
        let count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM artifact_versions WHERE task_id=?1 AND environment_id=?2",
                params![scope.task_id, scope.environment_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if count >= 1000 {
            return Err(err("artifact version count exhausted"));
        }
        let used:i64=tx.query_row("SELECT COALESCE(SUM(length(payload)),0) FROM artifact_versions WHERE task_id=?1 AND environment_id=?2",params![scope.task_id,scope.environment_id],|r|r.get(0)).map_err(err)?;
        if used < 0
            || used
                .checked_add(m.bytes)
                .ok_or(err("artifact budget overflow"))?
                > MAX_SCOPE_BYTES
        {
            return Err(err("artifact scope exceeds 32 MiB"));
        }
        let inserted = tx
            .execute(
                "INSERT INTO artifact_versions VALUES(?1,?2,?3,?4,?5,?6,0)",
                params![
                    scope.task_id,
                    scope.environment_id,
                    id,
                    version,
                    serde_json::to_string(&m).map_err(err)?,
                    payload
                ],
            )
            .map_err(err)?;
        if inserted != 1 {
            return Err(err("artifact capture did not persist"));
        }
        tx.commit().map_err(err)?;
        Ok(Capture::Created(m))
    }
    pub fn read_version(
        &mut self,
        scope: &PlanBinding,
        id: &str,
        version: i64,
    ) -> Result<Option<Artifact>, String> {
        binding(scope, &self.project)?;
        text(id, 256)?;
        if version < 1 {
            return Err(err("invalid artifact version"));
        }
        let tx = self.conn.transaction().map_err(err)?;
        if !layout(&tx)? {
            return Err(err("artifact namespace missing"));
        }
        validate_scope(&tx, scope, &self.project)?;
        read(&tx, scope, id, version, &self.project)
    }
    pub fn list_versions(
        &mut self,
        scope: &PlanBinding,
        after: Option<(String, i64)>,
        limit: usize,
    ) -> Result<Page, String> {
        binding(scope, &self.project)?;
        if !(1..=200).contains(&limit) {
            return Err(err("artifact page limit must be 1..200"));
        }
        if let Some((id, version)) = &after {
            text(id, 256)?;
            if *version < 1 {
                return Err(err("invalid artifact cursor"));
            }
        }
        let tx = self.conn.transaction().map_err(err)?;
        if !layout(&tx)? {
            return Err(err("artifact namespace missing"));
        }
        validate_scope(&tx, scope, &self.project)?;
        let (id, version) = after
            .as_ref()
            .map(|(a, b)| (Some(a.as_str()), *b))
            .unwrap_or((None, 0));
        let mut q=tx.prepare("SELECT artifact_id,version FROM artifact_versions WHERE task_id=?1 AND environment_id=?2 AND deleted=0 AND (?3 IS NULL OR artifact_id>?3 OR (artifact_id=?3 AND version>?4)) ORDER BY artifact_id,version LIMIT ?5").map_err(err)?;
        let keys = q
            .query_map(
                params![
                    scope.task_id,
                    scope.environment_id,
                    id,
                    version,
                    (limit + 1) as i64
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        drop(q);
        let has_more = keys.len() > limit;
        let mut versions = Vec::new();
        for (id, version) in keys.into_iter().take(limit) {
            versions.push(
                read(&tx, scope, &id, version, &self.project)?
                    .ok_or(err("artifact list missing version"))?
                    .metadata,
            );
        }
        let next_cursor = versions.last().map(|m| (m.artifact_id.clone(), m.version));
        Ok(Page {
            versions,
            next_cursor,
            has_more,
        })
    }
    /// Advisory version candidate, includes tombstones. Capture still CASes immutable identity;
    /// concurrent callers must handle conflict, never overwrite a version.
    pub fn next_version(&mut self, scope: &PlanBinding, id: &str) -> Result<i64, String> {
        binding(scope, &self.project)?;
        text(id, 256)?;
        let tx = self.conn.transaction().map_err(err)?;
        if !layout(&tx)? {
            return Err(err("artifact namespace missing"));
        }
        validate_scope(&tx, scope, &self.project)?;
        let current:i64=tx.query_row("SELECT COALESCE(MAX(version),0) FROM artifact_versions WHERE task_id=?1 AND environment_id=?2 AND artifact_id=?3",params![scope.task_id,scope.environment_id,id],|r|r.get(0)).map_err(err)?;
        current
            .checked_add(1)
            .ok_or(err("artifact version exhausted"))
    }
    /// Deletes captured payload only, never a live source file. Return success only after commit.
    pub fn delete_version(
        &mut self,
        scope: &PlanBinding,
        id: &str,
        version: i64,
    ) -> Result<Delete, String> {
        binding(scope, &self.project)?;
        text(id, 256)?;
        if version < 1 {
            return Err(err("invalid artifact version"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        if !layout(&tx)? {
            return Err(err("artifact namespace missing"));
        }
        validate_scope(&tx, scope, &self.project)?;
        if read(&tx, scope, id, version, &self.project)?.is_none() {
            let deleted:Option<i64> = tx.query_row("SELECT deleted FROM artifact_versions WHERE task_id=?1 AND environment_id=?2 AND artifact_id=?3 AND version=?4",params![scope.task_id,scope.environment_id,id,version],|r|r.get(0)).optional().map_err(err)?;
            return Ok(if deleted == Some(1) {
                Delete::AlreadyDeleted
            } else {
                Delete::NotFound
            });
        }
        let count=tx.execute("UPDATE artifact_versions SET payload=X'',deleted=1 WHERE deleted=0 AND task_id=?1 AND environment_id=?2 AND artifact_id=?3 AND version=?4",params![scope.task_id,scope.environment_id,id,version]).map_err(err)?;
        if count != 1 {
            return Err(err("artifact deletion did not persist"));
        }
        tx.commit().map_err(err)?;
        Ok(Delete::Deleted)
    }
}
