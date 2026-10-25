//! Windows end-to-end recovery contract through the kernel shell tool.
//! A real two-level `cmd` child tree is started, then terminated by timeout and by
//! cancellation. The structured `recovery_v1` block must report `tree_terminated`, the tree
//! must be observed alive while running, and no process carrying this run's marker may survive.
#![cfg(windows)]

use piscis_kernel::agent::tool::{Tool, ToolContext, ToolSettings};
use piscis_kernel::tools::shell::ShellTool;
use serde_json::json;
use std::{
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

/// Unique marker per test so this run's processes can be identified by command line.
fn marker(offset: u32) -> u32 {
    5000 + (std::process::id() % 1000) * 10 + offset
}

/// Long-running two-level cmd tree (outer cmd -> inner cmd loop). Ping is denied by the
/// AppContainer, so the loop is local-only; the unique `rem` tag identifies this run's processes.
fn long_tree(marker: u32) -> String {
    format!("cmd /c for /L %i in (1,1,2000000000) do @rem AZ022-{marker}")
}

/// Number of live cmd.exe processes whose command line carries this run's marker.
/// A failed query returns usize::MAX so it can never be mistaken for "zero survivors".
fn surviving_children(marker: u32) -> usize {
    let script = format!(
        "@(Get-CimInstance Win32_Process | Where-Object {{ $_.Name -eq 'cmd.exe' -and $_.CommandLine -like '*AZ022-{marker}*' }}).Count"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .expect("query child processes");
    if !output.status.success() || !output.stderr.is_empty() {
        return usize::MAX;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(usize::MAX)
}

/// Minimum number of marker-carrying cmd.exe processes that proves the two-level tree existed
/// (outer kernel cmd plus inner loop cmd).
const TREE_MIN: usize = 2;

/// Polls while the tool runs; records the largest live tree size observed.
struct Observer {
    done: Arc<AtomicBool>,
    max_seen: Arc<AtomicUsize>,
    handle: thread::JoinHandle<()>,
}

impl Observer {
    fn start(marker: u32) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let (d, m) = (done.clone(), max_seen.clone());
        let handle = thread::spawn(move || {
            while !d.load(Ordering::SeqCst) {
                let count = surviving_children(marker);
                if count != usize::MAX {
                    m.fetch_max(count, Ordering::SeqCst);
                }
                thread::sleep(Duration::from_millis(200));
            }
        });
        Self { done, max_seen, handle }
    }

    fn finish(self) -> bool {
        self.done.store(true, Ordering::SeqCst);
        self.handle.join().unwrap();
        self.max_seen.load(Ordering::SeqCst) >= TREE_MIN
    }
}

fn wait_for_no_survivors(marker: u32) -> usize {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let survivors = surviving_children(marker);
        if survivors == 0 || Instant::now() >= deadline {
            return survivors;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn context(root: PathBuf, cancel: Arc<AtomicBool>) -> ToolContext {
    ToolContext {
        session_id: "az022-tool-recovery".into(),
        workspace_root: root,
        bypass_permissions: false,
        settings: Arc::new(ToolSettings::default()),
        max_iterations: None,
        memory_owner_id: "az022".into(),
        pool_session_id: None,
        tool_use_id: None,
        cancel,
        loop_halt: None,
    }
}

fn scratch_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("agentz-az022-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(root.join("workspace")).expect("create workspace");
    root.join("workspace")
}

#[tokio::test]
async fn timed_out_child_tree_is_terminated_and_reported() {
    let count = marker(1);
    let root = scratch_root("timeout");
    let ctx = context(root.clone(), Arc::new(AtomicBool::new(false)));
    let observer = Observer::start(count);

    let started = Instant::now();
    let result = ShellTool::default()
        .call(json!({"command": long_tree(count), "interpreter": "cmd", "timeout": 8}), &ctx)
        .await
        .expect("shell call returns a result");
    let elapsed = started.elapsed();
    let seen_alive = observer.finish();

    assert!(result.is_error, "{}", result.content);
    assert!(result.content.contains("[recovery_v1]"), "{}", result.content);
    assert!(
        result.content.contains("\"outcome\":\"timed_out\""),
        "{}",
        result.content
    );
    assert!(
        result.content.contains("\"termination\":\"tree_terminated\""),
        "{}",
        result.content
    );
    assert!(
        elapsed < Duration::from_secs(60),
        "timeout did not bound the run: {elapsed:?}"
    );
    assert!(seen_alive, "two-level child tree was never observed alive; test proves nothing");
    assert_eq!(
        wait_for_no_survivors(count),
        0,
        "cmd child tree survived timeout termination"
    );
    let _ = std::fs::remove_dir_all(root.parent().unwrap());
}

#[tokio::test]
async fn cancelled_child_tree_is_terminated_and_reported() {
    let count = marker(2);
    let root = scratch_root("cancel");
    let cancel = Arc::new(AtomicBool::new(false));
    let ctx = context(root.clone(), cancel.clone());
    let observer = Observer::start(count);

    // Cancel only after the two-level tree has been observed alive (bounded wait).
    let trigger = cancel.clone();
    let observed = Arc::new(AtomicUsize::new(0));
    let observed_in = observed.clone();
    let max_seen = observer.max_seen.clone();
    let canceller = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while max_seen.load(Ordering::SeqCst) < TREE_MIN && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
        observed_in.store(max_seen.load(Ordering::SeqCst), Ordering::SeqCst);
        trigger.store(true, Ordering::SeqCst);
    });
    let result = ShellTool::default()
        .call(json!({"command": long_tree(count), "interpreter": "cmd", "timeout": 120}), &ctx)
        .await
        .expect("shell call returns a result");
    canceller.join().unwrap();
    let seen_alive = observer.finish();
    assert!(
        observed.load(Ordering::SeqCst) >= TREE_MIN,
        "tree never reached {TREE_MIN} live processes before cancel"
    );

    assert!(result.is_error, "{}", result.content);
    assert!(result.content.contains("[recovery_v1]"), "{}", result.content);
    assert!(
        result.content.contains("\"outcome\":\"cancelled\""),
        "{}",
        result.content
    );
    assert!(
        result.content.contains("\"termination\":\"tree_terminated\""),
        "{}",
        result.content
    );
    assert!(seen_alive, "two-level child tree was never observed alive; test proves nothing");
    assert_eq!(
        wait_for_no_survivors(count),
        0,
        "cmd child tree survived cancellation"
    );
    let _ = std::fs::remove_dir_all(root.parent().unwrap());
}
