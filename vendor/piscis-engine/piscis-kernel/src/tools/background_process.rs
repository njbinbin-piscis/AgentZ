//! Agent-facing tools for registered background tasks (`process_wait`,
//! `process_output`, `process_kill`), plus shared helpers the command tools
//! (`shell`, `code_run`, `process_control`) use to start detached work.
//!
//! Waiting is done host-side in one tool call so polling never burns LLM
//! iterations; output reads are cursor-based so long logs stream in slices.

use crate::agent::tool::{Tool, ToolContext, ToolResult};
use crate::background::{
    BackgroundRegistry, BgEntry, BgOutcome, BgSnapshot, BgSpec, NotifyOn, ReadySpec,
};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

/// Parsed background options shared by `shell` / `code_run`.
pub struct BgParams {
    pub detached: bool,
    pub notify_on: NotifyOn,
    pub ready: ReadySpec,
}

pub fn parse_bg_params(input: &Value) -> BgParams {
    let ready = if let Some(port) = input["ready_port"].as_u64() {
        ReadySpec::Port(port.min(u16::MAX as u64) as u16)
    } else if let Some(needle) = input["ready_log"].as_str().filter(|s| !s.is_empty()) {
        ReadySpec::LogSubstring(needle.to_string())
    } else {
        ReadySpec::None
    };
    let notify_on = match input["notify_on"].as_str() {
        Some("ready") if ready != ReadySpec::None => NotifyOn::Ready,
        _ => NotifyOn::Exit,
    };
    BgParams {
        detached: input["background"].as_bool().unwrap_or(false),
        notify_on,
        ready,
    }
}

pub fn bg_spec(
    ctx: &ToolContext,
    label: String,
    cwd: String,
    params: &BgParams,
) -> BgSpec {
    BgSpec {
        session_id: ctx.session_id.clone(),
        workspace_root: ctx.workspace_root.to_string_lossy().into_owned(),
        label,
        cwd,
        notify_on: params.notify_on,
        ready: params.ready.clone(),
        max_runtime: None,
    }
}

/// Tool result returned when a command starts (or moves) into the background.
pub fn detached_result(
    entry: &BgEntry,
    started_detached: bool,
    stdout_so_far: &[u8],
    stderr_so_far: &[u8],
) -> ToolResult {
    let snap = entry.snapshot();
    let mut parts = vec![format!(
        "[background: {}] [pid: {}] [session: {}]\n{}",
        snap.id,
        snap.pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "?".into()),
        snap.session_id,
        if started_detached {
            "Started in the background; still running."
        } else {
            "Moved to the background after the foreground timeout; still running (NOT killed)."
        },
    )];
    let stdout = String::from_utf8_lossy(stdout_so_far);
    let stderr = String::from_utf8_lossy(stderr_so_far);
    if !stdout.trim().is_empty() {
        parts.push(format!("--- stdout so far ---\n{}", stdout.trim()));
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("--- stderr so far ---\n{}", stderr.trim()));
    }
    parts.push(
        "The task is registered and keeps running after this turn. \
         Use the wait tool to block until it exits (or is ready), process_output to read new output, \
         and process_kill to stop it. Do not poll with sleep loops."
            .to_string(),
    );
    ToolResult::ok(parts.join("\n\n"))
}

fn snapshot_text(entry: &BgEntry, tail_bytes: usize) -> String {
    let snap = entry.snapshot();
    let mut parts = vec![format!(
        "[background: {}] [status: {}] [pid: {}] [elapsed: {}ms]{}",
        snap.id,
        snap.status,
        snap.pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "?".into()),
        snap.elapsed_ms,
        snap.exit_code
            .map(|c| format!(" [exit: {c}]"))
            .unwrap_or_default(),
    )];
    if let Some(error) = &snap.error {
        parts.push(format!("error: {error}"));
    }
    let tail = entry.output_tail(tail_bytes);
    if !tail.trim().is_empty() {
        parts.push(format!("--- output tail ---\n{}", tail.trim()));
    }
    parts.join("\n\n")
}

