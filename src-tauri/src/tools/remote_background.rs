//! Remote `background=true` commands.
//!
//! The process runs on the SSH / Docker / WSL host, detached from the control
//! connection. Output, readiness and stop go through the same registry the
//! IDE panel already lists.

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use piscis_kernel::agent::tool::{ToolContext, ToolResult};
use piscis_kernel::background::{BackgroundRegistry, BgEntry, BgOutcome, BgSpec, ReadySpec};
use piscis_kernel::tools::background_process::{self, detached_result};

use crate::remote::{self, shell_quote, RemoteTarget, SERVER_DIR};

const POLL: Duration = Duration::from_millis(1000);

pub fn start(
    ctx: &ToolContext,
    command: &str,
    cwd: &str,
    input: &serde_json::Value,
) -> anyhow::Result<ToolResult> {
    let params = background_process::parse_bg_params(input);
    if !params.detached {
        return Ok(ToolResult::ok(String::new()));
    }
    let registry = remote::background_registry()
        .ok_or_else(|| anyhow::anyhow!("background registry unavailable"))?;
    let session = remote::bound_session()
        .ok_or_else(|| anyhow::anyhow!("remote session is not bound"))?;
    let spec = background_process::bg_spec(
        ctx,
        command.chars().take(200).collect(),
        cwd.to_string(),
        &params,
    );
    let env = env_pairs(input);
    let entry = spawn(registry, session.target, spec, command, &env)?;
    Ok(detached_result(&entry, true, &[], &[]))
}

/// `Ok(None)` when the caller should run the command in the foreground.
pub fn start_if_background(
    ctx: &ToolContext,
    command: &str,
    cwd: &str,
    input: &serde_json::Value,
) -> anyhow::Result<Option<ToolResult>> {
    if !input["background"].as_bool().unwrap_or(false) {
        return Ok(None);
    }
    start(ctx, command, cwd, input).map(Some)
}

