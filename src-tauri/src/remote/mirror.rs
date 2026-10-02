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
    "node_modules", ".git", "target", "dist", "build", "out", ".venv", "venv", "__pycache__",
    ".agentz", ".next", ".cache", ".gradle", ".idea",
];

fn last_sync() -> &'static Mutex<HashMap<String, Instant>> {
    static M: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sync_locks() -> &'static Mutex<HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>> {
    static M: OnceLock<Mutex<HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key(workspace: &str) -> String {
    workspace.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

pub fn mirror_dir(workspace: &str) -> Option<PathBuf> {
    Some(super::dirs_home()?.join(".agentz").join("remote-mirror").join(fnv1a(key(workspace).as_bytes())))
}

fn find_expr(newer: Option<&str>) -> String {
    let prune = PRUNE.iter().map(|n| format!("-name {}", shell_quote(n))).collect::<Vec<_>>().join(" -o ");
    let newer = newer.map(|s| format!("-newer {} ", shell_quote(s))).unwrap_or_default();
    format!("find . \\( {prune} \\) -prune -o -type f -size -{MAX_FILE_KB}k {newer}-print | {GITIGNORE_FILTER}")
}

/// Drops `.gitignore`d paths inside a git work tree; passthrough elsewhere.
const GITIGNORE_FILTER: &str = "{ if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then \
     git check-ignore --stdin -nv --non-matching | awk -F'\\t' '$1==\"::\"{print $2}'; else cat; fi; }";

/// Bring the mirror up to date and return its local path.
pub async fn sync(workspace: &str, force: bool) -> Result<PathBuf, String> {
    let k = key(workspace);
    let dir = mirror_dir(workspace).ok_or("no home directory")?;
    if !force {
        if let Some(t) = last_sync().lock().unwrap().get(&k) {
            if t.elapsed() < MIN_INTERVAL && dir.exists() {
                return Ok(dir);
            }
        }
    }
    let lock = sync_locks().lock().unwrap().entry(k.clone()).or_default().clone();
    let _guard = lock.lock().await;

    let uri = vfs::parse(workspace).ok_or("not a remote workspace")?;
    let mgr = super::manager().ok_or("remote broker unavailable")?;
    let target = mgr.wait_for_authority(&uri.authority).await?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    let stamp = format!("$HOME/.agentz-server/mirror-{}.stamp", fnv1a(k.as_bytes()));
    let fresh = force || !dir.join(".agentz-mirror-ok").exists();
    let script = sync_script(&uri.path, &stamp, fresh);
    let out = target
        .sh(&script)?
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("mirror transfer failed: {e}"))?;
    if !out.stdout.is_empty() {
        extract(&dir, &out.stdout).await?;
    }

    prune_deleted(&target, &uri.path, &dir).await?;
    let _ = std::fs::write(dir.join(".agentz-mirror-ok"), workspace);
    last_sync().lock().unwrap().insert(k, Instant::now());
    Ok(dir)
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
        .spawn()
        .map_err(|e| format!("local tar not available: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    stdin.write_all(archive).await.map_err(|e| e.to_string())?;
    drop(stdin);
    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
    // Names that are invalid on the local FS (e.g. `:` on Windows) make tar exit
    // non-zero; everything else is still extracted.
    if !out.status.success() {
        tracing::warn!("mirror extract: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

async fn prune_deleted(target: &super::RemoteTarget, root: &str, dir: &Path) -> Result<(), String> {
    let listing = target.run(&format!("cd {} && {}", shell_quote(root), find_expr(None))).await?;
    let remote: HashSet<String> = listing
        .lines()
        .filter_map(|l| l.strip_prefix("./"))
        .map(str::to_string)
        .collect();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            let rel = p.strip_prefix(dir).unwrap_or(&p).to_string_lossy().replace('\\', "/");
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
    fn mirror_key_is_case_insensitive() {
        assert_eq!(key("agentz-remote://ssh-remote+Box/p/"), key("agentz-remote://ssh-remote+box/p"));
    }
}