/// Drive a plain (non-sandboxed) child process as a registered background
/// task. Used by `process_control` and by the Unix fallback for command
/// tools. The child runs in the caller's security context — callers decide
/// whether that is acceptable (sandboxed runners use their own exec).
pub fn spawn_std_detached(
    registry: &Arc<BackgroundRegistry>,
    entry: Arc<BgEntry>,
    mut command: Command,
) {
    let reg = registry.clone();
    registry.spawn_exec(entry, move |e| {
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt;
            command.pre_exec(|| {
                // New session so the task survives the agent's process group
                // signals; kill still targets the child explicitly.
                if libc_setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return BgOutcome::Failed(format!("failed to spawn: {error}")),
        };
        e.set_pid(child.id());
        let out_stream = e.stdout.clone();
        let err_stream = e.stderr.clone();
        let out_thread = child.stdout.take().map(|mut pipe| {
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match pipe.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => out_stream.append(&buf[..n]),
                    }
                }
            })
        });
        let err_thread = child.stderr.take().map(|mut pipe| {
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match pipe.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => err_stream.append(&buf[..n]),
                    }
                }
            })
        });
        let started = std::time::Instant::now();
        let outcome = loop {
            if e.is_killed() {
                #[cfg(unix)]
                libc_killpg(child.id() as i32);
                let _ = child.kill();
                let _ = child.wait();
                break BgOutcome::Exited(130);
            }
            if started.elapsed() >= e.max_runtime {
                #[cfg(unix)]
                libc_killpg(child.id() as i32);
                let _ = child.kill();
                let _ = child.wait();
                break BgOutcome::Failed(format!(
                    "background task exceeded max runtime ({}s)",
                    e.max_runtime.as_secs()
                ));
            }
            reg.probe_ready(&e);
            match child.try_wait() {
                Ok(Some(status)) => break BgOutcome::Exited(status.code().unwrap_or(-1)),
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(error) => break BgOutcome::Failed(format!("wait failed: {error}")),
            }
        };
        if let Some(t) = out_thread {
            let _ = t.join();
        }
        if let Some(t) = err_thread {
            let _ = t.join();
        }
        outcome
    });
}

/// Create a registry entry from `spec` and drive `command` as its process.
/// Shared by `process_control` and the non-Windows command-tool runners.
pub fn spawn_command_detached(
    registry: &Arc<BackgroundRegistry>,
    spec: BgSpec,
    command: Command,
) -> std::result::Result<Arc<BgEntry>, String> {
    let entry = registry.create(spec)?;
    spawn_std_detached(registry, entry.clone(), command);
    Ok(entry)
}

#[cfg(unix)]
fn libc_killpg(pgid: i32) {
    extern "C" {
        fn killpg(pgrp: i32, sig: i32) -> i32;
    }
    // SIGKILL the whole group created by setsid so grandchildren die too.
    unsafe {
        killpg(pgid, 9);
    }
}

