//! Cross-process owner leases for active Run attempts (AZ-019).
//!
//! The process that queues or executes a Run attempt holds an exclusive OS lock on
//! `<project>/.agentz/owners/<run>.<attempt>.lease` for the attempt's whole queued
//! and running lifetime. The lease file also records the full binding, so startup
//! reconciliation rebuilds the captured identity from host-written evidence instead
//! of renderer or model input.
//!
//! Evidence, in order:
//! - lease file present and lock held by a live handle: the owner is alive, preserved;
//! - lease file present and lock free: the owner process is gone (the OS releases the
//!   lock on process death), so the Run is interrupted without re-execution;
//! - no lease file: no owner evidence, unverifiable, preserved.

use agentz_task_state::{
    approval::Binding,
    recovery::{CapturedRun, Outcome, OwnerEvidence, RecoveryStore},
    run::RunStore,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    binding: Binding,
    pid: u32,
    acquired_ms: i64,
}

/// Held for one Run attempt's queued/running lifetime. Dropping it, or the owning
/// process dying, releases the OS lock.
#[derive(Debug)]
pub struct OwnerLease {
    /// Keeps the OS lock alive; never read after acquisition.
    _lock: File,
    record: Record,
}

#[derive(Debug)]
pub enum Probe {
    /// No lease file exists for this attempt.
    Missing,
    /// A live handle holds the lock.
    Held,
    /// The lock was free and this caller now holds it while inspecting the record.
    Gone(OwnerLease),
}

/// Per-Run disposition reported by [`reconcile_project`].
#[derive(Debug, PartialEq, Eq)]
pub enum Disposition {
    Unverifiable,
    Alive,
    Interrupted,
    Error(String),
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn lease_path(project: &Path, run: &str, attempt: &str) -> Result<PathBuf, String> {
    for id in [run, attempt] {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err("unsafe owner lease identity".into());
        }
    }
    // Canonical root so acquire (caller's path form) and reconcile agree on one file.
    let root = agentz_task_state::canonical_root(project)?;
    Ok(PathBuf::from(root)
        .join(".agentz")
        .join("owners")
        .join(format!("{run}.{attempt}.lease")))
}

/// `Ok(true)` when the exclusive lock was taken, `Ok(false)` when another live handle holds it.
#[cfg(windows)]
fn try_exclusive(file: &File) -> Result<bool, String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;
    // SAFETY: OVERLAPPED is plain data; an all-zero value is a valid offset-0 request.
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    // SAFETY: the handle is owned by `file` for the duration of the call; the byte-range lock
    // covers byte 0 only and is released when the handle closes.
    let ok = unsafe {
        LockFileEx(
            file.as_raw_handle() as _,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        )
    };
    if ok != 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
        Ok(false)
    } else {
        Err(format!("owner lease lock failed: {error}"))
    }
}

/// `Ok(true)` when the exclusive lock was taken, `Ok(false)` when another live handle holds it.
#[cfg(unix)]
fn try_exclusive(file: &File) -> Result<bool, String> {
    use std::os::unix::io::AsRawFd;
    // SAFETY: the descriptor is valid for the lifetime of `file`; flock has no memory effects.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::WouldBlock {
        Ok(false)
    } else {
        Err(format!("owner lease lock failed: {error}"))
    }
}

impl OwnerLease {
    /// Creates the lease for a binding and holds it. Fails if a live handle already holds it.
    pub fn acquire(project: &Path, binding: &Binding) -> Result<Self, String> {
        binding.validate()?;
        let path = lease_path(project, &binding.run_id, &binding.attempt_id)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("owner lease directory: {e}"))?;
        }
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| format!("owner lease open: {e}"))?;
        if !try_exclusive(&file)? {
            return Err("owner lease already held by a live owner".into());
        }
        let record = Record {
            binding: binding.clone(),
            pid: std::process::id(),
            acquired_ms: now(),
        };
        let bytes = serde_json::to_vec(&record).map_err(|e| format!("owner lease encode: {e}"))?;
        file.set_len(0)
            .map_err(|e| format!("owner lease truncate: {e}"))?;
        file.seek(SeekFrom::Start(0))
            .and_then(|_| file.write_all(&bytes))
            .and_then(|_| file.sync_data())
            .map_err(|e| format!("owner lease write: {e}"))?;
        Ok(Self {
            _lock: file,
            record,
        })
    }
}

/// Checks whether the owner of one attempt is still alive. A `Gone` result keeps the lock
/// held by the caller, so no new owner can begin the attempt while it is reconciled.
pub fn probe(project: &Path, run: &str, attempt: &str) -> Result<Probe, String> {
    let path = lease_path(project, run, attempt)?;
    let mut file = match OpenOptions::new().read(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Probe::Missing),
        Err(error) => return Err(format!("owner lease probe: {error}")),
    };
    if !try_exclusive(&file)? {
        return Ok(Probe::Held);
    }
    let mut raw = Vec::new();
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.read_to_end(&mut raw))
        .map_err(|e| format!("owner lease read: {e}"))?;
    let record: Record = serde_json::from_slice(&raw)
        .map_err(|_| "owner lease record is corrupt; preserved".to_string())?;
    if record.binding.run_id != run || record.binding.attempt_id != attempt {
        return Err("owner lease identity mismatch".into());
    }
    Ok(Probe::Gone(OwnerLease {
        _lock: file,
        record,
    }))
}

