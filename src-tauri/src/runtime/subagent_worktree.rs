//! Isolated workspaces for writing sub-agents.
//!
//! Each writer runs in its own detached git worktree created from a snapshot of
//! the user's *current* working tree (uncommitted and untracked files included).
//! When it finishes, only what the sub-agent changed (snapshot diff) is merged
//! back file by file with a three-way merge against whatever the workspace looks
//! like at that moment, so edits the user or other writers made meanwhile are
//! kept. Nothing touches HEAD, the index, or the user's branch. Every run leaves
//! recovery refs under `refs/agentz/subagent/<run>/`:
//! `base` (what the sub-agent started from), `result` (what it produced) and
//! `premerge` (the workspace right before the merge, for rollback).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// Directories linked into the worktree so dependency installs are not repeated.
/// They are build inputs, never merged back.
const LINKED_DIRS: &[&str] = &["node_modules"];

static MERGE_LOCK: Mutex<()> = Mutex::new(());

/// Writers hold the read side (they are isolated and may run in parallel).
/// Anything that must edit the workspace in place takes the write side.
pub static WRITERS: tokio::sync::RwLock<()> = tokio::sync::RwLock::const_new(());

fn run_git(dir: &Path, args: &[&str], envs: &[(&str, &OsStr)]) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "AgentZ")
        .env("GIT_AUTHOR_EMAIL", "agentz@localhost")
        .env("GIT_COMMITTER_NAME", "AgentZ")
        .env("GIT_COMMITTER_EMAIL", "agentz@localhost");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| format!("git unavailable: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim().to_string()
}

