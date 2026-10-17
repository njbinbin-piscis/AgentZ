//! Local source mirror of a remote workspace, so the existing local indexes
//! (codebase / graph / symbols) work unchanged for remote projects.
//!
//! Sync is incremental: the remote keeps a stamp file and only files newer than
//! it are shipped (tar.gz over the target transport); a file listing prunes
//! deletions. Big/vendored trees are skipped, matching what indexing ignores.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;

use super::{fnv1a, shell_quote, vfs};

/// Re-syncs closer together than this reuse the mirror as-is.
const MIN_INTERVAL: Duration = Duration::from_secs(20);
const MAX_FILE_KB: u32 = 512;
const PRUNE: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    "out",
    ".venv",
    "venv",
    "__pycache__",
    ".agentz",
    ".next",
    ".cache",
    ".gradle",
    ".idea",
];

fn last_sync() -> &'static Mutex<HashMap<String, Instant>> {
    static M: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sync_locks() -> &'static Mutex<HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>> {
    static M: OnceLock<Mutex<HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> =
        OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key(workspace: &str) -> String {
    if let Some(uri) = vfs::parse(workspace) {
        let authority = uri.authority.to_ascii_lowercase();
        // Remote POSIX paths are case sensitive. Length-prefix the authority
        // so delimiters in decoded paths cannot produce ambiguous identities.
        format!(
            "v2:{}:{}{}",
            authority.len(),
            authority,
            uri.path.trim_end_matches('/')
        )
    } else {
        format!("v2:invalid:{workspace}")
    }
}

pub fn mirror_dir(workspace: &str) -> Option<PathBuf> {
    Some(
        super::dirs_home()?
            .join(".agentz")
            .join("remote-mirror")
            // Do not reuse legacy mirrors that folded the complete path case.
            .join(format!("v2-{}", fnv1a(key(workspace).as_bytes()))),
    )
}

fn find_expr(newer: Option<&str>) -> String {
    let prune = PRUNE
        .iter()
        .map(|n| format!("-name {}", shell_quote(n)))
        .collect::<Vec<_>>()
        .join(" -o ");
    let newer = newer
        .map(|s| format!("-newer {} ", shell_quote(s)))
        .unwrap_or_default();
    format!("find . \\( {prune} \\) -prune -o -type f -size -{MAX_FILE_KB}k {newer}-print | {GITIGNORE_FILTER}")
}

/// Drops `.gitignore`d paths inside a git work tree; passthrough elsewhere.
const GITIGNORE_FILTER: &str = "{ if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then \
     git check-ignore --stdin -nv --non-matching | awk -F'\\t' '$1==\"::\"{print $2}'; else cat; fi; }";

/// Bring the mirror up to date and return its local path.
pub async fn sync(workspace: &str, force: bool) -> Result<PathBuf, String> {
    let uri = vfs::parse(workspace).ok_or("not a remote workspace")?;
    let session = if let Some(session) = super::bound_session() {
        session
    } else {
        super::bind(workspace).await?
    };
    validate_session(&session, &uri.authority).await?;
    let k = key(workspace);
    let dir = mirror_dir(workspace).ok_or("no home directory")?;
    if !force {
        if let Some(t) = last_sync().lock().unwrap().get(&k) {
            if t.elapsed() < MIN_INTERVAL && dir.join(".agentz-mirror-ok").is_file() {
                return Ok(dir);
            }
        }
    }
    let lock = sync_locks()
        .lock()
        .unwrap()
        .entry(k.clone())
        .or_default()
        .clone();
    let _guard = lock.lock().await;

    validate_session(&session, &uri.authority).await?;
    let target = &session.target;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    let stamp = format!(
        "$HOME/.agentz-server/mirror-v2-{}.stamp",
        fnv1a(k.as_bytes())
    );
    let fresh = force || !dir.join(".agentz-mirror-ok").exists();
    // The remote advances its stamp before streaming. Invalidate locally before
    // starting so failure or cancellation forces a full snapshot on the next try.
    invalidate_completion(&dir)?;
    let script = sync_script(&uri.path, &stamp, fresh);
    let out = target
        .sh(&script)?
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("mirror transfer failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "mirror transfer failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    // This transfer has its own transport process. Reject results if the
    // original broker generation changed while it was in flight.
    validate_session(&session, &uri.authority).await?;
    if !out.stdout.is_empty() {
        extract(&dir, &out.stdout).await?;
    }

    prune_deleted(target, &uri.path, &dir).await?;
    validate_session(&session, &uri.authority).await?;
    std::fs::write(dir.join(".agentz-mirror-ok"), workspace)
        .map_err(|e| format!("record mirror completion: {e}"))?;
    last_sync().lock().unwrap().insert(k, Instant::now());
    Ok(dir)
}

async fn validate_session(session: &super::RemoteSession, authority: &str) -> Result<(), String> {
    if !session.authority.eq_ignore_ascii_case(authority) {
        return Err("mirror workspace authority differs from execution session".into());
    }
    let target = session
        .manager
        .target_for_generation(session.generation)
        .await?;
    if target != session.target {
        return Err("mirror target differs from execution session".into());
    }
    Ok(())
}

fn invalidate_completion(dir: &Path) -> Result<(), String> {
    match std::fs::remove_file(dir.join(".agentz-mirror-ok")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("invalidate mirror completion: {e}")),
    }
}