/// Reconciles every active Run in a project against its owner lease. Runs without a lease
/// file are reported as unverifiable and left untouched. Errors are per Run.
pub fn reconcile_project(
    project: &Path,
    now_ms: i64,
) -> Result<Vec<(String, Disposition)>, String> {
    if !project.join(".agentz").join("runs.db").is_file() {
        return Ok(Vec::new());
    }
    let active = RunStore::open_existing_readonly(project)?.active_runs()?;
    if active.is_empty() {
        return Ok(Vec::new());
    }
    let mut recovery = RecoveryStore::open_existing(project)?;
    let mut report = Vec::with_capacity(active.len());
    for run in active {
        let disposition = match probe(project, &run.run_id, &run.attempt_id) {
            Ok(Probe::Missing) => Disposition::Unverifiable,
            Ok(Probe::Held) => Disposition::Alive,
            Ok(Probe::Gone(lease)) => {
                let captured = CapturedRun {
                    binding: lease.record.binding.clone(),
                    execution: run.execution,
                    updated_ms: run.updated_ms,
                };
                let evidence = OwnerEvidence::ConfirmedGone {
                    evidence_id: format!("owner-lease:{}:{}", run.run_id, lease.record.acquired_ms),
                };
                match recovery.reconcile(&captured, evidence, now_ms) {
                    Ok(Outcome::Interrupted(_)) => Disposition::Interrupted,
                    Ok(Outcome::PreservedAlive) => Disposition::Alive,
                    Ok(Outcome::PreservedUnverifiable) => Disposition::Unverifiable,
                    Err(error) => Disposition::Error(error),
                }
            }
            Err(error) => Disposition::Error(error),
        };
        report.push((run.run_id, disposition));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::durable_run::ManagedRun;
    use agentz_task_state::run::Execution;
    use std::time::{Duration, Instant};

    const CHILD_ENV: &str = "AGENTZ_OWNER_LEASE_CHILD";
    const CHILD_RUN: &str = "child-run";
    const CHILD_ATTEMPT: &str = "child-attempt";

    fn project() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("agentz-owner-lease-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        RunStore::open(&root).unwrap();
        root
    }

    fn binding(root: &Path, run: &str, attempt: &str) -> Binding {
        let bv = "a".repeat(64);
        Binding {
            project_id: agentz_task_state::canonical_root(root).unwrap(),
            task_id: "task".into(),
            environment_id: format!("direct:{bv}"),
            binding_version: bv,
            run_id: run.into(),
            attempt_id: attempt.into(),
            capability_version: "fixture".into(),
        }
    }

    #[test]
    fn held_lease_is_alive_and_released_lease_reads_back_its_binding() {
        let root = project();
        let b = binding(&root, "run-held", "attempt-held");
        let lease = OwnerLease::acquire(&root, &b).unwrap();
        assert!(matches!(
            probe(&root, "run-held", "attempt-held").unwrap(),
            Probe::Held
        ));
        assert!(
            OwnerLease::acquire(&root, &b).is_err(),
            "a second live owner must not acquire the same attempt"
        );
        drop(lease);
        match probe(&root, "run-held", "attempt-held").unwrap() {
            Probe::Gone(lease) => assert_eq!(lease.record.binding, b),
            other => panic!("expected Gone after release, got {other:?}"),
        }
    }

    #[test]
    fn attempt_without_lease_file_is_unverifiable() {
        let root = project();
        assert!(matches!(
            probe(&root, "run-none", "attempt-none").unwrap(),
            Probe::Missing
        ));
    }

    #[test]
    fn unsafe_identities_never_reach_the_filesystem() {
        let root = project();
        assert!(probe(&root, "../escape", "a").is_err());
        assert!(probe(&root, "run", "a/b").is_err());
        assert!(lease_path(&root, "", "a").is_err());
    }

    /// Child half of the crash fixture. Runs only when the parent sets `CHILD_ENV`.
    #[test]
    fn lease_child_process() {
        let Ok(root) = std::env::var(CHILD_ENV) else {
            return;
        };
        let root = PathBuf::from(root);
        let b = binding(&root, CHILD_RUN, CHILD_ATTEMPT);
        let _run = ManagedRun::begin_bound(&root, &b).unwrap();
        fs::write(root.join("child.ready"), b"ready").unwrap();
        std::thread::sleep(Duration::from_secs(600));
    }

    #[test]
    fn killed_owner_is_interrupted_while_live_owner_is_preserved() {
        let root = project();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "runtime::owner_lease::tests::lease_child_process",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_ENV, &root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let ready = root.join("child.ready");
        let started = Instant::now();
        while !ready.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(120),
                "child never became ready"
            );
            std::thread::sleep(Duration::from_millis(50));
        }

        // Live owner: lock held by the child, Run still Running. Must be preserved.
        let report = reconcile_project(&root, now()).unwrap();
        assert_eq!(report.len(), 1, "{report:?}");
        assert_eq!(report[0], (CHILD_RUN.to_string(), Disposition::Alive));
        let store = RunStore::open_existing_readonly(&root).unwrap();
        assert_eq!(
            store.load(CHILD_RUN).unwrap().unwrap().execution,
            Execution::Running
        );
        drop(store);

        // Crash: kill the owner. The OS releases its lock; the Run must be interrupted.
        child.kill().unwrap();
        child.wait().unwrap();
        let report = reconcile_project(&root, now()).unwrap();
        assert_eq!(
            report,
            vec![(CHILD_RUN.to_string(), Disposition::Interrupted)]
        );
        let store = RunStore::open_existing_readonly(&root).unwrap();
        let run = store.load(CHILD_RUN).unwrap().unwrap();
        assert_eq!(run.execution, Execution::Interrupted);
        assert_eq!(run.delivery, agentz_task_state::run::Delivery::NotReady);
        drop(store);

        // Idempotent: nothing active remains.
        assert!(reconcile_project(&root, now()).unwrap().is_empty());
    }
}