/// Repository root containing `workspace`, if it is inside a git repo.
pub fn repo_root(workspace: &Path) -> Option<PathBuf> {
    let out = run_git(workspace, &["rev-parse", "--show-toplevel"], &[]).ok()?;
    let s = text(out);
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

/// Commit object describing the working tree right now (tracked, modified and
/// untracked-but-not-ignored files) without touching HEAD, branches or the index.
pub fn snapshot(dir: &Path, label: &str) -> Result<String, String> {
    let idx = std::env::temp_dir().join(format!("agentz-idx-{}", uuid::Uuid::new_v4()));
    let envs: [(&str, &OsStr); 1] = [("GIT_INDEX_FILE", idx.as_os_str())];
    let result = (|| {
        let head = run_git(dir, &["rev-parse", "--verify", "-q", "HEAD"], &[])
            .ok()
            .map(text)
            .filter(|h| !h.is_empty());
        if head.is_some() {
            run_git(dir, &["read-tree", "HEAD"], &envs)?;
        }
        run_git(dir, &["add", "-A"], &envs)?;
        let tree = text(run_git(dir, &["write-tree"], &envs)?);
        let mut args = vec!["commit-tree", tree.as_str()];
        if let Some(h) = head.as_deref() {
            args.extend(["-p", h]);
        }
        args.extend(["-m", label]);
        Ok(text(run_git(dir, &args, &[])?))
    })();
    let _ = std::fs::remove_file(&idx);
    result
}

fn normalize(p: &Path) -> String {
    p.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

pub struct Isolation {
    pub repo_root: PathBuf,
    pub worktree: PathBuf,
    pub base: String,
    pub run_id: String,
    linked: Vec<PathBuf>,
}

fn link_dir(target: &Path, link: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .creation_flags(0x0800_0000)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
}

/// Remove a link without ever following it into the real directory.
fn unlink_dir(link: &Path) {
    #[cfg(windows)]
    let _ = std::fs::remove_dir(link);
    #[cfg(not(windows))]
    let _ = std::fs::remove_file(link);
}

/// Create an isolated worktree for `run_id` under `wt_root`.
pub fn create(repo: &Path, run_id: &str, wt_root: &Path) -> Result<Isolation, String> {
    let base = snapshot(repo, "agentz sub-agent base")?;
    std::fs::create_dir_all(wt_root).map_err(|e| e.to_string())?;
    let wt = wt_root.join(run_id);
    run_git(
        repo,
        &["worktree", "add", "--detach", &wt.to_string_lossy(), &base],
        &[],
    )?;
    let _ = run_git(
        repo,
        &["update-ref", &format!("refs/agentz/subagent/{run_id}/base"), &base],
        &[],
    );
    let mut linked = Vec::new();
    for name in LINKED_DIRS {
        let src = repo.join(name);
        let dst = wt.join(name);
        if src.is_dir() && !dst.exists() && link_dir(&src, &dst) {
            linked.push(dst);
        }
    }
    Ok(Isolation {
        repo_root: repo.to_path_buf(),
        worktree: wt,
        base,
        run_id: run_id.to_string(),
        linked,
    })
}

/// Where `workspace` (inside the repo) lives inside the worktree.
pub fn workspace_in(iso: &Isolation, workspace: &Path) -> PathBuf {
    let root = normalize(&iso.repo_root);
    let ws = normalize(workspace);
    if ws == root {
        return iso.worktree.clone();
    }
    match ws.strip_prefix(&format!("{root}/")) {
        Some(rel) => {
            // Re-derive the relative part from the original (case preserved).
            let skip = root.len() + 1;
            let original = workspace.to_string_lossy().replace('\\', "/");
            let rel_orig = original.get(skip..).unwrap_or(rel);
            iso.worktree.join(rel_orig)
        }
        None => iso.worktree.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
    pub reason: String,
    /// Merged-with-markers copy saved for the parent to resolve, if any.
    pub saved_copy: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct MergeReport {
    /// Files written into the project, as `A path` / `M path` / `D path`.
    pub changed: Vec<String>,
    /// Subset of `changed` where the project file had also changed since the
    /// sub-agent started, so a three-way merge combined both sides.
    pub auto_merged: Vec<String>,
    /// Changes that needed no action (project already identical).
    pub skipped: usize,
    pub conflicts: Vec<Conflict>,
    pub run_id: String,
    pub merged: bool,
    /// 1-based order in which finished writers were merged in this app session.
    pub order: u64,
    pub base: String,
    pub premerge: String,
    pub note: String,
}

static MERGE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn short(h: &str) -> &str {
    h.get(..10).unwrap_or(h)
}

impl MergeReport {
    pub fn render(&self) -> String {
        let r = &self.run_id;
        let mut s = String::from("## Workspace effects (reported by the system)\n");
        s.push_str("- Mode: isolated git worktree started from a snapshot of the project's working tree (uncommitted and untracked files included).\n");
        if !self.base.is_empty() {
            s.push_str(&format!(
                "- Start snapshot: refs/agentz/subagent/{r}/base ({})\n",
                short(&self.base)
            ));
        }
        if !self.premerge.is_empty() {
            s.push_str(&format!(
                "- Checkpoint of the project taken right before merging: refs/agentz/subagent/{r}/premerge ({})\n",
                short(&self.premerge)
            ));
        }
        if !self.note.is_empty() {
            s.push_str(&format!("- {}\n", self.note));
        }
        if self.merged {
            s.push_str(&format!(
                "- Merge: this was merge #{} among finished writers. Only this sub-agent's own changes (start snapshot -> result) were applied to the live project with a per-file three-way merge, so edits made by you, the user or earlier-merged writers are kept. HEAD, branches and the index were not touched. Changes are uncommitted.\n",
                self.order
            ));
            s.push_str(&format!(
                "- Applied {} file(s), {} already identical, {} conflict(s).\n",
                self.changed.len(),
                self.skipped,
                self.conflicts.len()
            ));
            for p in &self.changed {
                s.push_str(&format!("  - {p}\n"));
            }
            if !self.auto_merged.is_empty() {
                s.push_str("- Auto-merged with concurrent edits (verify these still make sense together):\n");
                for p in &self.auto_merged {
                    s.push_str(&format!("  - {p}\n"));
                }
            }
        }
        if !self.conflicts.is_empty() {
            s.push_str("- CONFLICTS - NOT applied, the project file was left untouched. Resolve each yourself:\n");
            for c in &self.conflicts {
                s.push_str(&format!("  - {}: {}", c.path, c.reason));
                match &c.saved_copy {
                    Some(copy) => s.push_str(&format!(
                        " | merged copy with <<<<<<< markers: {copy} | sub-agent version: `git show refs/agentz/subagent/{r}/result:{}`\n",
                        c.path
                    )),
                    None => s.push_str(&format!(
                        " | sub-agent version: `git show refs/agentz/subagent/{r}/result:{}`\n",
                        c.path
                    )),
                }
            }
        }
        if self.merged {
            s.push_str(&format!(
                "- Inspect: `git diff refs/agentz/subagent/{r}/premerge` shows everything this merge changed. Roll back one file: `git checkout refs/agentz/subagent/{r}/premerge -- <path>`.\n"
            ));
        } else {
            s.push_str(&format!(
                "- Nothing was applied to the project. The sub-agent's work is at refs/agentz/subagent/{r}/result.\n"
            ));
        }
        s
    }
}

/// Report for a writer that edited the project in place (no worktree).
pub fn render_in_place(run_id: &str, checkpoint: Option<&str>, reason: &str) -> String {
    let mut s = String::from("## Workspace effects (reported by the system)\n");
    s.push_str(&format!(
        "- Mode: edited the project IN PLACE, exclusive (no other writer ran at the same time); {reason}. No merge step: its changes are already in the working tree, uncommitted.\n"
    ));
    match checkpoint {
        Some(h) => s.push_str(&format!(
            "- Checkpoint taken before the run: refs/agentz/subagent/{run_id}/premerge ({}). See what it changed: `git diff refs/agentz/subagent/{run_id}/premerge`; roll back a file: `git checkout refs/agentz/subagent/{run_id}/premerge -- <path>`.\n",
            short(h)
        )),
        None => s.push_str("- No git checkpoint was possible (not a git repository), so changes cannot be rolled back with git.\n"),
    }
    s
}

fn show(repo: &Path, commit: &str, path: &str) -> Option<Vec<u8>> {
    run_git(repo, &["show", &format!("{commit}:{path}")], &[]).ok()
}

fn is_binary(b: &[u8]) -> bool {
    b.iter().take(8000).any(|c| *c == 0)
}

fn safe_rel(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(':')
        && !path.split('/').any(|s| s == ".." || s.is_empty())
}

/// Three-way merge of one file; `Ok(merged)` or `Err(conflict markers copy)`.
fn merge3(ours: &[u8], base: &[u8], theirs: &[u8]) -> Result<Vec<u8>, Option<Vec<u8>>> {
    if is_binary(ours) || is_binary(base) || is_binary(theirs) {
        return Err(None);
    }
    let dir = std::env::temp_dir().join(format!("agentz-m3-{}", uuid::Uuid::new_v4()));
    if std::fs::create_dir_all(&dir).is_err() {
        return Err(None);
    }
    let (o, b, t) = (dir.join("ours"), dir.join("base"), dir.join("theirs"));
    let ok = std::fs::write(&o, ours).is_ok()
        && std::fs::write(&b, base).is_ok()
        && std::fs::write(&t, theirs).is_ok();
    let out = if ok {
        let mut cmd = Command::new("git");
        cmd.args(["merge-file", "-p", "-L", "project", "-L", "base", "-L", "sub-agent"])
            .arg(&o)
            .arg(&b)
            .arg(&t);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        cmd.output().ok()
    } else {
        None
    };
    let _ = std::fs::remove_dir_all(&dir);
    match out {
        Some(o) if o.status.code() == Some(0) => Ok(o.stdout),
        Some(o) if o.status.code().map_or(false, |c| c > 0) => Err(Some(o.stdout)),
        _ => Err(None),
    }
}

enum Applied {
    Done,
    Merged,
    Skipped,
    Conflict(String, Option<Vec<u8>>),
}

fn apply_change(repo: &Path, base: &str, result: &str, status: &str, path: &str) -> Applied {
    let target = repo.join(path);
    let ours = std::fs::read(&target).ok();
    let base_c = show(repo, base, path);
    let theirs = show(repo, result, path);
    let write = |bytes: &[u8]| -> Applied {
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&target, bytes) {
            Ok(_) => Applied::Done,
            Err(e) => Applied::Conflict(format!("could not write: {e}"), None),
        }
    };
    match status {
        "D" => match (ours, base_c) {
            (None, _) => Applied::Skipped,
            (Some(o), Some(b)) if o == b => match std::fs::remove_file(&target) {
                Ok(_) => Applied::Done,
                Err(e) => Applied::Conflict(format!("could not delete: {e}"), None),
            },
            _ => Applied::Conflict("sub-agent deleted it but the project copy changed".into(), None),
        },
        "A" => match (ours, theirs) {
            (_, None) => Applied::Skipped,
            (None, Some(t)) => write(&t),
            (Some(o), Some(t)) if o == t => Applied::Skipped,
            (Some(_), Some(_)) => {
                Applied::Conflict("both the project and the sub-agent created it".into(), None)
            }
        },
        _ => match (ours, base_c, theirs) {
            (_, _, None) => Applied::Skipped,
            (None, _, Some(_)) => {
                Applied::Conflict("sub-agent modified it but the project copy was deleted".into(), None)
            }
            (Some(o), Some(b), Some(t)) => {
                if o == t {
                    Applied::Skipped
                } else if o == b {
                    write(&t)
                } else {
                    match merge3(&o, &b, &t) {
                        Ok(m) => match write(&m) {
                            Applied::Done => Applied::Merged,
                            other => other,
                        },
                        Err(copy) => Applied::Conflict(
                            "overlapping edits in the project and the sub-agent".into(),
                            copy,
                        ),
                    }
                }
            }
            (Some(o), None, Some(t)) => {
                if o == t {
                    Applied::Skipped
                } else {
                    Applied::Conflict("no common base for this file".into(), None)
                }
            }
        },
    }
}

fn is_linked(path: &str) -> bool {
    LINKED_DIRS
        .iter()
        .any(|d| path == *d || path.starts_with(&format!("{d}/")))
}

/// Finish an isolated run: snapshot the result, optionally merge it back, and
/// always remove the worktree. `merge = false` keeps only the recovery refs.
pub fn finish(iso: Isolation, merge: bool, conflict_dir: &Path) -> MergeReport {
    let mut report = MergeReport {
        run_id: iso.run_id.clone(),
        ..Default::default()
    };
    for l in &iso.linked {
        unlink_dir(l);
    }
    let result = match snapshot(&iso.worktree, "agentz sub-agent result") {
        Ok(r) => r,
        Err(e) => {
            report.note = format!("Could not snapshot the sub-agent's work: {e}");
            cleanup(&iso);
            return report;
        }
    };
    let _ = run_git(
        &iso.repo_root,
        &[
            "update-ref",
            &format!("refs/agentz/subagent/{}/result", iso.run_id),
            &result,
        ],
        &[],
    );
    if !merge {
        report.note = "The run did not complete; its work was kept only under the recovery refs, not merged.".into();
        cleanup(&iso);
        return report;
    }

    let _guard = MERGE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    report.order = MERGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    report.base = iso.base.clone();
    let diff = run_git(
        &iso.repo_root,
        &["diff", "--name-status", "-z", "--no-renames", &iso.base, &result],
        &[],
    );
    let changes: Vec<(String, String)> = match diff {
        Ok(bytes) => {
            let parts: Vec<String> = bytes
                .split(|b| *b == 0)
                .map(|p| String::from_utf8_lossy(p).to_string())
                .collect();
            parts
                .chunks(2)
                .filter(|c| c.len() == 2 && !c[0].is_empty())
                .map(|c| (c[0].chars().take(1).collect::<String>(), c[1].clone()))
                .filter(|(_, p)| safe_rel(p) && !is_linked(p))
                .collect()
        }
        Err(e) => {
            report.note = format!("Could not diff the sub-agent's work: {e}");
            cleanup(&iso);
            return report;
        }
    };

    if let Ok(pre) = snapshot(&iso.repo_root, "agentz pre-merge checkpoint") {
        report.premerge = pre.clone();
        let _ = run_git(
            &iso.repo_root,
            &[
                "update-ref",
                &format!("refs/agentz/subagent/{}/premerge", iso.run_id),
                &pre,
            ],
            &[],
        );
    }

    for (status, path) in changes {
        match apply_change(&iso.repo_root, &iso.base, &result, &status, &path) {
            Applied::Done => report.changed.push(format!("{status} {path}")),
            Applied::Merged => {
                report.auto_merged.push(path.clone());
                report.changed.push(format!("{status} {path}"));
            }
            Applied::Skipped => report.skipped += 1,
            Applied::Conflict(reason, copy) => {
                let saved = copy.and_then(|bytes| {
                    let p = conflict_dir.join(&iso.run_id).join(&path);
                    p.parent().map(|d| std::fs::create_dir_all(d));
                    std::fs::write(&p, bytes).ok().map(|_| p.to_string_lossy().to_string())
                });
                report.conflicts.push(Conflict {
                    path,
                    reason,
                    saved_copy: saved,
                });
            }
        }
    }
    report.merged = true;
    cleanup(&iso);
    report
}

fn cleanup(iso: &Isolation) {
    for l in &iso.linked {
        unlink_dir(l);
    }
    let _ = run_git(
        &iso.repo_root,
        &["worktree", "remove", "--force", &iso.worktree.to_string_lossy()],
        &[],
    );
    if iso.worktree.exists() {
        let _ = std::fs::remove_dir_all(&iso.worktree);
    }
    let _ = run_git(&iso.repo_root, &["worktree", "prune"], &[]);
}

/// Checkpoint the project in place before a writer runs without isolation.
pub fn checkpoint_in_place(repo: &Path, run_id: &str) -> Option<String> {
    let snap = snapshot(repo, "agentz pre-run checkpoint").ok()?;
    run_git(
        repo,
        &["update-ref", &format!("refs/agentz/subagent/{run_id}/premerge"), &snap],
        &[],
    )
    .ok()?;
    Some(snap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_ok() -> bool {
        Command::new("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    struct Repo {
        dir: PathBuf,
        wt_root: PathBuf,
    }

    impl Repo {
        fn new(files: &[(&str, &str)]) -> Repo {
            let base = std::env::temp_dir().join(format!("agentz-wt-test-{}", uuid::Uuid::new_v4()));
            let dir = base.join("repo");
            std::fs::create_dir_all(&dir).unwrap();
            run_git(&dir, &["init", "-q"], &[]).unwrap();
            run_git(&dir, &["config", "user.email", "t@t"], &[]).unwrap();
            run_git(&dir, &["config", "user.name", "t"], &[]).unwrap();
            for (p, c) in files {
                let f = dir.join(p);
                std::fs::create_dir_all(f.parent().unwrap()).unwrap();
                std::fs::write(f, c).unwrap();
            }
            run_git(&dir, &["add", "-A"], &[]).unwrap();
            run_git(&dir, &["commit", "-q", "-m", "init"], &[]).unwrap();
            Repo {
                wt_root: base.join("wts"),
                dir,
            }
        }
        fn read(&self, p: &str) -> String {
            std::fs::read_to_string(self.dir.join(p)).unwrap_or_default()
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.dir.parent().unwrap());
        }
    }

    fn lines(n: usize) -> String {
        (1..=n).map(|i| format!("line{i}\n")).collect()
    }

    #[test]
    fn isolated_edits_merge_back_without_touching_head() {
        if !git_ok() {
            return;
        }
        let r = Repo::new(&[("src/a.txt", "one\n")]);
        let head_before = text(run_git(&r.dir, &["rev-parse", "HEAD"], &[]).unwrap());
        let iso = create(&r.dir, "run-a", &r.wt_root).unwrap();
        std::fs::write(iso.worktree.join("src/a.txt"), "one\ntwo\n").unwrap();
        std::fs::create_dir_all(iso.worktree.join("tests")).unwrap();
        std::fs::write(iso.worktree.join("tests/new.rs"), "test\n").unwrap();
        let rep = finish(iso, true, &r.dir.join(".agentz/conflicts"));
        assert!(rep.conflicts.is_empty(), "{}", rep.render());
        assert_eq!(r.read("src/a.txt"), "one\ntwo\n");
        assert_eq!(r.read("tests/new.rs"), "test\n");
        assert_eq!(head_before, text(run_git(&r.dir, &["rev-parse", "HEAD"], &[]).unwrap()));
        assert!(!r.wt_root.join("run-a").exists(), "worktree removed");
        for suffix in ["base", "result", "premerge"] {
            run_git(&r.dir, &["rev-parse", &format!("refs/agentz/subagent/run-a/{suffix}")], &[])
                .unwrap_or_else(|e| panic!("missing {suffix} ref: {e}"));
        }
    }

    #[test]
    fn uncommitted_user_changes_are_visible_to_the_sub_agent_and_survive() {
        if !git_ok() {
            return;
        }
        let r = Repo::new(&[("a.txt", &lines(10))]);
        // User edits line 1 without committing.
        std::fs::write(r.dir.join("a.txt"), lines(10).replacen("line1\n", "USER\n", 1)).unwrap();
        let iso = create(&r.dir, "run-b", &r.wt_root).unwrap();
        let in_wt = std::fs::read_to_string(iso.worktree.join("a.txt"))
            .unwrap()
            .replace("\r\n", "\n");
        assert!(in_wt.starts_with("USER\n"), "worktree starts from the dirty tree");
        // Sub-agent edits line 10.
        std::fs::write(iso.worktree.join("a.txt"), in_wt.replacen("line10\n", "SUB\n", 1)).unwrap();
        // Meanwhile the user also edits line 5 in place.
        std::fs::write(r.dir.join("a.txt"), lines(10).replacen("line1\n", "USER\n", 1).replacen("line5\n", "USER5\n", 1)).unwrap();
        let rep = finish(iso, true, &r.dir.join(".agentz/conflicts"));
        assert!(rep.conflicts.is_empty(), "{}", rep.render());
        let merged = r.read("a.txt");
        assert!(merged.contains("USER\n") && merged.contains("USER5\n") && merged.contains("SUB\n"), "{merged}");
    }

    #[test]
    fn overlapping_edits_leave_project_file_untouched_and_save_a_merged_copy() {
        if !git_ok() {
            return;
        }
        let r = Repo::new(&[("a.txt", &lines(5))]);
        let iso = create(&r.dir, "run-c", &r.wt_root).unwrap();
        std::fs::write(iso.worktree.join("a.txt"), lines(5).replacen("line3\n", "SUB\n", 1)).unwrap();
        let user = lines(5).replacen("line3\n", "USER\n", 1);
        std::fs::write(r.dir.join("a.txt"), &user).unwrap();
        let conflicts = r.dir.join(".agentz/conflicts");
        let rep = finish(iso, true, &conflicts);
        assert_eq!(rep.conflicts.len(), 1, "{}", rep.render());
        assert_eq!(r.read("a.txt"), user, "project file untouched");
        let saved = rep.conflicts[0].saved_copy.clone().expect("merged copy saved");
        let copy = std::fs::read_to_string(saved).unwrap();
        assert!(copy.contains("<<<<<<<") && copy.contains("USER") && copy.contains("SUB"));
    }

    #[test]
    fn parallel_writers_on_different_files_both_land() {
        if !git_ok() {
            return;
        }
        let r = Repo::new(&[("a.txt", "a\n"), ("b.txt", "b\n")]);
        let i1 = create(&r.dir, "run-d1", &r.wt_root).unwrap();
        let i2 = create(&r.dir, "run-d2", &r.wt_root).unwrap();
        std::fs::write(i1.worktree.join("a.txt"), "a1\n").unwrap();
        std::fs::write(i2.worktree.join("b.txt"), "b2\n").unwrap();
        let c = r.dir.join(".agentz/conflicts");
        assert!(finish(i1, true, &c).conflicts.is_empty());
        assert!(finish(i2, true, &c).conflicts.is_empty());
        assert_eq!((r.read("a.txt"), r.read("b.txt")), ("a1\n".into(), "b2\n".into()));
    }

    #[test]
    fn parallel_writers_on_the_same_file_merge_when_regions_differ() {
        if !git_ok() {
            return;
        }
        let r = Repo::new(&[("a.txt", &lines(12))]);
        let i1 = create(&r.dir, "run-e1", &r.wt_root).unwrap();
        let i2 = create(&r.dir, "run-e2", &r.wt_root).unwrap();
        std::fs::write(i1.worktree.join("a.txt"), lines(12).replacen("line1\n", "ONE\n", 1)).unwrap();
        std::fs::write(i2.worktree.join("a.txt"), lines(12).replacen("line12\n", "TWELVE\n", 1)).unwrap();
        let c = r.dir.join(".agentz/conflicts");
        assert!(finish(i1, true, &c).conflicts.is_empty());
        let rep = finish(i2, true, &c);
        assert!(rep.conflicts.is_empty(), "{}", rep.render());
        let merged = r.read("a.txt");
        assert!(merged.contains("ONE\n") && merged.contains("TWELVE\n"));
    }

    #[test]
    fn unmerged_run_keeps_only_recovery_refs() {
        if !git_ok() {
            return;
        }
        let r = Repo::new(&[("a.txt", "a\n")]);
        let iso = create(&r.dir, "run-f", &r.wt_root).unwrap();
        std::fs::write(iso.worktree.join("a.txt"), "changed\n").unwrap();
        let rep = finish(iso, false, &r.dir.join(".agentz/conflicts"));
        assert!(!rep.merged);
        assert_eq!(r.read("a.txt"), "a\n");
        run_git(&r.dir, &["rev-parse", "refs/agentz/subagent/run-f/result"], &[]).unwrap();
    }

    #[test]
    fn workspace_subdirectory_maps_into_the_worktree() {
        if !git_ok() {
            return;
        }
        let r = Repo::new(&[("pkg/x.txt", "x\n")]);
        let iso = create(&r.dir, "run-g", &r.wt_root).unwrap();
        let mapped = workspace_in(&iso, &r.dir.join("pkg"));
        assert!(mapped.ends_with("pkg"));
        assert!(mapped.starts_with(&iso.worktree));
        assert_eq!(workspace_in(&iso, &r.dir), iso.worktree);
        finish(iso, false, &r.dir.join(".agentz/conflicts"));
    }

    #[test]
    fn report_tells_the_parent_everything_it_needs() {
        if !git_ok() {
            return;
        }
        let r = Repo::new(&[("a.txt", &lines(10)), ("b.txt", "b\n")]);
        let iso = create(&r.dir, "run-h", &r.wt_root).unwrap();
        std::fs::write(iso.worktree.join("a.txt"), lines(10).replacen("line10\n", "SUB\n", 1)).unwrap();
        std::fs::write(iso.worktree.join("b.txt"), "b2\n").unwrap();
        std::fs::write(iso.worktree.join("c.txt"), "new\n").unwrap();
        std::fs::write(r.dir.join("a.txt"), lines(10).replacen("line1\n", "USER\n", 1)).unwrap();
        std::fs::write(r.dir.join("b.txt"), "user-b\n").unwrap();
        let text = finish(iso, true, &r.dir.join(".agentz/conflicts")).render();
        assert!(text.contains("refs/agentz/subagent/run-h/premerge"), "{text}");
        assert!(text.contains("merge #"), "{text}");
        assert!(text.contains("A c.txt"), "{text}");
        assert!(text.contains("Auto-merged") && text.contains("a.txt"), "{text}");
        assert!(text.contains("CONFLICTS") && text.contains("b.txt"), "{text}");
        assert!(text.contains("git show refs/agentz/subagent/run-h/result:b.txt"), "{text}");
        let direct = render_in_place("run-i", Some("0123456789abcdef"), "x");
        assert!(direct.contains("IN PLACE") && direct.contains("run-i/premerge"));
        assert!(render_in_place("run-j", None, "x").contains("not a git repository"));
    }

    #[test]
    fn rel_path_safety() {
        assert!(safe_rel("src/a.rs"));
        assert!(!safe_rel("../x"));
        assert!(!safe_rel("/etc/passwd"));
        assert!(!safe_rel("C:/x"));
        assert!(!safe_rel(""));
        assert!(is_linked("node_modules/x/y.js"));
        assert!(!is_linked("node_modules_extra/y.js"));
    }

    #[test]
    fn repo_root_is_none_outside_git() {
        let d = std::env::temp_dir().join(format!("agentz-nogit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        // A temp dir is almost never inside a repo; only assert the helper does not panic.
        let _ = repo_root(&d);
        let _ = std::fs::remove_dir_all(&d);
    }
}
