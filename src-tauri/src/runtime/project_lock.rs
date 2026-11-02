//! One AgentZ instance per project. Opening a project takes an exclusive OS lock on
//! `{config_dir}/instances/<sha256(project)>.lock`; a second process sees it held and
//! leaves the project to its owner. The OS drops the lock when the process dies.

use super::os_lock;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
    sync::Mutex,
};

static HELD: Mutex<Option<HashMap<String, File>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Acquire {
    /// This process now owns the project.
    Acquired,
    /// This process already owned it.
    AlreadyHeld,
    /// Another live process owns it.
    HeldElsewhere { holder_pid: Option<u32> },
}

/// Stable identity for a local path or an `agentz-remote://` URI.
fn project_key(dir: &str) -> String {
    let dir = dir.trim();
    if crate::remote::is_remote(dir) {
        return dir.trim_end_matches('/').to_string();
    }
    let path = std::fs::canonicalize(dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| dir.to_string());
    let path = path.trim_end_matches(['/', '\\']).to_string();
    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path
    }
}

fn paths(config_dir: &Path, key: &str) -> (PathBuf, PathBuf) {
    let hash = hex::encode(Sha256::digest(key.as_bytes()));
    let base = config_dir.join("instances");
    (base.join(format!("{hash}.lock")), base.join(format!("{hash}.owner")))
}

pub fn acquire(config_dir: &Path, dir: &str) -> Result<Acquire, String> {
    let key = project_key(dir);
    let mut guard = HELD.lock().unwrap_or_else(|e| e.into_inner());
    let held = guard.get_or_insert_with(HashMap::new);
    if held.contains_key(&key) {
        return Ok(Acquire::AlreadyHeld);
    }
    let (lock_path, owner_path) = paths(config_dir, &key);
    match os_lock::try_lock(&lock_path)? {
        Some(file) => {
            // The locked byte range is unreadable to other processes on Windows, so the
            // owner pid lives in a sibling file.
            let _ = std::fs::write(&owner_path, std::process::id().to_string());
            held.insert(key, file);
            Ok(Acquire::Acquired)
        }
        None => Ok(Acquire::HeldElsewhere {
            holder_pid: std::fs::read_to_string(&owner_path)
                .ok()
                .and_then(|s| s.trim().parse().ok()),
        }),
    }
}

pub fn release(dir: &str) {
    let key = project_key(dir);
    let mut guard = HELD.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(held) = guard.as_mut() {
        held.remove(&key);
    }
}

/// True when this process owns `dir`.
pub fn holds(dir: &str) -> bool {
    let key = project_key(dir);
    HELD.lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|held| held.contains_key(&key))
}

/// True when another live process owns `dir`. Projects nobody locked (for example
/// directories used only by bots) are not held elsewhere.
pub fn held_elsewhere(config_dir: &Path, dir: &str) -> bool {
    if holds(dir) {
        return false;
    }
    let (lock_path, _) = paths(config_dir, &project_key(dir));
    os_lock::is_held(&lock_path).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_is_exclusive_until_release() {
        let root = std::env::temp_dir().join(format!("agentz-project-lock-{}", uuid::Uuid::new_v4()));
        let config = root.join("config");
        let project = root.join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let dir = project.to_string_lossy().to_string();

        assert_eq!(acquire(&config, &dir).unwrap(), Acquire::Acquired);
        assert_eq!(acquire(&config, &dir).unwrap(), Acquire::AlreadyHeld);
        assert!(holds(&dir));
        let (lock_path, _) = paths(&config, &project_key(&dir));
        assert!(os_lock::try_lock(&lock_path).unwrap().is_none(), "other handles are refused");

        release(&dir);
        assert!(!holds(&dir));
        assert!(os_lock::try_lock(&lock_path).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
