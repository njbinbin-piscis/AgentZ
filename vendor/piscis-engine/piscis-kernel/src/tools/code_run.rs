#[cfg(windows)]
use super::output::extract_error_lines;
use crate::agent::tool::{Tool, ToolContext, ToolResult};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT_SECS: u64 = 60;
const MAX_TIMEOUT_SECS: u64 = 300;
/// Keep head + tail of each stream; long build logs are rarely useful in full.
#[cfg(windows)]
const MAX_STREAM_BYTES: usize = 8 * 1024; // 8 KB per stream

#[derive(Default)]
pub struct CodeRunTool {
    pub background_registry: Option<std::sync::Arc<crate::background::BackgroundRegistry>>,
}

#[async_trait]
impl Tool for CodeRunTool {
    fn name(&self) -> &str {
        "code_run"
    }

    fn description(&self) -> &str {
        #[cfg(windows)]
        return "Run commands inside a Windows AppContainer with access limited to the selected workspace. Network and isolation follow the user's security level in Settings: strict = AppContainer with no network, balanced = run as the current user with network and cwd pinned to the workspace, dev = sandbox disabled. Other platforms do not execute this tool because an OS sandbox is unavailable.";

        #[cfg(not(windows))]
        return "Unavailable: this platform does not have an enabled operating-system command sandbox. Commands are not executed.";

        #[allow(unreachable_code)]
        "Run a shell command in a project directory and capture its output as the current OS user. \
         Operating-system containment is not enabled; approval and workspace scoping do not block access to other resources. \
         Designed for coding tasks: building, testing, linting, running scripts. \
         Returns exit_code, stdout, stderr, and duration_ms. \
         Use `cwd` to set the project root (e.g. where Cargo.toml / package.json lives). \
         Examples: \
         - Build Rust: command=\"cargo build\", cwd=\"C:\\\\myproject\" \
         - Run tests:  command=\"cargo test\", cwd=\"C:\\\\myproject\" \
         - Python:     command=\"python main.py\", cwd=\"C:\\\\myproject\" \
         - Node:       command=\"npm run build\", cwd=\"C:\\\\myproject\" \
         - Lint:       command=\"cargo clippy -- -D warnings\", cwd=\"C:\\\\myproject\" \
         Prefer this over `shell` for code execution — output is structured and \
         long outputs are automatically trimmed to keep context window usage low."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command to run (e.g. \"cargo test\", \"python main.py\", \"npm run build\")"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory — should be the project root. Defaults to workspace_root."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Foreground timeout in seconds (default 60, max 300). On timeout the still-running command is moved to the background registry instead of being killed."
                },
                "background": {
                    "type": "boolean",
                    "description": "Start in the background registry and return immediately with a bg_id. Use for slow builds/tests and servers. Manage it with process_wait/process_output/process_kill."
                },
                "notify_on": {
                    "type": "string",
                    "enum": ["exit", "ready"],
                    "description": "For background tasks: notify the session on exit (default) or when the readiness condition (ready_port/ready_log) first fires."
                },
                "ready_port": {
                    "type": "integer",
                    "description": "Readiness condition: a TCP connect to 127.0.0.1 on this port succeeds (dev servers)."
                },
                "ready_log": {
                    "type": "string",
                    "description": "Readiness condition: output contains this substring (dev servers)."
                },
                "env": {
                    "type": "object",
                    "description": "Extra environment variables (key-value string pairs)"
                }
            },
            "required": ["command"]
        })
    }

    fn needs_confirmation(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let command = match input["command"].as_str() {
            Some(c) if !c.trim().is_empty() => c,
            _ => return recovery_error("Missing required parameter: command", ctx, false),
        };

        let cwd = if let Some(cwd_str) = input["cwd"].as_str() {
            if std::path::Path::new(cwd_str).is_absolute() {
                std::path::PathBuf::from(cwd_str)
            } else {
                ctx.workspace_root.join(cwd_str)
            }
        } else {
            ctx.workspace_root.clone()
        };

        if !cwd.exists() {
            return recovery_error(
                format!("Working directory does not exist: {}", cwd.display()),
                ctx,
                false,
            );
        }

        let timeout_secs = input["timeout_secs"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .min(MAX_TIMEOUT_SECS);

        #[cfg(not(windows))]
        {
            // See shell.rs: foreground stays refused without an OS sandbox;
            // explicit background work runs registered via /bin/sh.
            let bg_params = super::background_process::parse_bg_params(&input);
            if !bg_params.detached {
                let _ = (command, cwd, timeout_secs, ctx);
                return recovery_error(
                    "Command execution refused: operating-system containment is unavailable on this platform.", ctx, false);
            }
            let Some(registry) = self.background_registry.clone() else {
                return recovery_error(
                    "Background execution is not configured by this host; run the command in the foreground instead.",
                    ctx,
                    false,
                );
            };
            let spec = super::background_process::bg_spec(
                ctx,
                command.chars().take(200).collect::<String>(),
                cwd.to_string_lossy().into_owned(),
                &bg_params,
            );
            let mut cmd = std::process::Command::new("/bin/sh");
            cmd.arg("-c").arg(command).current_dir(&cwd);
            return match super::background_process::spawn_command_detached(&registry, spec, cmd) {
                Ok(entry) => Ok(super::background_process::detached_result(
                    &entry,
                    true,
                    &[],
                    &[],
                )),
                Err(error) => recovery_error(error, ctx, false),
            };
        }

        #[cfg(windows)]
        let (exit_code, stdout_bytes, stderr_bytes, duration_ms, mut report) = {
            let workspace_root = match ctx.workspace_root.canonicalize() {
                Ok(root) => root,
                Err(error) => {
                    return recovery_error(
                        format!("Sandbox workspace is unavailable: {error}"),
                        ctx,
                        false,
                    )
                }
            };
            let sandbox_disabled =
                ctx.settings.sandbox_mode == crate::agent::tool::SandboxMode::Disabled;
            let cwd = match cwd.canonicalize() {
                Ok(cwd)
                    if cwd.is_dir()
                        && (sandbox_disabled || cwd.starts_with(&workspace_root)) =>
                {
                    cwd
                }
                Ok(_) => {
                    return recovery_error(
                        "Sandbox working directory must be an existing directory inside workspace_root.", ctx, false)
                }
                Err(error) => {
                    return recovery_error(format!(
                        "Sandbox working directory is unavailable: {error}"
                    ), ctx, false)
                }
            };
            let program = match std::env::var_os("WINDIR") {
                Some(windir) => PathBuf::from(windir).join("System32\\cmd.exe"),
                None => return recovery_error(
                    "Cannot locate the Windows command interpreter; refusing unsandboxed fallback.",
                    ctx,
                    false,
                ),
            };
            let full_command = format!("chcp 65001 >nul 2>&1 & {command}");
            let args = [
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from(full_command),
            ];
            let env_overrides = input["env"]
                .as_object()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|(key, value)| {
                            value
                                .as_str()
                                .map(|value| (OsString::from(key), OsString::from(value)))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let timeout = Duration::from_secs(timeout_secs);
            let cancel = ctx.cancel.clone();
            let sandbox_mode = ctx.settings.sandbox_mode;
            let bg_params = super::background_process::parse_bg_params(&input);
            let bg_registry = self.background_registry.clone();
            if bg_params.detached && bg_registry.is_none() {
                return recovery_error(
                    "Background execution is not configured by this host; run the command in the foreground instead.",
                    ctx,
                    false,
                );
            }
            let bg_spec = super::background_process::bg_spec(
                ctx,
                command.chars().take(200).collect::<String>(),
                cwd.to_string_lossy().into_owned(),
                &bg_params,
            );
            let bg_detached = bg_params.detached;
            let worker_workspace_root = workspace_root.clone();
            let start = Instant::now();
            let receipt = match ctx.owned_completion().register() {
                Ok(receipt) => receipt,
                Err(error) => return recovery_error(error, ctx, false),
            };
            let (tx, rx) = tokio::sync::oneshot::channel();
            let spawn_res = std::thread::Builder::new()
                .name("code-run-sandbox".to_string())
                .stack_size(8 * 1024 * 1024)
                .spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        match bg_registry {
                            Some(registry) => crate::proc::windows_sandbox::run_sandboxed_command_managed(
                                program.as_os_str(),
                                &args,
                                &worker_workspace_root,
                                &cwd,
                                timeout,
                                cancel,
                                &env_overrides,
                                MAX_STREAM_BYTES,
                                sandbox_mode,
                                crate::proc::windows_sandbox::ManagedBackground {
                                    registry,
                                    spec: bg_spec,
                                    detached: bg_detached,
                                },
                            ),
                            None => crate::proc::windows_sandbox::run_sandboxed_command(
                                program.as_os_str(),
                                &args,
                                &worker_workspace_root,
                                &cwd,
                                timeout,
                                cancel,
                                &env_overrides,
                                MAX_STREAM_BYTES,
                                sandbox_mode,
                            )
                            .map(crate::proc::windows_sandbox::ManagedSandboxed::Completed),
                        }
                    }))
                    .unwrap_or_else(|_| {
                        Err(anyhow::anyhow!("sandbox worker panicked unexpectedly"))
                    });
                    receipt.complete(
                        result
                            .as_ref()
                            .map(|_| ())
                            .map_err(|_| "sandbox cleanup unconfirmed".into()),
                    );
                    let _ = tx.send(result);
                });
            if let Err(error) = spawn_res {
                return recovery_error(
                    format!("Failed to spawn sandbox worker thread: {error}"),
                    ctx,
                    false,
                );
            }
            let run_result = rx.await;
            let duration_ms = start.elapsed().as_millis() as u64;
            if let Ok(Ok(crate::proc::windows_sandbox::ManagedSandboxed::Detached {
                entry,
                stdout_so_far,
                stderr_so_far,
            })) = &run_result
            {
                return Ok(super::background_process::detached_result(
                    entry,
                    bg_detached,
                    stdout_so_far,
                    stderr_so_far,
                ));
            }
            let output = match run_result {
                Ok(Ok(crate::proc::windows_sandbox::ManagedSandboxed::Completed(output))) => output,
                Ok(Ok(crate::proc::windows_sandbox::ManagedSandboxed::Detached { .. })) => {
                    unreachable!("detached handled above")
                }
                Ok(Err(error)) => {
                    return recovery_error(format!(
                        "Sandbox setup or cleanup failed; command was not retried outside the sandbox: {error:#}"
                    ), ctx, true)
                }
                Err(error) => {
                    return recovery_error(format!(
                        "Sandbox command worker failed: {error}"
                    ), ctx, true)
                }
            };
            let report = output.recovery_report(recovery_operation_id(ctx))?;
            if output.timed_out {
                return report.attach(ToolResult::err(format!(
                    "Sandbox command timed out after {timeout_secs}s; the Job Object was terminated."
                ))).map_err(anyhow::Error::msg);
            }
            if output.cancelled {
                return report
                    .attach(ToolResult::err(
                        "Sandbox command was cancelled; the Job Object was terminated.",
                    ))
                    .map_err(anyhow::Error::msg);
            }
            (
                output.exit_code,
                output.stdout,
                output.stderr,
                duration_ms,
                report,
            )
        };

        #[cfg(windows)]
        {
            let stdout_raw = String::from_utf8_lossy(&stdout_bytes);
            let stderr_raw = String::from_utf8_lossy(&stderr_bytes);
            let stdout = trim_stream(&stdout_raw, MAX_STREAM_BYTES);
            let stderr = trim_stream(&stderr_raw, MAX_STREAM_BYTES);
            report.note_presentation_truncation(
                stdout_raw.trim().len() > MAX_STREAM_BYTES,
                stderr_raw.trim().len() > MAX_STREAM_BYTES,
            );
            let status_label = if exit_code == 0 { "SUCCESS" } else { "FAILED" };
            let mut parts = vec![format!(
                "exit_code: {} ({})  duration: {}ms  cwd: {}",
                exit_code,
                status_label,
                duration_ms,
                cwd.display()
            )];
            if !stdout.is_empty() {
                parts.push(format!("--- stdout ---\n{}", stdout));
            }
            if !stderr.is_empty() {
                parts.push(format!("--- stderr ---\n{}", stderr));
            }
            if stdout.is_empty() && stderr.is_empty() {
                parts.push("(no output)".to_string());
            }
            if exit_code != 0 {
                let combined = format!("{}{}", stdout_raw, stderr_raw);
                if let Some(hint) = diagnose(&combined, command) {
                    parts.push(format!("--- diagnosis ---\n{}", hint));
                }
                let errors = extract_error_lines(&stdout_raw, &stderr_raw);
                if !errors.is_empty() {
                    parts.push(format!("--- errors ---\n{}", errors.join("\n")));
                }
            }
            report
                .attach(ToolResult::ok(parts.join("\n\n")))
                .map_err(anyhow::Error::msg)
        }
    }
}