/// Emits a tar.gz of files changed since the last sync (all files when
/// `fresh`). The stamp is renewed *before* listing so edits made during the
/// transfer are picked up next time.
fn sync_script(root: &str, stamp: &str, fresh: bool) -> String {
    format!(
        "cd {root} || exit 1; mkdir -p \"$(dirname \"{stamp}\")\"; S=\"{stamp}\"; \
         if [ -f \"$S\" ] && [ {fresh} = 0 ]; then N=1; mv \"$S\" \"$S.prev\"; else N=''; fi; touch \"$S\"; \
         if [ -n \"$N\" ]; then {find_newer}; else {find_all}; fi | tar -czf - -T - 2>/dev/null",
        root = shell_quote(root),
        fresh = if fresh { 1 } else { 0 },
        find_all = find_expr(None),
        find_newer = find_expr(Some("$S.prev")).replace("'$S.prev'", "\"$S.prev\""),
    )
}

#[cfg(test)]
pub(crate) fn sync_script_for_test(root: &str, stamp: &str, fresh: bool) -> String {
    sync_script(root, stamp, fresh)
}

#[cfg(test)]
pub(crate) async fn extract_for_test(dir: &Path, archive: &[u8]) {
    extract(dir, archive).await.unwrap()
}

/// Root that local index code should read: the mirror for remote projects.
/// `sync_first` refreshes it (rebuilds); status queries just peek.
pub async fn index_root(project: &str, sync_first: bool) -> Result<PathBuf, String> {
    if !super::is_remote(project) {
        return Ok(PathBuf::from(project));
    }
    if sync_first {
        sync(project, false).await
    } else {
        mirror_dir(project).ok_or_else(|| "no home directory".to_string())
    }
}

