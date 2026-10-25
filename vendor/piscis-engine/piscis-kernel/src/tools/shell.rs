// Only the Windows shell path uses these; gate them so Linux/macOS release builds (which
// treat warnings as errors) do not fail on unused imports.
#[cfg(windows)]
use super::output::{
    collapse_repeated_lines, extract_error_lines, smart_truncate, TruncateStrategy,
};
use crate::agent::tool::{Tool, ToolContext, ToolResult};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::borrow::Cow;

#[cfg(windows)]
use std::{
    ffi::OsString,
    path::PathBuf,
    time::{Duration, Instant},
};

#[cfg(windows)]
const DEFAULT_TIMEOUT_SECS: u64 = 120;
#[cfg(windows)]
const MAX_TIMEOUT_SECS: u64 = 300;
#[cfg(windows)]
const MAX_OUTPUT_BYTES: usize = 200 * 1024;

/// cmd fixes its output code page when it starts, so `chcp 65001` must run
/// in a parent cmd. The command reaches the child through this variable,
/// expanded (`!…!`) after the parent has split its own line, so the user's
/// quotes and `&` reach the child intact.
#[cfg(windows)]
pub(crate) const CMD_LINE_ENV: &str = "AGENTZ_CMD_LINE";

/// Python writes pipes in the ANSI code page (cp1252 raises on CJK text,
/// cp936 emits GBK) unless told otherwise.
#[cfg(windows)]
pub(crate) fn add_utf8_env_defaults(env: &mut Vec<(std::ffi::OsString, std::ffi::OsString)>) {
    for (key, value) in [("PYTHONUTF8", "1"), ("PYTHONIOENCODING", "utf-8")] {
        if !env.iter().any(|(k, _)| k.eq_ignore_ascii_case(key)) {
            env.push((key.into(), value.into()));
        }
    }
}

#[cfg(windows)]
pub(crate) fn cmd_utf8_args() -> Vec<std::ffi::OsString> {
    [
        "/d",
        "/v:on",
        "/c",
        "chcp 65001 >nul 2>&1 & cmd /d /v:off /s /c \"!AGENTZ_CMD_LINE!\"",
    ]
    .map(std::ffi::OsString::from)
    .to_vec()
}

