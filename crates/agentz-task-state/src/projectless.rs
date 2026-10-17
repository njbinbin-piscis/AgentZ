//! Managed projectless directories. No deletion or implicit Git initialization.
use std::path::{Path, PathBuf};

fn checked_id(id: &str) -> Result<(), String> {
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
    {
        return Err("task id must be a canonical lowercase UUID".into());
    }
    Ok(())
}
fn ordinary_directory(path: &Path) -> Result<(), String> {
    let m = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("managed directory is not an ordinary directory".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err("managed directory is a reparse point".into());
        }
    }
    Ok(())
}
/// Caller supplies the dedicated application-owned root, never a user project.
pub fn create(root: &Path, task_id: &str) -> Result<PathBuf, String> {
    checked_id(task_id)?;
    ordinary_directory(root)?;
    let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let dir = root.join(task_id);
    // Existing directories are never adopted or overwritten.
    std::fs::create_dir(&dir).map_err(|e| format!("create managed task directory: {e}"))?;
    use std::io::Write;
    let mut marker = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(".agentz-owned-task"))
        .map_err(|e| format!("write ownership marker; directory retained: {e}"))?;
    marker
        .write_all(task_id.as_bytes())
        .and_then(|_| marker.sync_all())
        .map_err(|e| e.to_string())?;
    open(&root, task_id)
}
/// Reopening verifies an existing marker; never recreates missing data.
pub fn open(root: &Path, task_id: &str) -> Result<PathBuf, String> {
    checked_id(task_id)?;
    ordinary_directory(root)?;
    let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let dir = root.join(task_id);
    ordinary_directory(&dir)?;
    let resolved = std::fs::canonicalize(&dir).map_err(|e| e.to_string())?;
    if resolved.parent() != Some(root.as_path()) {
        return Err("task directory escapes owned root".into());
    }
    let marker_path = resolved.join(".agentz-owned-task");
    let marker = std::fs::symlink_metadata(&marker_path).map_err(|e| e.to_string())?;
    if !marker.is_file() || marker.file_type().is_symlink() || marker.len() != 36 {
        return Err("invalid task ownership marker".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if marker.file_attributes() & 0x400 != 0 {
            return Err("task ownership marker is a reparse point".into());
        }
    }
    let contents = std::fs::read_to_string(marker_path).map_err(|e| e.to_string())?;
    if contents != task_id {
        return Err("task ownership marker mismatch".into());
    }
    Ok(resolved)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reopen_preserves_artifact_and_never_initializes_git() {
        let root =
            std::env::temp_dir().join(format!("agentz-projectless-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let dir = create(&root, &id).unwrap();
        std::fs::write(dir.join("artifact.txt"), "retained").unwrap();
        assert_eq!(open(&root, &id).unwrap(), dir);
        assert_eq!(
            std::fs::read_to_string(dir.join("artifact.txt")).unwrap(),
            "retained"
        );
        assert!(!dir.join(".git").exists());
        assert!(create(&root, &id).is_err());
        assert!(open(&root, "../escape").is_err());
        std::fs::write(dir.join(".agentz-owned-task"), "bad").unwrap();
        assert!(open(&root, &id).is_err());
        assert!(dir.join("artifact.txt").exists());
        // Remove only this unique test-owned temporary root.
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
/// Persistent registration is separate from a filesystem marker. A marker alone
/// is never enough for a catalog lookup; this catalog exposes no deletion API.
pub struct Catalog {
    connection: rusqlite::Connection,
    root: PathBuf,
}
/// Host-selected application data layout. This is not a renderer path resolver.
/// Reopen never provisions missing roots or a missing catalog.
pub struct AppDataStore {
    catalog: Catalog,
    bindings_database: PathBuf,
}
impl AppDataStore {
    pub fn initialize(app_data: &Path) -> Result<Self, String> {
        ordinary_directory(app_data)?;
        let base = app_data.join("projectless");
        provision_directory(&base)?;
        let root = base.join("workspaces");
        provision_directory(&root)?;
        let database = base.join("catalog.sqlite");
        reject_database_link(&database)?;
        Ok(Self {
            catalog: Catalog::initialize(&database, &root)?,
            bindings_database: base.join("session-bindings.sqlite"),
        })
    }
    pub fn open_existing(app_data: &Path) -> Result<Self, String> {
        ordinary_directory(app_data)?;
        let base = app_data.join("projectless");
        ordinary_directory(&base)?;
        let database = base.join("catalog.sqlite");
        reject_database_link(&database)?;
        Ok(Self {
            catalog: Catalog::open_existing(&database, &base.join("workspaces"))?,
            bindings_database: base.join("session-bindings.sqlite"),
        })
    }
    pub fn create_task(&self, task_id: &str) -> Result<PathBuf, String> {
        self.catalog.create_task(task_id)
    }
    pub fn reopen_task(&self, task_id: &str) -> Result<PathBuf, String> {
        self.catalog.reopen_task(task_id)
    }
    /// Host supplies the canonical session database project root. A binding is
    /// insert-only; neither another task nor a retry can redirect its directory.
    pub fn bind_session(
        &self,
        project: &Path,
        session_id: &str,
        directory_id: &str,
    ) -> Result<PathBuf, String> {
        checked_id(session_id)?;
        checked_id(directory_id)?;
        ordinary_directory(project)?;
        let project = std::fs::canonicalize(project).map_err(|e| e.to_string())?;
        let project = project.to_str().ok_or("session project is not UTF-8")?;
        let directory = self.catalog.reopen_task(directory_id)?;
        reject_database_link(&self.bindings_database)?;
        // A durable initialization watermark in the directory catalog prevents a
        // lost binding database being silently replaced by an empty one.
        let initialize = self
            .catalog
            .reserve_binding_store(&self.bindings_database)?;
        let mut connection =
            rusqlite::Connection::open(&self.bindings_database).map_err(|e| e.to_string())?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        if initialize {
            transaction
                .execute_batch(BINDINGS_DDL)
                .map_err(|e| e.to_string())?;
            transaction
                .execute(
                    "INSERT INTO projectless_binding_identity VALUES(1,1,?1)",
                    [self
                        .catalog
                        .root
                        .to_str()
                        .ok_or("owned root is not UTF-8")?],
                )
                .map_err(|e| e.to_string())?;
        }
        validate_bindings(&transaction, &self.catalog.root)?;
        use rusqlite::OptionalExtension;
        let existing: Option<String> = transaction.query_row("SELECT directory_id FROM projectless_session_bindings WHERE project_root=?1 AND session_id=?2", [project, session_id], |row| row.get(0)).optional().map_err(|e| e.to_string())?;
        match existing {
            Some(bound) if bound == directory_id => (),
            Some(_) => return Err("session already bound to another directory".into()),
            None => {
                transaction
                    .execute(
                        "INSERT INTO projectless_session_bindings VALUES(?1,?2,?3)",
                        [project, session_id, directory_id],
                    )
                    .map_err(|e| format!("session binding failed; directory retained: {e}"))?;
            }
        }
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(directory)
    }
    /// Missing binding storage is an error, never proof that fallback is safe.
    pub fn reopen_session(&self, project: &Path, session_id: &str) -> Result<PathBuf, String> {
        checked_id(session_id)?;
        ordinary_directory(project)?;
        let project = std::fs::canonicalize(project).map_err(|e| e.to_string())?;
        let project = project.to_str().ok_or("session project is not UTF-8")?;
        reject_database_link(&self.bindings_database)?;
        let mut connection = rusqlite::Connection::open_with_flags(
            &self.bindings_database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
        )
        .map_err(|e| e.to_string())?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        validate_bindings(&transaction, &self.catalog.root)?;
        let directory_id: String = transaction.query_row("SELECT directory_id FROM projectless_session_bindings WHERE project_root=?1 AND session_id=?2", [project, session_id], |row| row.get(0)).map_err(|e| format!("session has no managed directory binding: {e}"))?;
        let directory = self.catalog.reopen_task(&directory_id)?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(directory)
    }

    /// Read-only lookup of the managed directory bound to `session_id` under `project`.
    /// `Ok(None)` means the session is not projectless: no binding exists and the binding
    /// store was never created. A bound session whose store is gone or unreadable is an error,
    /// because the caller must not fall back to the shared project. Never creates anything.
    pub fn session_directory(
        &self,
        project: &Path,
        session_id: &str,
    ) -> Result<Option<PathBuf>, String> {
        use rusqlite::OptionalExtension;
        checked_id(session_id)?;
        ordinary_directory(project)?;
        let project = std::fs::canonicalize(project).map_err(|e| e.to_string())?;
        let project = project.to_str().ok_or("session project is not UTF-8")?;
        reject_database_link(&self.bindings_database)?;
        if !self.bindings_database.is_file() {
            return if self.catalog.binding_store_initialized()? {
                Err("binding store missing after initialization; retained bindings require recovery".into())
            } else {
                Ok(None)
            };
        }
        let connection = rusqlite::Connection::open_with_flags(
            &self.bindings_database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;
        validate_bindings(&connection, &self.catalog.root)?;
        let directory_id: Option<String> = connection
            .query_row(
                "SELECT directory_id FROM projectless_session_bindings WHERE project_root=?1 AND session_id=?2",
                [project, session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match directory_id {
            None => Ok(None),
            Some(id) => self.catalog.reopen_task(&id).map(Some),
        }
    }
}
const BINDINGS_TABLE: &str = "CREATE TABLE projectless_session_bindings(project_root TEXT NOT NULL,session_id TEXT NOT NULL,directory_id TEXT NOT NULL UNIQUE,PRIMARY KEY(project_root,session_id))";
const BINDINGS_IDENTITY: &str = "CREATE TABLE projectless_binding_identity(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema_version INTEGER NOT NULL,owned_root TEXT NOT NULL)";
const BINDINGS_DDL: &str = "CREATE TABLE projectless_session_bindings(project_root TEXT NOT NULL,session_id TEXT NOT NULL,directory_id TEXT NOT NULL UNIQUE,PRIMARY KEY(project_root,session_id));CREATE TABLE projectless_binding_identity(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema_version INTEGER NOT NULL,owned_root TEXT NOT NULL);";
fn validate_bindings(connection: &rusqlite::Connection, root: &Path) -> Result<(), String> {
    for (name, expected) in [
        ("projectless_session_bindings", BINDINGS_TABLE),
        ("projectless_binding_identity", BINDINGS_IDENTITY),
    ] {
        let sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if sql != expected {
            return Err("unexpected session binding layout".into());
        }
    }
    let triggers: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='trigger'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if triggers != 0 {
        return Err("unexpected session binding triggers".into());
    }
    let (version, stored_root): (i64, String) = connection
        .query_row(
            "SELECT schema_version,owned_root FROM projectless_binding_identity WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    if version != 1 || root.to_str() != Some(stored_root.as_str()) {
        return Err("session binding schema or root mismatch".into());
    }
    Ok(())
}
fn provision_directory(path: &Path) -> Result<(), String> {
    match std::fs::create_dir(path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.to_string()),
    }
    ordinary_directory(path)
}
fn reject_database_link(path: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("catalog is not an ordinary file".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("catalog is a reparse point".into());
        }
    }
    Ok(())
}
fn validate_catalog(connection: &rusqlite::Connection, root: &Path) -> Result<(), String> {
    let expected = [
        ("projectless_directories", "CREATE TABLE projectless_directories(task_id TEXT PRIMARY KEY, schema_version INTEGER NOT NULL CHECK(schema_version=1))"),
        ("projectless_catalog_identity", "CREATE TABLE projectless_catalog_identity(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema_version INTEGER NOT NULL,owned_root TEXT NOT NULL)")];
    for (name, sql) in expected {
        let actual: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let normalize = |value: &str| {
            value
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        };
        if normalize(&actual) != normalize(sql) {
            return Err("unexpected catalog table layout".into());
        }
    }
    let triggers: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='trigger'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if triggers != 0 {
        return Err("unexpected catalog triggers".into());
    }
    let (version, stored_root): (i64, String) = connection
        .query_row(
            "SELECT schema_version,owned_root FROM projectless_catalog_identity WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    if version != 1 || root.to_str() != Some(stored_root.as_str()) {
        return Err("catalog schema or owned root mismatch".into());
    }
    Ok(())
}
impl Catalog {
    /// Read-only: whether the binding store was ever initialized (its watermark exists).
    fn binding_store_initialized(&self) -> Result<bool, String> {
        use rusqlite::OptionalExtension;
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        validate_catalog(&transaction, &self.root)?;
        let found: Option<String> = transaction
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='projectless_binding_watermark'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(found.is_some())
    }

    fn reserve_binding_store(&self, database: &Path) -> Result<bool, String> {
        const DDL: &str = "CREATE TABLE projectless_binding_watermark(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema_version INTEGER NOT NULL)";
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        validate_catalog(&transaction, &self.root)?;
        use rusqlite::OptionalExtension;
        let definition: Option<String> = transaction.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='projectless_binding_watermark'", [], |row| row.get(0)).optional().map_err(|e| e.to_string())?;
        let initialize = match definition {
            Some(sql) => {
                if sql != DDL {
                    return Err("unexpected binding watermark layout".into());
                }
                let version: i64 = transaction.query_row("SELECT schema_version FROM projectless_binding_watermark WHERE singleton=1", [], |row| row.get(0)).map_err(|e| e.to_string())?;
                if version != 1 || !database.is_file() {
                    return Err(
                        "binding store missing or unsupported; retained data requires recovery"
                            .into(),
                    );
                }
                false
            }
            None => {
                transaction.execute_batch(DDL).map_err(|e| e.to_string())?;
                transaction
                    .execute("INSERT INTO projectless_binding_watermark VALUES(1,1)", [])
                    .map_err(|e| e.to_string())?;
                !database.exists()
            }
        };
        // Deliberately committed before provisioning the separate database. A
        // crash here is reported as incomplete initialization, never repaired
        // by discarding previous identity. There is no cross-database atomicity.
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(initialize)
    }
    pub fn initialize(database: &Path, owned_root: &Path) -> Result<Self, String> {
        if database.exists() {
            return Self::open_existing(database, owned_root);
        }
        ordinary_directory(owned_root)?;
        let root = std::fs::canonicalize(owned_root).map_err(|e| e.to_string())?;
        let root_text = root.to_str().ok_or("owned root is not UTF-8")?;
        let mut connection = rusqlite::Connection::open(database).map_err(|e| e.to_string())?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction.execute_batch("CREATE TABLE projectless_directories(task_id TEXT PRIMARY KEY, schema_version INTEGER NOT NULL CHECK(schema_version=1)); CREATE TABLE projectless_catalog_identity(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema_version INTEGER NOT NULL,owned_root TEXT NOT NULL);").map_err(|e| e.to_string())?;
        transaction
            .execute(
                "INSERT INTO projectless_catalog_identity VALUES(1,1,?1)",
                [root_text],
            )
            .map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(Self { connection, root })
    }
    pub fn open_existing(database: &Path, owned_root: &Path) -> Result<Self, String> {
        ordinary_directory(owned_root)?;
        let root = std::fs::canonicalize(owned_root).map_err(|e| e.to_string())?;
        let connection = rusqlite::Connection::open_with_flags(
            database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
        )
        .map_err(|e| e.to_string())?;
        validate_catalog(&connection, &root)?;
        Ok(Self { connection, root })
    }
    pub fn create_task(&self, task_id: &str) -> Result<PathBuf, String> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        validate_catalog(&transaction, &self.root)?;
        let directory = create(&self.root, task_id)?;
        // Failed registration retains files and reports failure; no implicit adopt.
        transaction
            .execute(
                "INSERT INTO projectless_directories(task_id,schema_version) VALUES(?1,1)",
                [task_id],
            )
            .map_err(|e| format!("registration failed; owned files retained: {e}"))?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(directory)
    }
    pub fn reopen_task(&self, task_id: &str) -> Result<PathBuf, String> {
        checked_id(task_id)?;
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        validate_catalog(&transaction, &self.root)?;
        let version: i64 = transaction
            .query_row(
                "SELECT schema_version FROM projectless_directories WHERE task_id=?1",
                [task_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("task directory is not registered: {e}"))?;
        if version != 1 {
            return Err("unsupported task directory schema".into());
        }
        let directory = open(&self.root, task_id)?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(directory)
    }
}
#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[test]
    fn session_binding_is_durable_insert_only_and_missing_data_never_falls_back() {
        let app_data =
            std::env::temp_dir().join(format!("agentz-session-binding-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&app_data).unwrap();
        let project = app_data.join("free-session-db");
        let other = app_data.join("another-project");
        std::fs::create_dir(&project).unwrap();
        std::fs::create_dir(&other).unwrap();
        let store = AppDataStore::initialize(&app_data).unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        assert!(store.reopen_session(&project, &session).is_err());
        assert!(!app_data
            .join("projectless/session-bindings.sqlite")
            .exists());
        let id = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        let directory = store.create_task(&id).unwrap();
        store.create_task(&second).unwrap();
        std::fs::write(directory.join("report.txt"), "research result").unwrap();
        assert_eq!(
            store.bind_session(&project, &session, &id).unwrap(),
            directory
        );
        assert_eq!(
            store.bind_session(&project, &session, &id).unwrap(),
            directory
        );
        assert!(store.bind_session(&project, &session, &second).is_err());
        assert!(store.bind_session(&other, &session, &id).is_err());
        drop(store);
        let store = AppDataStore::open_existing(&app_data).unwrap();
        assert_eq!(store.reopen_session(&project, &session).unwrap(), directory);
        assert!(store.reopen_session(&other, &session).is_err());
        assert_eq!(
            std::fs::read_to_string(directory.join("report.txt")).unwrap(),
            "research result"
        );
        std::fs::rename(&directory, app_data.join("retained-report")).unwrap();
        assert!(store.reopen_session(&project, &session).is_err());
        assert!(!directory.exists());
        assert!(app_data.join("retained-report/report.txt").exists());
        drop(store);
        assert!(app_data.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(app_data).unwrap();
    }
    #[test]
    fn session_directory_is_read_only_and_a_lost_binding_store_is_not_unbound() {
        let app_data =
            std::env::temp_dir().join(format!("agentz-session-lookup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&app_data).unwrap();
        let project = app_data.join("free-session-db");
        std::fs::create_dir(&project).unwrap();
        let store = AppDataStore::initialize(&app_data).unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        // Never initialized: the session is simply not projectless, and nothing is created.
        assert_eq!(store.session_directory(&project, &session).unwrap(), None);
        assert!(!app_data.join("projectless/session-bindings.sqlite").exists());

        let id = uuid::Uuid::new_v4().to_string();
        let directory = store.create_task(&id).unwrap();
        store.bind_session(&project, &session, &id).unwrap();
        assert_eq!(
            store.session_directory(&project, &session).unwrap(),
            Some(directory.clone())
        );
        let unbound = uuid::Uuid::new_v4().to_string();
        assert_eq!(store.session_directory(&project, &unbound).unwrap(), None);

        // After initialization, a missing binding store must refuse instead of reading as unbound.
        let database = app_data.join("projectless/session-bindings.sqlite");
        std::fs::rename(&database, app_data.join("retained-bindings.sqlite")).unwrap();
        assert!(store.session_directory(&project, &session).is_err());
        assert!(!database.exists());
        assert!(directory.exists());
        drop(store);
        assert!(app_data.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(app_data).unwrap();
    }

    #[test]
    fn session_binding_refuses_trigger_and_missing_binding_catalog() {
        let app_data =
            std::env::temp_dir().join(format!("agentz-binding-corrupt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&app_data).unwrap();
        let store = AppDataStore::initialize(&app_data).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let session = uuid::Uuid::new_v4().to_string();
        let directory = store.create_task(&id).unwrap();
        store.bind_session(&app_data, &session, &id).unwrap();
        let database = app_data.join("projectless/session-bindings.sqlite");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection.execute_batch("CREATE TRIGGER erase_binding AFTER INSERT ON projectless_session_bindings BEGIN DELETE FROM projectless_session_bindings; END;").unwrap();
        assert!(store.reopen_session(&app_data, &session).is_err());
        assert!(store.bind_session(&app_data, &session, &id).is_err());
        drop(connection);
        std::fs::rename(&database, app_data.join("retained-bindings.sqlite")).unwrap();
        assert!(store.reopen_session(&app_data, &session).is_err());
        assert!(!database.exists());
        assert!(store.bind_session(&app_data, &session, &id).is_err());
        assert!(!database.exists());
        assert!(directory.exists());
        drop(store);
        assert!(app_data.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(app_data).unwrap();
    }
    #[test]
    fn app_data_layout_preserves_reports_and_reopen_never_provisions() {
        let app_data =
            std::env::temp_dir().join(format!("agentz-appdata-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&app_data).unwrap();
        assert!(AppDataStore::open_existing(&app_data).is_err());
        assert!(!app_data.join("projectless").exists());
        let store = AppDataStore::initialize(&app_data).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let directory = store.create_task(&id).unwrap();
        assert_eq!(
            directory.parent().unwrap(),
            std::fs::canonicalize(app_data.join("projectless/workspaces")).unwrap()
        );
        std::fs::write(directory.join("report.txt"), "retained").unwrap();
        drop(store);
        let reopened = AppDataStore::open_existing(&app_data).unwrap();
        assert_eq!(reopened.reopen_task(&id).unwrap(), directory);
        assert_eq!(
            std::fs::read_to_string(directory.join("report.txt")).unwrap(),
            "retained"
        );
        assert!(!directory.join(".git").exists());
        drop(reopened);
        let database = app_data.join("projectless/catalog.sqlite");
        std::fs::rename(&database, app_data.join("retained.sqlite")).unwrap();
        assert!(AppDataStore::open_existing(&app_data).is_err());
        assert!(!database.exists());
        // A directory at the database location is never adopted as a catalog.
        std::fs::create_dir(&database).unwrap();
        assert!(AppDataStore::initialize(&app_data).is_err());
        assert!(directory.join("report.txt").exists());
        assert!(app_data.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(app_data).unwrap();
    }
    #[test]
    fn persistent_catalog_reopens_only_registered_tasks_without_recreating_data() {
        let root = std::env::temp_dir().join(format!(
            "agentz-projectless-catalog-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let db = root.join("catalog.sqlite");
        assert!(Catalog::open_existing(&db, &root).is_err());
        assert!(!db.exists());
        let id = uuid::Uuid::new_v4().to_string();
        let catalog = Catalog::initialize(&db, &root).unwrap();
        let directory = catalog.create_task(&id).unwrap();
        std::fs::write(directory.join("report.txt"), "keep").unwrap();
        drop(catalog);
        let other_root = root.join("another-owned-root");
        std::fs::create_dir(&other_root).unwrap();
        assert!(Catalog::open_existing(&db, &other_root).is_err());
        assert!(Catalog::initialize(&db, &other_root).is_err());
        let reopened = Catalog::open_existing(&db, &root).unwrap();
        assert_eq!(reopened.reopen_task(&id).unwrap(), directory);
        assert_eq!(
            std::fs::read_to_string(directory.join("report.txt")).unwrap(),
            "keep"
        );
        let orphan = uuid::Uuid::new_v4().to_string();
        create(&root, &orphan).unwrap();
        assert!(reopened.reopen_task(&orphan).is_err());
        std::fs::rename(&directory, root.join("retained-away")).unwrap();
        assert!(reopened.reopen_task(&id).is_err());
        assert!(!directory.exists());
        drop(reopened);
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod corruption_tests {
    use super::*;
    #[test]
    fn catalog_guards_reject_triggers_schema_and_layout_before_creating_files() {
        let root = std::env::temp_dir().join(format!(
            "agentz-projectless-corrupt-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let db = root.join("catalog.sqlite");
        let catalog = Catalog::initialize(&db, &root).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        catalog.connection.execute_batch("CREATE TRIGGER erase_registration AFTER INSERT ON projectless_directories BEGIN DELETE FROM projectless_directories WHERE task_id=NEW.task_id; END;").unwrap();
        assert!(catalog.create_task(&id).is_err());
        assert!(!root.join(&id).exists());
        assert!(Catalog::open_existing(&db, &root).is_err());
        catalog.connection.execute_batch("DROP TRIGGER erase_registration; UPDATE projectless_catalog_identity SET schema_version=2;").unwrap();
        assert!(catalog.create_task(&id).is_err());
        assert!(!root.join(&id).exists());
        assert!(Catalog::open_existing(&db, &root).is_err());
        catalog.connection.execute_batch("UPDATE projectless_catalog_identity SET schema_version=1; ALTER TABLE projectless_directories ADD COLUMN unexpected TEXT;").unwrap();
        assert!(catalog.create_task(&id).is_err());
        assert!(!root.join(&id).exists());
        assert!(Catalog::open_existing(&db, &root).is_err());
        drop(catalog);
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