async fn extract(dir: &Path, archive: &[u8]) -> Result<(), String> {
    let mut child = piscis_kernel::proc::tokio_command("tar")
        .arg("-xzf")
        .arg("-")
        .arg("-C")
        .arg(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("local tar not available: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    stdin.write_all(archive).await.map_err(|e| e.to_string())?;
    drop(stdin);
    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
    // Partial extraction, including unsupported local names, is not a complete
    // mirror. Keep the completion marker absent so recovery uses a full snapshot.
    if !out.status.success() {
        return Err(format!(
            "mirror extract failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

async fn prune_deleted(target: &super::RemoteTarget, root: &str, dir: &Path) -> Result<(), String> {
    let listing = target
        .run(&format!("cd {} && {}", shell_quote(root), find_expr(None)))
        .await?;
    let remote: HashSet<String> = listing
        .lines()
        .filter_map(|l| l.strip_prefix("./"))
        .map(str::to_string)
        .collect();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let rel = p
                .strip_prefix(dir)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            if rel == ".agentz" || rel == ".agentz-mirror-ok" {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if !remote.contains(&rel) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disconnected_session() -> super::super::RemoteSession {
        super::super::RemoteSession {
            target: super::super::RemoteTarget::Ssh { host: "a".into() },
            authority: "ssh-remote+a".into(),
            path: "/work".into(),
            generation: 0,
            manager: std::sync::Arc::new(crate::commands::ext_host::ExtHostManager::new()),
        }
    }

    #[tokio::test]
    async fn mirror_rejects_another_authority_before_transport_or_cache() {
        let session = disconnected_session();
        let error =
            super::super::with_session(session, sync("agentz-remote://ssh-remote+b/work", false))
                .await
                .unwrap_err();
        assert!(error.contains("authority differs"));
    }

    #[tokio::test]
    async fn mirror_rejects_disconnected_session_before_transport_or_cache() {
        let session = disconnected_session();
        let error =
            super::super::with_session(session, sync("agentz-remote://ssh-remote+a/work", false))
                .await
                .unwrap_err();
        assert!(error.contains("no target"));
    }

    #[test]
    fn failed_sync_invalidates_existing_completion_and_is_repeatable() {
        let dir =
            std::env::temp_dir().join(format!("agentz-mirror-marker-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join(".agentz-mirror-ok");
        std::fs::write(&marker, "previous successful workspace").unwrap();
        invalidate_completion(&dir).unwrap();
        assert!(
            !marker.exists(),
            "a failed/cancelled attempt must require fresh sync"
        );
        invalidate_completion(&dir).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn invalidation_error_prevents_remote_stamp_advance() {
        let dir =
            std::env::temp_dir().join(format!("agentz-mirror-marker-{}", uuid::Uuid::new_v4()));
        let marker = dir.join(".agentz-mirror-ok");
        std::fs::create_dir_all(&marker).unwrap();
        assert!(invalidate_completion(&dir).is_err());
        std::fs::remove_dir(&marker).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }

    #[tokio::test]
    async fn malformed_archive_is_not_a_successful_mirror() {
        let dir =
            std::env::temp_dir().join(format!("agentz-mirror-archive-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = extract(&dir, b"not a gzip archive").await;
        std::fs::remove_dir(&dir).unwrap();
        assert!(
            result.is_err(),
            "tar failure must not mark a partial mirror complete"
        );
    }

    #[test]
    fn find_expression_prunes_and_filters() {
        let e = find_expr(Some("/tmp/s"));
        assert!(e.contains("-name 'node_modules'"));
        assert!(e.contains("-size -512k -newer '/tmp/s' -print"));
        assert!(find_expr(None).contains("-size -512k -print | { if git rev-parse"));
    }

    /// `AGENTZ_DUMP_MIRROR_SCRIPT=<root>|<stamp>|<fresh>` prints the script so
    /// it can be exercised by a real shell.
    #[test]
    fn dump_sync_script() {
        if let Ok(spec) = std::env::var("AGENTZ_DUMP_MIRROR_SCRIPT") {
            let p: Vec<&str> = spec.split('|').collect();
            println!("SCRIPT>>{}<<", sync_script(p[0], p[1], p[2] == "1"));
        }
    }

    #[test]
    fn mirror_key_normalizes_only_authority_ascii_case() {
        assert_eq!(
            key("agentz-remote://ssh-remote+Box/p/"),
            key("agentz-remote://ssh-remote+box/p")
        );
        assert_eq!(
            key("AGENTZ-REMOTE://ssh-remote%2BBox/Work/"),
            key("agentz-remote://ssh-remote+box/Work")
        );
        assert_ne!(
            key("agentz-remote://ssh-remote+Ä/Work"),
            key("agentz-remote://ssh-remote+ä/Work")
        );
    }

    #[test]
    fn distinct_remote_path_case_has_isolated_cache_identity() {
        let upper = "agentz-remote://ssh-remote+box/Work";
        let lower = "agentz-remote://ssh-remote+box/work";
        assert_ne!(key(upper), key(lower));
        assert_ne!(fnv1a(key(upper).as_bytes()), fnv1a(key(lower).as_bytes()));
        if let (Some(upper_dir), Some(lower_dir)) = (mirror_dir(upper), mirror_dir(lower)) {
            assert_ne!(upper_dir, lower_dir);
            assert!(upper_dir
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("v2-"));
        }
    }

    #[test]
    fn legacy_colliding_mirror_namespace_is_not_reused() {
        let workspace = "agentz-remote://ssh-remote+Box/Work";
        let legacy = fnv1a(workspace.to_lowercase().as_bytes());
        if let Some(dir) = mirror_dir(workspace) {
            assert_ne!(dir.file_name().unwrap().to_string_lossy(), legacy);
        }
    }
}