fn env_pairs(input: &serde_json::Value) -> Vec<(String, String)> {
    input["env"]
        .as_object()
        .map(|env| {
            env.iter()
                .filter_map(|(k, v)| {
                    if k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                        Some((k.clone(), v.as_str().unwrap_or_default().to_string()))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn spawn(
    registry: Arc<BackgroundRegistry>,
    target: RemoteTarget,
    spec: BgSpec,
    command: &str,
    env: &[(String, String)],
) -> anyhow::Result<Arc<BgEntry>> {
    let handle = tokio::runtime::Handle::current();
    let entry = registry
        .create(spec)
        .map_err(anyhow::Error::msg)?;
    let command = command.to_string();
    let env = env.to_vec();
    let reg = registry.clone();
    registry.spawn_exec(entry.clone(), move |task| {
        handle.block_on(drive(target, task, reg, command, env))
    });
    Ok(entry)
}

async fn drive(
    target: RemoteTarget,
    task: Arc<BgEntry>,
    registry: Arc<BackgroundRegistry>,
    command: String,
    env: Vec<(String, String)>,
) -> BgOutcome {
    let started = Instant::now();
    let launch = match target.run(&launch_script(&task.id, &task.cwd, &command, &env)).await {
        Ok(out) => out,
        Err(error) => return BgOutcome::Failed(error),
    };
    let Some(pid) = launch
        .lines()
        .rev()
        .find_map(|line| line.trim().parse::<u32>().ok())
    else {
        return BgOutcome::Failed(format!(
            "remote background start did not return a pid: {}",
            launch.trim()
        ));
    };
    task.set_pid(pid);
    let mut offset = 0u64;
    loop {
        if task.is_killed() {
            let _ = target.run(&kill_script(&task.id, pid)).await;
            pull_log(&target, &task, &mut offset).await;
            return BgOutcome::Exited(130);
        }
        if started.elapsed() >= task.max_runtime {
            let _ = target.run(&kill_script(&task.id, pid)).await;
            pull_log(&target, &task, &mut offset).await;
            return BgOutcome::Failed(format!(
                "background task exceeded max runtime ({}s)",
                task.max_runtime.as_secs()
            ));
        }
        let poll = match target.run(&poll_script(&task.id, offset, pid)).await {
            Ok(text) => text,
            Err(error) => return BgOutcome::Failed(error),
        };
        let parsed = parse_poll(&poll);
        if !parsed.log.is_empty() {
            task.stdout.append(parsed.log.as_bytes());
            offset = offset.saturating_add(parsed.log.len() as u64);
        }
        match &task.ready {
            ReadySpec::LogSubstring(_) => {
                registry.probe_ready(&task);
            }
            ReadySpec::Port(port) => {
                if port_open(&target, *port).await {
                    registry.note_ready(&task);
                }
            }
            ReadySpec::None => {}
        }
        if !parsed.alive {
            pull_log(&target, &task, &mut offset).await;
            return BgOutcome::Exited(parsed.exit_code.unwrap_or(-1));
        }
        tokio::time::sleep(POLL).await;
    }
}

async fn pull_log(target: &RemoteTarget, task: &BgEntry, offset: &mut u64) {
    let script = format!(
        "log=\"$HOME/{SERVER_DIR}/bg/{}.log\"; if [ -f \"$log\" ]; then tail -c +{} \"$log\"; fi",
        task.id,
        offset.saturating_add(1),
    );
    if let Ok(text) = target.run(&script).await {
        if !text.is_empty() {
            task.stdout.append(text.as_bytes());
            *offset = offset.saturating_add(text.len() as u64);
        }
    }
}

async fn port_open(target: &RemoteTarget, port: u16) -> bool {
    let script = format!(
        "if command -v bash >/dev/null 2>&1; then bash -c 'echo >/dev/tcp/127.0.0.1/{port}' >/dev/null 2>&1 && echo open || echo closed; else echo closed; fi"
    );
    target
        .run(&script)
        .await
        .map(|out| out.lines().any(|line| line.trim() == "open"))
        .unwrap_or(false)
}

pub(crate) fn launch_script(id: &str, cwd: &str, command: &str, env: &[(String, String)]) -> String {
    let mut body = format!("#!/bin/sh\ncd {} || exit 1\n", shell_quote(cwd));
    for (key, value) in env {
        body.push_str(&format!("export {key}={}\n", shell_quote(value)));
    }
    body.push_str(remote::TOOL_PATH_SETUP);
    body.push('\n');
    body.push_str(&format!(
        "sh -c {}\ncode=$?\necho \"$code\" > \"$HOME/{SERVER_DIR}/bg/{id}.exit\"\nexit \"$code\"\n",
        shell_quote(command)
    ));
    let encoded = STANDARD.encode(body.as_bytes());
    format!(
        "d=\"$HOME/{SERVER_DIR}/bg\"; mkdir -p \"$d\"; printf %s {enc} | base64 -d > \"$d/{id}.sh\" && chmod +x \"$d/{id}.sh\" && \
         if command -v setsid >/dev/null 2>&1; then setsid \"$d/{id}.sh\" > \"$d/{id}.log\" 2>&1 </dev/null & else nohup \"$d/{id}.sh\" > \"$d/{id}.log\" 2>&1 </dev/null & fi; echo $!",
        enc = shell_quote(&encoded),
    )
}

fn poll_script(id: &str, offset: u64, pid: u32) -> String {
    format!(
        "d=\"$HOME/{SERVER_DIR}/bg\"; log=\"$d/{id}.log\"; \
         if kill -0 {pid} >/dev/null 2>&1; then echo alive; else echo dead; fi; \
         if [ -f \"$d/{id}.exit\" ]; then cat \"$d/{id}.exit\"; else echo -; fi; \
         if [ -f \"$log\" ]; then tail -c +{start} \"$log\"; fi",
        start = offset.saturating_add(1),
    )
}

fn kill_script(id: &str, pid: u32) -> String {
    format!(
        "kill -TERM -{pid} >/dev/null 2>&1 || kill -TERM {pid} >/dev/null 2>&1 || true; \
         sleep 0.3; \
         kill -KILL -{pid} >/dev/null 2>&1 || kill -KILL {pid} >/dev/null 2>&1 || true; \
         echo killed > \"$HOME/{SERVER_DIR}/bg/{id}.killed\""
    )
}

struct Poll {
    alive: bool,
    exit_code: Option<i32>,
    log: String,
}

fn parse_poll(text: &str) -> Poll {
    let mut lines = text.splitn(3, '\n');
    let alive = lines.next().unwrap_or("").trim() == "alive";
    let exit_line = lines.next().unwrap_or("").trim();
    let exit_code = exit_line.parse::<i32>().ok();
    let log = lines.next().unwrap_or("").to_string();
    Poll {
        alive,
        exit_code,
        log,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_script_decodes_the_command_without_shell_splitting() {
        let script = launch_script("bg-1", "/work/proj", "echo 'a b' && sleep 2", &[("FOO".into(), "b'ar".into())]);
        assert!(script.contains("base64 -d"));
        assert!(script.contains("setsid"));
        assert!(!script.contains("echo 'a b'"));
    }

    #[test]
    fn poll_keeps_log_newlines() {
        let parsed = parse_poll("dead\n0\nhello\nworld\n");
        assert!(!parsed.alive);
        assert_eq!(parsed.exit_code, Some(0));
        assert_eq!(parsed.log, "hello\nworld\n");
    }
}
