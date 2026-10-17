//! Explicit start selection. Never initialize a repository or commit its index.
use crate::archive::{capture_source, git, materialize, string};
use serde::Serialize;
use std::{fs, path::Path};

#[derive(Debug, Serialize)]
pub struct Start {
    pub schema_version: u32,
    pub requested_ref: String,
    pub mode: String,
    pub base_sha: String,
    pub start_sha: String,
    pub base_ref: String,
    pub omitted_paths: Vec<String>,
}

/// Commit mode requires explicit acknowledgement before excluding local edits.
/// Working mode captures final source bytes, with HEAD as its sole supported base.
pub struct Options<'a> {
    pub base: Option<&'a str>,
    pub mode: &'a str,
    pub allow_local_exclusion: bool,
    pub nonce: &'a str,
}

pub fn create(
    project: &Path,
    destination: &Path,
    branch: &str,
    options: Options<'_>,
) -> Result<Start, String> {
    let Options {
        base,
        mode,
        allow_local_exclusion,
        nonce,
    } = options;
    if !destination.is_absolute() || destination.exists() {
        return Err("task destination must be new and absolute; existing checkout requires explicit recovery".into());
    }
    if !project.join(".git").exists() {
        return Err(
            "TASK_START_NOT_GIT: use a local task, or initialize Git explicitly before isolation"
                .into(),
        );
    }
    let head = string(
        git(
            project,
            None,
            &["rev-parse", "--verify", "HEAD^{commit}"],
            None,
        )
        .map_err(|_| {
            "TASK_START_NO_COMMIT: create an initial commit explicitly, or use a local task"
        })?,
    )?;
    if mode != "commit" && mode != "working" {
        return Err("invalid task start mode".into());
    }
    let base = base
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("HEAD")
        .trim();
    if base.starts_with('-') {
        return Err("invalid start reference".into());
    }
    let base_sha = string(git(
        project,
        None,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{base}^{{commit}}"),
        ],
        None,
    )?)?;
    if mode == "working" && base_sha != head {
        return Err("local source capture requires the current HEAD as base".into());
    }
    let dirty = !git(
        project,
        None,
        &["status", "--porcelain", "-z", "--untracked-files=all"],
        None,
    )?
    .is_empty();
    if mode == "commit" && dirty && !allow_local_exclusion {
        return Err(
            "TASK_START_LOCAL_CHANGES: choose working source or explicitly exclude local changes"
                .into(),
        );
    }
    // A commit SHA is a comparison point, never a branch to check out for merge.
    let base_ref = if base == "HEAD"
        || git(
            project,
            None,
            &["show-ref", "--verify", &format!("refs/heads/{base}")],
            None,
        )
        .is_err()
    {
        string(
            git(
                project,
                None,
                &["symbolic-ref", "--quiet", "--short", "HEAD"],
                None,
            )
            .unwrap_or_else(|_| b"HEAD".to_vec()),
        )?
    } else {
        base.into()
    };
    let mut start = Start {
        schema_version: 1,
        requested_ref: base.into(),
        mode: mode.into(),
        base_sha: base_sha.clone(),
        start_sha: base_sha,
        base_ref,
        omitted_paths: Vec::new(),
    };
    if mode == "working" {
        if nonce.is_empty()
            || !nonce
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("invalid capture identity".into());
        }
        let index = std::env::temp_dir().join(format!("agentz-start-{nonce}.index"));
        if index.exists() || index.with_extension("index.lock").exists() {
            return Err("capture index exists; retain for diagnosis".into());
        }
        let captured = capture_source(project, &head, &index);
        // This nonce belongs to this operation; a preexisting index was rejected above.
        let _ = fs::remove_file(&index);
        let (tree, omitted) = captured?;
        start.omitted_paths = omitted;
        start.start_sha = string(git(
            project,
            None,
            &["commit-tree", &tree, "-p", &head],
            Some(b"AgentZ explicit local source start\n"),
        )?)?;
    }
    // Preserve the chosen object across a failed checkout; no source index or branch moves.
    let reference = format!("refs/agentz/starts/{nonce}");
    if nonce.is_empty()
        || !nonce
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err("invalid start identity".into());
    }
    git(
        project,
        None,
        &[
            "update-ref",
            &reference,
            &start.start_sha,
            &"0".repeat(start.start_sha.len()),
        ],
        None,
    )?;
    let receipts = project.join(".agentz/start-snapshots");
    fs::create_dir_all(&receipts)
        .map_err(|e| format!("start retained at {reference}; metadata directory failed: {e}"))?;
    let receipt = receipts.join(format!("{nonce}.json"));
    let mut out = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(receipt)
        .map_err(|e| format!("start retained at {reference}; metadata failed: {e}"))?;
    use std::io::Write;
    out.write_all(&serde_json::to_vec_pretty(&start).map_err(|e| e.to_string())?)
        .map_err(|e| format!("start retained at {reference}; metadata failed: {e}"))?;
    out.sync_all()
        .map_err(|e| format!("start retained at {reference}; metadata sync failed: {e}"))?;
    drop(out);
    fs::create_dir_all(destination.parent().ok_or("invalid task destination")?)
        .map_err(|e| e.to_string())?;
    materialize(project, &start.start_sha, destination, Some(branch))
        .map_err(|e| format!("task start retained at {reference}: {e}"))?;
    Ok(start)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!("agentz-start-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        Fixture(root)
    }
    fn run(root: &Path, args: &[&str]) -> String {
        string(git(root, None, args, None).unwrap()).unwrap()
    }
    fn options(mode: &'static str, ack: bool, nonce: &'static str) -> Options<'static> {
        Options {
            base: None,
            mode,
            allow_local_exclusion: ack,
            nonce,
        }
    }
    #[test]
    fn refuses_non_git_and_unborn_without_initializing_or_committing_staged_files() {
        let fixture = fixture();
        let root = fixture.0.join("project");
        fs::create_dir(&root).unwrap();
        let target = fixture.0.join("task");
        fs::write(root.join("private.txt"), b"private").unwrap();
        assert!(create(
            &root,
            &target,
            "workz/task-a",
            options("working", false, "non-git")
        )
        .unwrap_err()
        .contains("TASK_START_NOT_GIT"));
        assert!(!root.join(".git").exists());
        assert!(!target.exists());
        run(&root, &["init", "-b", "main"]);
        run(&root, &["add", "private.txt"]);
        let index = fs::read(root.join(".git/index")).unwrap();
        assert!(create(
            &root,
            &target,
            "workz/task-a",
            options("working", false, "unborn")
        )
        .unwrap_err()
        .contains("TASK_START_NO_COMMIT"));
        assert_eq!(index, fs::read(root.join(".git/index")).unwrap());
        assert!(git(&root, None, &["rev-parse", "--verify", "HEAD"], None).is_err());
        assert!(!target.exists());
    }
    #[test]
    fn explicit_commit_and_local_source_preserve_original_head_index_and_bytes() {
        let fixture = fixture();
        let root = fixture.0.join("project");
        fs::create_dir(&root).unwrap();
        run(&root, &["init", "-b", "main"]);
        run(&root, &["config", "core.autocrlf", "false"]);
        fs::write(root.join("file.txt"), b"base\n").unwrap();
        fs::write(root.join("deleted.txt"), b"delete me").unwrap();
        fs::write(root.join(".gitignore"), b"ignored.txt\n.agentz/\n").unwrap();
        run(&root, &["add", "."]);
        run(&root, &["commit", "-m", "base"]);
        let older = run(&root, &["rev-parse", "HEAD"]);
        fs::write(root.join("later.txt"), b"later commit").unwrap();
        run(&root, &["add", "later.txt"]);
        run(&root, &["commit", "-m", "later"]);
        let head = run(&root, &["rev-parse", "HEAD"]);
        fs::write(root.join("file.txt"), b"staged\n").unwrap();
        run(&root, &["add", "file.txt"]);
        fs::write(root.join("file.txt"), b"unstaged\r\n").unwrap();
        run(&root, &["rm", "deleted.txt"]);
        fs::write(root.join("新 file.bin"), [0, 255, 9]).unwrap();
        fs::write(root.join("ignored.txt"), b"ignored secret").unwrap();
        fs::create_dir(root.join(".agentz")).unwrap();
        fs::write(root.join(".agentz/journal.db"), b"runtime").unwrap();
        let index = fs::read(root.join(".git/index")).unwrap();
        let commit_target = fixture.0.join("commit-task");
        assert!(create(
            &root,
            &commit_target,
            "workz/task-c",
            options("commit", false, "unacknowledged")
        )
        .unwrap_err()
        .contains("TASK_START_LOCAL_CHANGES"));
        assert!(!commit_target.exists());
        let selected = create(
            &root,
            &commit_target,
            "workz/task-c",
            options("commit", true, "commit"),
        )
        .unwrap();
        assert_eq!(selected.base_sha, head);
        assert_eq!(selected.start_sha, head);
        assert_eq!(fs::read(commit_target.join("file.txt")).unwrap(), b"base\n");
        assert!(commit_target.join("deleted.txt").exists());
        assert!(!commit_target.join("新 file.bin").exists());
        let historical_target = fixture.0.join("historical-task");
        let historical = create(
            &root,
            &historical_target,
            "workz/task-h",
            Options {
                base: Some(&older),
                mode: "commit",
                allow_local_exclusion: true,
                nonce: "historical",
            },
        )
        .unwrap();
        assert_eq!(historical.base_sha, older);
        assert_eq!(historical.base_ref, "main");
        assert!(!historical_target.join("later.txt").exists());
        assert!(create(
            &root,
            &fixture.0.join("invalid-local"),
            "workz/task-invalid",
            Options {
                base: Some(&older),
                mode: "working",
                allow_local_exclusion: false,
                nonce: "invalid-local"
            }
        )
        .unwrap_err()
        .contains("current HEAD"));
        let local_target = fixture.0.join("local-task");
        let local = create(
            &root,
            &local_target,
            "workz/task-l",
            options("working", false, "working"),
        )
        .unwrap();
        assert_eq!(local.base_sha, head);
        assert_ne!(local.start_sha, head);
        assert_eq!(local.base_ref, "main");
        assert_eq!(
            run(&root, &["rev-parse", &format!("{}^", local.start_sha)]),
            head
        );
        assert_eq!(
            fs::read(local_target.join("file.txt")).unwrap(),
            b"unstaged\r\n"
        );
        assert_eq!(
            fs::read(local_target.join("新 file.bin")).unwrap(),
            [0, 255, 9]
        );
        assert!(!local_target.join("deleted.txt").exists());
        assert!(!local_target.join("ignored.txt").exists());
        assert!(!local_target.join(".agentz/journal.db").exists());
        assert!(local.omitted_paths.contains(&"ignored.txt".into()));
        assert!(local.omitted_paths.contains(&".agentz/journal.db".into()));
        assert_eq!(index, fs::read(root.join(".git/index")).unwrap());
        assert_eq!(run(&root, &["rev-parse", "HEAD"]), head);
        assert_eq!(run(&root, &["branch", "--show-current"]), "main");
        assert_eq!(fs::read(root.join("file.txt")).unwrap(), b"unstaged\r\n");
        assert!(create(
            &root,
            &local_target,
            "workz/task-other",
            options("working", false, "duplicate")
        )
        .is_err());
    }
}