#[cfg(unix)]
fn libc_setsid() -> i32 {
    // Avoid a libc dependency for one call; the symbol is in the linked libc.
    extern "C" {
        fn setsid() -> i32;
    }
    unsafe { setsid() }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::agent::tool::{ToolSettings, ToolContext};
    use crate::tools::shell::ShellTool;
    use std::sync::atomic::AtomicBool;

    fn ctx(workspace: std::path::PathBuf) -> ToolContext {
        ToolContext {
            session_id: "bg-shell-test".into(),
            workspace_root: workspace,
            bypass_permissions: false,
            settings: Arc::new(ToolSettings {
                sandbox_mode: crate::agent::tool::SandboxMode::Networked,
                ..ToolSettings::default()
            }),
            max_iterations: None,
            memory_owner_id: "test".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        }
    }

    fn bg_id_of(content: &str) -> String {
        let start = content.find("[background: ").unwrap() + "[background: ".len();
        let end = content[start..].find(']').unwrap() + start;
        content[start..end].to_string()
    }

    #[tokio::test]
    async fn shell_background_start_wait_kill_and_timeout_detach() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let registry = Arc::new(BackgroundRegistry::new());
        let tool = ShellTool {
            background_registry: Some(registry.clone()),
        };
        let ctx = ctx(workspace);

        // Explicit background start: returns immediately with a bg_id.
        let started = tool
            .call(
                json!({"command": "ping -n 30 127.0.0.1", "interpreter": "cmd", "background": true}),
                &ctx,
            )
            .await
            .expect("background start");
        assert!(!started.is_error, "{}", started.content);
        assert!(started.content.contains("Started in the background"), "{}", started.content);
        let bg_id = bg_id_of(&started.content);
        let entry = registry.get(&bg_id).expect("registered");
        assert_eq!(entry.snapshot().status, "running");

        // process_wait with a short timeout reports still-running.
        let waiter = ProcessWaitTool {
            background_registry: Some(registry.clone()),
        };
        let waited = waiter
            .call(json!({"bg_id": bg_id, "timeout_secs": 1}), &ctx)
            .await
            .expect("wait");
        assert!(waited.content.contains("still running"), "{}", waited.content);

        // Kill terminates the tree and the terminal state is observable.
        let killer = ProcessKillTool {
            background_registry: Some(registry.clone()),
        };
        let killed = killer
            .call(json!({"bg_id": bg_id}), &ctx)
            .await
            .expect("kill");
        assert!(killed.content.contains("killed"), "{}", killed.content);

        // Foreground timeout now detaches instead of killing: the returned
        // bg_id keeps running and can be killed afterwards.
        let detached = tool
            .call(
                json!({"command": "ping -n 30 127.0.0.1", "interpreter": "cmd", "timeout": 1}),
                &ctx,
            )
            .await
            .expect("foreground timeout detach");
        assert!(!detached.is_error, "{}", detached.content);
        assert!(
            detached.content.contains("Moved to the background"),
            "{}",
            detached.content
        );
        let bg_id2 = bg_id_of(&detached.content);
        assert_eq!(
            registry.get(&bg_id2).expect("detached registered").snapshot().status,
            "running"
        );
        assert!(registry.kill(&bg_id2));
        let snap = registry.wait_for(
            &registry.get(&bg_id2).unwrap(),
            Duration::from_secs(10),
            false,
        );
        assert_eq!(snap.status, "killed");
    }

    #[tokio::test]
    async fn wait_tool_session_scope_and_plain_delay() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let registry = Arc::new(BackgroundRegistry::new());
        let tool = ShellTool {
            background_registry: Some(registry.clone()),
        };
        let ctx = ctx(workspace);
        let started = tool
            .call(
                json!({"command": "ping -n 30 127.0.0.1", "interpreter": "cmd", "background": true}),
                &ctx,
            )
            .await
            .expect("background start");
        let bg_id = bg_id_of(&started.content);

        let wait = WaitTool {
            background_registry: Some(registry.clone()),
        };
        // Session-scope wait (no bg_id) sees the running task and times out politely.
        let waited = wait
            .call(json!({"timeout_secs": 1}), &ctx)
            .await
            .expect("session wait");
        assert!(waited.content.contains("Still waiting"), "{}", waited.content);

        // After kill, waiting on the id reports the terminal state.
        assert!(registry.kill(&bg_id));
        let done = wait
            .call(json!({"bg_id": bg_id, "timeout_secs": 10}), &ctx)
            .await
            .expect("id wait");
        assert!(done.content.contains("killed"), "{}", done.content);

        // Plain delay path with nothing running.
        let delayed = wait
            .call(json!({"seconds": 1}), &ctx)
            .await
            .expect("plain delay");
        assert!(delayed.content.contains("Waited 1s"), "{}", delayed.content);
    }
}

fn registry_of(
    registry: &Option<Arc<BackgroundRegistry>>,
) -> std::result::Result<Arc<BackgroundRegistry>, ToolResult> {
    registry.clone().ok_or_else(|| {
        ToolResult::err(
            "Background processes are not configured by this host; run the command in the foreground instead.",
        )
    })
}