/// Rewrite top-level unquoted `&&` chains into Windows PowerShell 5.1 compatible
/// conditional blocks so LLM-generated `cmd1 && cmd2` commands do not fail with
/// `The token '&&' is not a valid statement separator in this version`.
pub fn rewrite_powershell5_chain(command: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let bytes = command.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut seg_start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\'' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_single = false;
            }
            i += 1;
            continue;
        }
        if in_double {
            if b == b'`' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_double = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' => in_single = true,
            b'"' => in_double = true,
            b'{' => brace_depth += 1,
            b'}' => brace_depth = brace_depth.saturating_sub(1),
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b'&' if brace_depth == 0
                && paren_depth == 0
                && i + 1 < bytes.len()
                && bytes[i + 1] == b'&' =>
            {
                parts.push(command[seg_start..i].trim());
                i += 2;
                seg_start = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    if parts.is_empty() {
        return command.to_string();
    }
    parts.push(command[seg_start..].trim());
    if parts.iter().any(|p| p.is_empty()) {
        return command.to_string();
    }

    let mut acc = parts.last().unwrap().to_string();
    for seg in parts[..parts.len() - 1].iter().rev() {
        acc = format!(
            "{seg}; if ($? -and ($null -eq $LASTEXITCODE -or $LASTEXITCODE -eq 0)) {{ {acc} }}"
        );
    }
    format!("$global:LASTEXITCODE = 0; {acc}")
}

#[derive(Default)]
pub struct ShellTool {
    pub background_registry: Option<std::sync::Arc<crate::background::BackgroundRegistry>>,
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        #[cfg(windows)]
        return "Execute a Windows shell command inside a Windows AppContainer with filesystem access limited to the selected workspace. Network and isolation follow the user's security level in Settings: strict = AppContainer with no network, balanced = run as the current user with network and cwd pinned to the workspace, dev = sandbox disabled (cwd may leave workspace_root). The working directory must be inside workspace_root unless the sandbox is disabled; by default it is workspace_root. Elevated execution and other platforms are unavailable. Setup or cleanup failures stop execution without an unsandboxed retry. Supports PowerShell, 32-bit PowerShell, and cmd.exe. Windows PowerShell 5.1 drops double quotes inside arguments passed to native programs (python, node, git): write them as \\\" (python -c 'print(\\\"x\\\")') or use code_run, whose cmd syntax passes quotes unchanged. In strict mode, cmd directory listings and PowerShell location/child-process commands fail because ancestors of the workspace are not listable, and cmd built-ins print non-ANSI characters as '?'; use PowerShell, file_list/file_search or .NET APIs instead.";

        #[cfg(not(windows))]
        return "Unavailable: this platform does not have an enabled operating-system command sandbox. Commands are not executed.";
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The command to execute" },
                "interpreter": {
                    "type": "string",
                    "enum": ["powershell", "powershell32", "cmd"],
                    "description": "Windows interpreter to use. Defaults to PowerShell."
                },
                "elevated": {
                    "type": "boolean",
                    "description": "Unsupported. Elevated execution is refused to preserve the AppContainer boundary."
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory inside workspace_root. Defaults to workspace_root."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Foreground timeout in seconds (default 120, maximum 300). On timeout the still-running command is moved to the background registry instead of being killed."
                },
                "background": {
                    "type": "boolean",
                    "description": "Start in the background registry and return immediately with a bg_id. Use for builds, tests and servers that may run long. Manage it with process_wait/process_output/process_kill. Output is captured automatically (process_output, and the user's background panel), so redirecting it to a file is usually unnecessary; redirect only when you need the file itself."
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
                    "description": "Extra environment variables (key-value pairs)."
                }
            },
            "required": ["command"]
        })
    }

    fn description_minimal(&self) -> Cow<'_, str> {
        #[cfg(windows)]
        return Cow::Borrowed("Run a Windows shell command in an AppContainer scoped to workspace_root; network/isolation follow the Settings security level (strict/balanced/dev, dev disables the sandbox). cwd must remain inside that root unless the sandbox is disabled. elevated=true is refused; other platforms do not execute shell commands.");

        #[cfg(not(windows))]
        Cow::Borrowed("Unavailable: shell commands are not executed because an OS sandbox is unavailable on this platform.")
    }

    fn input_schema_minimal(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "interpreter": { "type": "string", "enum": ["powershell", "powershell32", "cmd"] },
                "elevated": { "type": "boolean" },
                "cwd": { "type": "string" },
                "timeout": { "type": "integer", "minimum": 1 },
                "background": { "type": "boolean" },
                "notify_on": { "type": "string", "enum": ["exit", "ready"] },
                "ready_port": { "type": "integer" },
                "ready_log": { "type": "string" },
                "env": { "type": "object" }
            },
            "required": ["command"]
        })
    }

    fn needs_confirmation(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let command = match input["command"].as_str() {
            Some(command) if !command.trim().is_empty() => command,
            _ => return recovery_error("Missing required parameter: command", ctx, false),
        };

        if input["elevated"].as_bool().unwrap_or(false) {
            let report = crate::tool_recovery::RecoveryReport::refused_elevation_before_launch(
                recovery_operation_id(ctx),
            )
            .map_err(anyhow::Error::msg)?;
            return report.attach(ToolResult::err(
                "Elevated shell execution is refused: it is incompatible with the restricted AppContainer policy.",
            )).map_err(anyhow::Error::msg);
        }

        #[cfg(not(windows))]
        {
            // No OS sandbox exists on this platform, so foreground execution
            // stays refused. Explicit background work is the one exception:
            // it runs as the current user (setsid process group) under the
            // registry, with cwd pinned to the workspace unless the sandbox
            // is disabled — the same scoping rule as the Windows runner.
            let bg_params = super::background_process::parse_bg_params(&input);
            if !bg_params.detached {
                let _ = command;
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
            let requested_cwd = match input["cwd"].as_str() {
                Some(cwd) if std::path::Path::new(cwd).is_absolute() => {
                    std::path::PathBuf::from(cwd)
                }
                Some(cwd) => workspace_root.join(cwd),
                None => workspace_root.clone(),
            };
            let sandbox_disabled =
                ctx.settings.sandbox_mode == crate::agent::tool::SandboxMode::Disabled;
            let cwd = match requested_cwd.canonicalize() {
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
            let spec = super::background_process::bg_spec(
                ctx,
                command.chars().take(200).collect::<String>(),
                cwd.to_string_lossy().into_owned(),
                &bg_params,
            );
            let mut cmd = std::process::Command::new("/bin/sh");
            cmd.arg("-c").arg(command).current_dir(&cwd);
            if let Some(env) = input["env"].as_object() {
                for (key, value) in env {
                    if let Some(value) = value.as_str() {
                        cmd.env(key, value);
                    }
                }
            }
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
        {
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
            let requested_cwd = match input["cwd"].as_str() {
                Some(cwd) if PathBuf::from(cwd).is_absolute() => PathBuf::from(cwd),
                Some(cwd) => workspace_root.join(cwd),
                None => workspace_root.clone(),
            };
            let sandbox_disabled =
                ctx.settings.sandbox_mode == crate::agent::tool::SandboxMode::Disabled;
            let cwd = match requested_cwd.canonicalize() {
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

            let interpreter = input["interpreter"].as_str().unwrap_or("powershell");
            let windir = match std::env::var_os("WINDIR") {
                Some(windir) => PathBuf::from(windir),
                None => {
                    return recovery_error(
                        "Cannot locate Windows system tools; refusing unsandboxed fallback.",
                        ctx,
                        false,
                    )
                }
            };
            let (program, args) = match interpreter {
                "cmd" => {
                    let program = windir.join("System32\\cmd.exe");
                    (program, cmd_utf8_args())
                }
                "powershell" | "powershell32" => {
                    let system_dir = if interpreter == "powershell32" {
                        "SysWOW64"
                    } else {
                        "System32"
                    };
                    let program = windir
                        .join(system_dir)
                        .join("WindowsPowerShell\\v1.0\\powershell.exe");
                    let compat_cmd = rewrite_powershell5_chain(command);
                    let full_command = format!(
                        "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; $OutputEncoding=[System.Text.Encoding]::UTF8; {compat_cmd}"
                    );
                    (
                        program,
                        vec![
                            OsString::from("-NoProfile"),
                            OsString::from("-NonInteractive"),
                            OsString::from("-Command"),
                            OsString::from(full_command),
                        ],
                    )
                }
                _ => {
                    return recovery_error(
                        "Unsupported shell interpreter. Choose powershell, powershell32, or cmd.",
                        ctx,
                        false,
                    )
                }
            };

            let timeout_secs = input["timeout"]
                .as_u64()
                .unwrap_or(DEFAULT_TIMEOUT_SECS)
                .clamp(1, MAX_TIMEOUT_SECS);
            let mut env_overrides = input["env"]
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
            add_utf8_env_defaults(&mut env_overrides);
            if interpreter == "cmd" {
                env_overrides.push((OsString::from(CMD_LINE_ENV), OsString::from(command)));
            }
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
            let worker_cwd = cwd.clone();
            let started = Instant::now();
            let receipt = match ctx.owned_completion().register() {
                Ok(receipt) => receipt,
                Err(error) => return recovery_error(error, ctx, false),
            };
            let (tx, rx) = tokio::sync::oneshot::channel();
            let spawn_res = std::thread::Builder::new()
                .name("shell-sandbox".to_string())
                .stack_size(8 * 1024 * 1024)
                .spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        match bg_registry {
                            Some(registry) => crate::proc::windows_sandbox::run_sandboxed_command_managed(
                                program.as_os_str(),
                                &args,
                                &worker_workspace_root,
                                &worker_cwd,
                                timeout,
                                cancel,
                                &env_overrides,
                                MAX_OUTPUT_BYTES,
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
                                &worker_cwd,
                                timeout,
                                cancel,
                                &env_overrides,
                                MAX_OUTPUT_BYTES,
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
            let result = rx.await;
            let duration_ms = started.elapsed().as_millis();
            if let Ok(Ok(crate::proc::windows_sandbox::ManagedSandboxed::Detached {
                entry,
                stdout_so_far,
                stderr_so_far,
            })) = &result
            {
                return Ok(super::background_process::detached_result(
                    entry,
                    bg_detached,
                    stdout_so_far,
                    stderr_so_far,
                ));
            }
            let output = match result {
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
            let mut report = output.recovery_report(recovery_operation_id(ctx))?;
            if output.timed_out {
                return report.attach(ToolResult::err(format!(
                    "Command timed out after {timeout_secs}s; the AppContainer Job Object was terminated."
                ))).map_err(anyhow::Error::msg);
            }
            if output.cancelled {
                return report
                    .attach(ToolResult::err(
                        "Command was cancelled; the AppContainer Job Object was terminated.",
                    ))
                    .map_err(anyhow::Error::msg);
            }

            // Presentation budget is deliberately far below the capture
            // cap: long lines are elided, whole lines head/tailed, and the
            // full output spills to disk (named below) when truncated.
            let stdout_raw = super::output::decode_process_output(&output.stdout);
            let stderr_raw = super::output::decode_process_output(&output.stderr);
            let (stdout, stdout_trimmed) = truncate_output(&stdout_raw, 24 * 1024);
            let (stderr, stderr_trimmed) = truncate_output(&stderr_raw, 12 * 1024);
            let collapsed_stdout = collapse_repeated_lines(&stdout);
            let collapsed_stderr = collapse_repeated_lines(&stderr);
            report.note_presentation_truncation(
                stdout_trimmed || collapsed_stdout != stdout,
                stderr_trimmed || collapsed_stderr != stderr,
            );
            let stdout = collapsed_stdout;
            let stderr = collapsed_stderr;
            let mut parts = vec![format!(
                "[exit: {}] [duration: {}ms]",
                output.exit_code, duration_ms
            )];
            if !stdout.is_empty() {
                parts.push(format!("STDOUT:\n{stdout}"));
            }
            if !stderr.is_empty() {
                parts.push(format!("STDERR:\n{stderr}"));
            }
            if stdout.is_empty() && stderr.is_empty() {
                parts.push("(no output)".to_string());
            }
            if stdout_trimmed || stderr_trimmed {
                if let Some(path) = crate::tools::output::spill_tool_output(
                    &workspace_root,
                    "shell",
                    &stdout_raw,
                    &stderr_raw,
                ) {
                    parts.push(format!(
                        "Full output saved to {} — inspect omitted parts with file_read (offset/limit) or file_search instead of re-running.",
                        crate::path_text::display_path(&path)
                    ));
                }
            }
            if output.exit_code != 0 {
                let errors = extract_error_lines(&stdout, &stderr);
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

#[cfg(windows)]
fn truncate_output(s: &str, max_bytes: usize) -> (String, bool) {
    let strategy = if s.contains("error") || s.contains("Error") || s.contains("FAILED") {
        TruncateStrategy::ErrorAware
    } else {
        TruncateStrategy::HeadTail
    };
    let (body, meta) = smart_truncate(s.trim(), max_bytes, strategy);
    (body, meta.truncated)
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
    fn rewrite_powershell5_chain_converts_top_level_and_and_only() {
        let rewritten = rewrite_powershell5_chain("Set-Location src && cargo check");
        assert!(rewritten.contains("Set-Location src; if ($?"));
        assert!(rewritten.contains("cargo check"));

        let quoted = "Write-Output \"a && b\"";
        assert_eq!(rewrite_powershell5_chain(quoted), quoted);
    }

    #[test]
    fn shell_discloses_supported_os_sandbox_boundary() {
        let tool = ShellTool::default();
        let description = tool.description();
        if cfg!(windows) {
            assert!(description.contains("Windows AppContainer"));
            assert!(description.contains("security level in Settings"));
            assert!(description.contains("sandbox disabled"));
            assert!(description.contains("Elevated execution"));
        } else {
            assert!(description.contains("Commands are not executed"));
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn shell_uses_appcontainer_and_rejects_out_of_workspace_or_elevation() {
        use crate::agent::tool::ToolSettings;
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let temp = tempfile::tempdir().expect("create shell test root");
        let workspace = temp.path().join("workspace");
        let outside_file = temp.path().join("outside-secret.txt");
        std::fs::create_dir_all(&workspace).expect("create shell workspace");
        std::fs::write(&outside_file, "must-not-leak-shell").expect("write outside canary");
        let ctx = ToolContext {
            session_id: "shell-appcontainer-test".into(),
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

        let result = ShellTool::default()
            .call(
                json!({"command": "echo shell-appcontainer", "interpreter": "cmd", "timeout": 10}),
                &ctx,
            )
            .await
            .expect("call shell");
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("shell-appcontainer"));
        assert!(result.content.contains("\"outcome\":\"completed\""));
        assert!(result
            .content
            .contains("\"termination\":\"tree_terminated\""));

        let outside_read = format!("type \"{}\"", outside_file.display());
        let result = ShellTool::default()
            .call(
                json!({"command": outside_read, "interpreter": "cmd", "timeout": 10}),
                &ctx,
            )
            .await
            .expect("call shell with out-of-scope file read");
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("[exit: 1]"), "{}", result.content);
        assert!(result.content.contains("\"outcome\":\"tool_failed\""));
        assert!(!result.content.contains("must-not-leak-shell"));

        let outside = temp.path().to_string_lossy().to_string();
        let result = ShellTool::default()
            .call(
                json!({"command": "echo must-not-run", "cwd": outside}),
                &ctx,
            )
            .await
            .expect("call shell with out-of-workspace cwd");
        assert!(result.is_error);
        assert!(result.content.contains("inside workspace_root"));
        assert!(result.content.contains("\"termination\":\"not_started\""));

        let result = ShellTool::default()
            .call(
                json!({"command": "echo must-not-run", "elevated": true}),
                &ctx,
            )
            .await
            .expect("call elevated shell");
        assert!(result.is_error);
        assert!(result
            .content
            .contains("Elevated shell execution is refused"));
        assert!(result
            .content
            .contains("\"outcome\":\"elevation_unsupported\""));
    }
}