/// Trim a stream to at most `max_bytes`, keeping head and tail.
#[cfg(windows)]
fn trim_stream(s: &str, max_bytes: usize) -> String {
    let s = s.trim();
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let head_bytes = max_bytes * 3 / 4;
    let tail_bytes = max_bytes - head_bytes;
    // Find valid char boundaries
    let head_end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= head_bytes)
        .last()
        .unwrap_or(0);
    let tail_start = s
        .char_indices()
        .rev()
        .map(|(i, _)| i)
        .take_while(|&i| s.len() - i <= tail_bytes)
        .last()
        .unwrap_or(s.len());
    format!(
        "{}\n\n... [{} bytes trimmed — use file_read or scroll output for full log] ...\n\n{}",
        &s[..head_end],
        s.len() - max_bytes,
        &s[tail_start..]
    )
}

/// Heuristic diagnosis for common build/test failure patterns.
#[cfg(windows)]
fn diagnose(output: &str, command: &str) -> Option<String> {
    let out_lower = output.to_lowercase();
    let cmd_lower = command.to_lowercase();

    // Rust / Cargo
    if cmd_lower.contains("cargo") {
        if out_lower.contains("error[e") {
            let error_count = output.matches("error[E").count() + output.matches("error[e").count();
            return Some(format!(
                "Rust compilation failed with ~{} error(s). \
                 Fix each `error[Exxxx]` in order — later errors often cascade from earlier ones.",
                error_count
            ));
        }
        if out_lower.contains("test failed") || out_lower.contains("failures:") {
            let failed = output
                .lines()
                .filter(|l| l.trim_start().starts_with("FAILED"))
                .count();
            return Some(format!(
                "{} test(s) failed. Check the `failures:` section above for panic messages and expected vs actual values.",
                failed.max(1)
            ));
        }
        if out_lower.contains("warning:") && !out_lower.contains("error") {
            return Some(
                "Build succeeded with warnings. Run `cargo clippy` for detailed lint suggestions."
                    .to_string(),
            );
        }
    }

    // Python
    if cmd_lower.contains("python") || cmd_lower.contains("pytest") || cmd_lower.contains("pip") {
        if out_lower.contains("syntaxerror") {
            return Some(
                "Python SyntaxError detected. Check the file and line number shown above."
                    .to_string(),
            );
        }
        if out_lower.contains("modulenotfounderror") || out_lower.contains("importerror") {
            return Some("Missing Python module. Run `pip install <module>` or check your virtual environment is activated.".to_string());
        }
        if out_lower.contains("failed") && cmd_lower.contains("pytest") {
            return Some(
                "pytest failures detected. Check the FAILED lines and assertion errors above."
                    .to_string(),
            );
        }
    }

    // Node / npm
    if cmd_lower.contains("npm") || cmd_lower.contains("node") || cmd_lower.contains("yarn") {
        if out_lower.contains("err!") || out_lower.contains("npm error") {
            return Some("npm error detected. Check for missing dependencies (`npm install`) or script errors above.".to_string());
        }
        if out_lower.contains("typeerror") || out_lower.contains("syntaxerror") {
            return Some(
                "JavaScript/TypeScript error detected. Check the file and line shown above."
                    .to_string(),
            );
        }
    }

    // Generic
    if out_lower.contains("permission denied") || out_lower.contains("access is denied") {
        return Some(
            "Permission denied. Check workspace access and the sandbox capability; elevated execution is unavailable."
                .to_string(),
        );
    }
    if out_lower.contains("command not found") || out_lower.contains("is not recognized") {
        return Some(
            "Command not found. Ensure the tool is installed and on PATH, or use an absolute path."
                .to_string(),
        );
    }

    None
}