fn entry_of(
    registry: &BackgroundRegistry,
    input: &Value,
) -> std::result::Result<Arc<BgEntry>, ToolResult> {
    let id = match input["bg_id"].as_str() {
        Some(id) if !id.is_empty() => id,
        _ => return Err(ToolResult::err("Missing required parameter: bg_id")),
    };
    registry
        .get(id)
        .ok_or_else(|| ToolResult::err(format!("Unknown background task: {id}")))
}

pub struct ProcessWaitTool {
    pub background_registry: Option<Arc<BackgroundRegistry>>,
}

#[async_trait]
impl Tool for ProcessWaitTool {
    fn name(&self) -> &str {
        "process_wait"
    }

    fn description(&self) -> &str {
        "Block until a registered background task exits (or its readiness condition fires), up to timeout_secs. \
         This waits host-side in a single call — always prefer it over sleep/poll loops. \
         Returns the final status, exit code and an output tail."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bg_id": { "type": "string", "description": "Background task id returned by shell/code_run/process_control." },
                "timeout_secs": { "type": "integer", "description": "Max seconds to wait (default 60, max 600)." },
                "wait_for": { "type": "string", "enum": ["exit", "ready"], "description": "Wait for process exit (default) or readiness (port/log condition given at start)." }
            },
            "required": ["bg_id"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let registry = match registry_of(&self.background_registry) {
            Ok(r) => r,
            Err(e) => return Ok(e),
        };
        let entry = match entry_of(&registry, &input) {
            Ok(e) => e,
            Err(e) => return Ok(e),
        };
        let timeout = Duration::from_secs(
            input["timeout_secs"]
                .as_u64()
                .unwrap_or(60)
                .clamp(1, 600),
        );
        let want_ready = input["wait_for"].as_str() == Some("ready");
        let reg = registry.clone();
        let entry_for_wait = entry.clone();
        let snap = tokio::task::spawn_blocking(move || {
            reg.wait_for(&entry_for_wait, timeout, want_ready)
        })
        .await
        .map_err(|e| anyhow::anyhow!("process_wait worker failed: {e}"))?;
        let mut text = snapshot_text(&entry, 8192);
        if snap.status == "running" && !(want_ready && snap.ready) {
            text.push_str("\n\n(still running after the wait; call process_wait again or process_kill)");
        }
        Ok(ToolResult::ok(text))
    }
}

/// The general-purpose wait primitive (Cursor-style "await").
///
/// - With `bg_id`: block until that background task exits / is ready.
/// - Without `bg_id`: block until background tasks of this session finish —
///   `scope: "any"` (default; first finisher wins) or `"all"`.
/// - With `seconds` and no running tasks: a plain bounded delay, so agents
///   never need `sleep` shell hacks. Respects turn cancellation.
///
/// All waiting happens host-side in this single call; no LLM iterations or
/// polling loops are burned while waiting.
pub struct WaitTool {
    pub background_registry: Option<Arc<BackgroundRegistry>>,
}

#[async_trait]
impl Tool for WaitTool {
    fn name(&self) -> &str {
        "wait"
    }

    fn description(&self) -> &str {
        "Wait for background work, host-side in one call. Pass bg_id to wait for one task, \
         or omit it to wait for this session's background tasks (scope=any: first to finish; scope=all: every one). \
         until=ready waits for the readiness condition (port/log) instead of exit. \
         With seconds and nothing to wait on, it is a plain bounded delay. \
         Prefer this over shell sleep/poll loops whenever you start long-running work."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bg_id": { "type": "string", "description": "Optional background task id (from shell/code_run/process_control). Omit to wait on this session's tasks." },
                "scope": { "type": "string", "enum": ["any", "all"], "description": "Without bg_id: return when the first task finishes (any, default) or when all finish (all)." },
                "until": { "type": "string", "enum": ["exit", "ready"], "description": "Wait for process exit (default) or the readiness condition given at start." },
                "timeout_secs": { "type": "integer", "description": "Max seconds to wait (default 120, max 600)." },
                "seconds": { "type": "integer", "description": "Plain delay in seconds when there is no task to wait on (max 600)." }
            }
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let registry = match registry_of(&self.background_registry) {
            Ok(r) => r,
            Err(e) => return Ok(e),
        };
        let timeout = Duration::from_secs(
            input["timeout_secs"].as_u64().unwrap_or(120).clamp(1, 600),
        );
        let want_ready = input["until"].as_str() == Some("ready");