fn recovery_operation_id(ctx: &ToolContext) -> String {
    ctx.tool_use_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

fn recovery_error(
    content: impl Into<String>,
    ctx: &ToolContext,
    launched: bool,
) -> Result<ToolResult> {
    use crate::tool_recovery::RecoveryReport;
    let report = if launched {
        RecoveryReport::unknown_failure(recovery_operation_id(ctx))
    } else {
        RecoveryReport::refused_before_launch(recovery_operation_id(ctx))
    }
    .map_err(anyhow::Error::msg)?;
    report
        .attach(ToolResult::err(content))
        .map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_run_discloses_supported_os_sandbox_boundary() {
        let tool = CodeRunTool::default();
        let description = tool.description();
        if cfg!(windows) {
            assert!(description.contains("Windows AppContainer"));
            assert!(description.contains("security level in Settings"));
            assert!(description.contains("sandbox disabled"));
        } else {
            assert!(description.contains("Commands are not executed"));
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn code_run_uses_appcontainer_and_rejects_out_of_workspace_cwd() {
        use crate::agent::tool::ToolSettings;
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let temp = tempfile::tempdir().expect("create tool workspace");
        let workspace = temp.path().join("workspace");
        let outside_file = temp.path().join("outside-secret.txt");
        std::fs::create_dir_all(&workspace).expect("create tool workspace root");
        std::fs::write(&outside_file, "must-not-leak-code-run").expect("write outside canary");
        let ctx = ToolContext {
            session_id: "code-run-appcontainer-test".into(),
            workspace_root: workspace,
            bypass_permissions: false,
            settings: Arc::new(ToolSettings {
                sandbox_mode: crate::agent::tool::SandboxMode::Isolated,
                ..ToolSettings::default()
            }),
            max_iterations: None,
            memory_owner_id: "test".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        };
        let result = CodeRunTool::default()
            .call(
                json!({"command": "echo code-run-appcontainer", "timeout_secs": 10}),
                &ctx,
            )
            .await
            .expect("call code_run");
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("code-run-appcontainer"));
        assert!(result.content.contains("\"outcome\":\"completed\""));
        assert!(result
            .content
            .contains("\"termination\":\"tree_terminated\""));

        let outside_read = format!("type \"{}\"", outside_file.display());
        let result = CodeRunTool::default()
            .call(json!({"command": outside_read, "timeout_secs": 10}), &ctx)
            .await
            .expect("call code_run with out-of-scope file read");
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("exit_code: 1 (FAILED)"),
            "{}",
            result.content
        );
        assert!(!result.content.contains("must-not-leak-code-run"));
        assert!(result.content.contains("\"outcome\":\"tool_failed\""));

        let outside = temp.path().to_string_lossy().to_string();
        let result = CodeRunTool::default()
            .call(
                json!({"command": "echo must-not-run", "cwd": outside}),
                &ctx,
            )
            .await
            .expect("call code_run with out-of-scope cwd");
        assert!(result.is_error);
        assert!(result.content.contains("inside workspace_root"));
        assert!(result.content.contains("\"termination\":\"not_started\""));
    }
}