        // One specific task: identical semantics to process_wait.
        if let Some(id) = input["bg_id"].as_str().filter(|v| !v.is_empty()) {
            let entry = match entry_of(&registry, &json!({ "bg_id": id })) {
                Ok(e) => e,
                Err(e) => return Ok(e),
            };
            let reg = registry.clone();
            let entry_for_wait = entry.clone();
            let snap = tokio::task::spawn_blocking(move || {
                reg.wait_for(&entry_for_wait, timeout, want_ready)
            })
            .await
            .map_err(|e| anyhow::anyhow!("wait worker failed: {e}"))?;
            let mut text = snapshot_text(&entry, 8192);
            if snap.status == "running" && !(want_ready && snap.ready) {
                text.push_str("\n\n(still running after the wait; call wait again or process_kill)");
            }
            return Ok(ToolResult::ok(text));
        }

        // Session-wide wait over currently running tasks.
        let session_tasks: Vec<Arc<BgEntry>> = registry
            .list(Some(&ctx.session_id), None)
            .iter()
            .filter(|snap| snap.status == "running")
            .filter_map(|snap| registry.get(&snap.id))
            .collect();

        if session_tasks.is_empty() {
            if let Some(seconds) = input["seconds"].as_u64().filter(|v| *v > 0) {
                let total = Duration::from_secs(seconds.min(600));
                let started = std::time::Instant::now();
                while started.elapsed() < total {
                    if ctx.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        return Ok(ToolResult::ok(format!(
                            "Wait cancelled after {}s of {}s.",
                            started.elapsed().as_secs(),
                            total.as_secs()
                        )));
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                return Ok(ToolResult::ok(format!(
                    "Waited {}s (no background tasks running in this session).",
                    total.as_secs()
                )));
            }
            let recent = registry.list(Some(&ctx.session_id), None);
            if recent.is_empty() {
                return Ok(ToolResult::ok(
                    "No background tasks in this session — nothing to wait for. \
                     Start one with shell/code_run background=true, then call wait."
                        .to_string(),
                ));
            }
            let summary = recent
                .iter()
                .take(5)
                .map(|t| format!("{} [{}] {}", t.id, t.status, t.label))
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(ToolResult::ok(format!(
                "No running background tasks in this session. Recent tasks:\n{summary}"
            )));
        }

        let scope_all = input["scope"].as_str() == Some("all");
        let reg = registry.clone();
        let tasks = session_tasks.clone();
        let finished = tokio::task::spawn_blocking(move || {
            let deadline = std::time::Instant::now() + timeout;
            loop {
                let mut done: Vec<BgSnapshot> = Vec::new();
                let mut pending = 0usize;
                for entry in &tasks {
                    if want_ready {
                        reg.probe_ready(entry);
                    }
                    let snap = entry.snapshot();
                    let satisfied = if want_ready {
                        snap.ready || snap.status != "running"
                    } else {
                        snap.status != "running"
                    };
                    if satisfied {
                        done.push(snap);
                    } else {
                        pending += 1;
                    }
                }
                let satisfied_all = pending == 0;
                if (scope_all && satisfied_all) || (!scope_all && !done.is_empty()) {
                    return done;
                }
                if std::time::Instant::now() >= deadline {
                    return done;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("wait worker failed: {e}"))?;

        if finished.is_empty() {
            let names = session_tasks
                .iter()
                .map(|e| format!("{} ({})", e.id, e.label))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(ToolResult::ok(format!(
                "Still waiting after {}s — no task satisfied the condition yet: {names}. Call wait again to keep waiting.",
                timeout.as_secs()
            )));
        }
        let mut parts = vec![format!(
            "Wait finished: {} task(s) {}.",
            finished.len(),
            if want_ready { "ready/finished" } else { "finished" }
        )];
        for snap in &finished {
            if let Some(entry) = registry.get(&snap.id) {
                parts.push(snapshot_text(&entry, 4096));
            }
        }
        let still = session_tasks
            .iter()
            .filter(|e| e.snapshot().status == "running")
            .count();
        if still > 0 {
            parts.push(format!(
                "{still} background task(s) still running in this session."
            ));
        }
        Ok(ToolResult::ok(parts.join("\n\n")))
    }
}

pub struct ProcessOutputTool {
    pub background_registry: Option<Arc<BackgroundRegistry>>,
}

#[async_trait]
impl Tool for ProcessOutputTool {
    fn name(&self) -> &str {
        "process_output"
    }

    fn description(&self) -> &str {
        "Read new output of a registered background task using absolute cursors returned by the previous call \
         (start with cursor_out=0, cursor_err=0). Returns the new text plus updated cursors; truncated=true means \
         the ring buffer already dropped older bytes."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bg_id": { "type": "string" },
                "cursor_out": { "type": "integer", "description": "stdout cursor from the previous call (default 0)." },
                "cursor_err": { "type": "integer", "description": "stderr cursor from the previous call (default 0)." }
            },
            "required": ["bg_id"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let registry = match registry_of(&self.background_registry) {
            Ok(r) => r,
            Err(e) => return Ok(e),
        };
        let entry = match entry_of(&registry, &input) {
            Ok(e) => e,
            Err(e) => return Ok(e),
        };
        let (out, next_out, out_trunc) =
            entry.stdout.read_from(input["cursor_out"].as_u64().unwrap_or(0));
        let (err, next_err, err_trunc) =
            entry.stderr.read_from(input["cursor_err"].as_u64().unwrap_or(0));
        let snap = entry.snapshot();
        let mut parts = vec![format!(
            "[background: {}] [status: {}] cursor_out={} cursor_err={}{}{}",
            snap.id,
            snap.status,
            next_out,
            next_err,
            if out_trunc || err_trunc { " truncated=true" } else { "" },
            snap.exit_code
                .map(|c| format!(" [exit: {c}]"))
                .unwrap_or_default(),
        )];
        if !out.is_empty() {
            parts.push(format!(
                "--- stdout ---\n{}",
                String::from_utf8_lossy(&out)
            ));
        }
        if !err.is_empty() {
            parts.push(format!(
                "--- stderr ---\n{}",
                String::from_utf8_lossy(&err)
            ));
        }
        if out.is_empty() && err.is_empty() {
            parts.push("(no new output)".to_string());
        }
        Ok(ToolResult::ok(parts.join("\n\n")))
    }
}

pub struct ProcessKillTool {
    pub background_registry: Option<Arc<BackgroundRegistry>>,
}

#[async_trait]
impl Tool for ProcessKillTool {
    fn name(&self) -> &str {
        "process_kill"
    }

    fn description(&self) -> &str {
        "Terminate a registered background task and its whole process tree. \
         Returns once the termination has been fully confirmed."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bg_id": { "type": "string" }
            },
            "required": ["bg_id"]
        })
    }

    fn needs_confirmation(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let registry = match registry_of(&self.background_registry) {
            Ok(r) => r,
            Err(e) => return Ok(e),
        };
        let entry = match entry_of(&registry, &input) {
            Ok(e) => e,
            Err(e) => return Ok(e),
        };
        if entry.snapshot().status != "running" {
            return Ok(ToolResult::ok(format!(
                "Background task {} is already {}.",
                entry.id,
                entry.snapshot().status
            )));
        }
        registry.kill(&entry.id);
        let reg = registry.clone();
        let entry_for_wait = entry.clone();
        let snap = tokio::task::spawn_blocking(move || {
            reg.wait_for(&entry_for_wait, Duration::from_secs(30), false)
        })
        .await
        .map_err(|e| anyhow::anyhow!("process_kill worker failed: {e}"))?;
        Ok(ToolResult::ok(format!(
            "Background task {} terminated. [status: {}]",
            snap.id, snap.status
        )))
    }
}
