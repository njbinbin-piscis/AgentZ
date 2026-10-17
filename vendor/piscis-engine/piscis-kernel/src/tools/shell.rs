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

pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        #[cfg(windows)]
        return "Execute a Windows shell command inside a Windows AppContainer with filesystem access limited to the selected workspace and no network capability. The working directory must be inside workspace_root; by default it is workspace_root. Elevated execution and other platforms are unavailable. Setup or cleanup failures stop execution without an unsandboxed retry. Supports PowerShell, 32-bit PowerShell, and cmd.exe.";

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
                    "description": "Timeout in seconds (default 120, maximum 300)."
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
        return Cow::Borrowed("Run a Windows shell command in a no-network AppContainer scoped to workspace_root. cwd must remain inside that root. elevated=true is refused; other platforms do not execute shell commands.");

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
            let _ = (command, ctx);
            return recovery_error(
                "Command execution refused: operating-system containment is unavailable on this platform.", ctx, false);
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
            let cwd = match requested_cwd.canonicalize() {
                Ok(cwd) if cwd.is_dir() && cwd.starts_with(&workspace_root) => cwd,
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
                    let full_command = format!("chcp 65001 >nul 2>&1 & {command}");
                    (
                        program,
                        vec![
                            OsString::from("/d"),
                            OsString::from("/c"),
                            OsString::from(full_command),
                        ],
                    )
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
                    let full_command = format!(
                        "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; $OutputEncoding=[System.Text.Encoding]::UTF8; {command}"
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
            let worker_workspace_root = workspace_root.clone();
            let worker_cwd = cwd.clone();
            let started = Instant::now();
            let receipt = match ctx.owned_completion().register() {
                Ok(receipt) => receipt,
                Err(error) => return recovery_error(error, ctx, false),
            };
            let result = tokio::task::spawn_blocking(move || {
                let result = crate::proc::windows_sandbox::run_appcontainer_command(
                    program.as_os_str(),
                    &args,
                    &worker_workspace_root,
                    &worker_cwd,
                    timeout,
                    cancel,
                    &env_overrides,
                    MAX_OUTPUT_BYTES,
                );
                receipt.complete(
                    result
                        .as_ref()
                        .map(|_| ())
                        .map_err(|_| "sandbox cleanup unconfirmed".into()),
                );
                result
            })
            .await;
            let duration_ms = started.elapsed().as_millis();
            let output = match result {
                Ok(Ok(output)) => output,
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

            let (stdout, stdout_trimmed) = truncate_output(
                &String::from_utf8_lossy(&output.stdout),
                MAX_OUTPUT_BYTES * 3 / 4,
            );
            let (stderr, stderr_trimmed) = truncate_output(
                &String::from_utf8_lossy(&output.stderr),
                MAX_OUTPUT_BYTES / 4,
            );
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
    fn shell_discloses_supported_os_sandbox_boundary() {
        let description = ShellTool.description();
        if cfg!(windows) {
            assert!(description.contains("Windows AppContainer"));
            assert!(description.contains("no network capability"));
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
            settings: Arc::new(ToolSettings::default()),
            max_iterations: None,
            memory_owner_id: "test".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        };

        let result = ShellTool
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
        let result = ShellTool
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
        let result = ShellTool
            .call(
                json!({"command": "echo must-not-run", "cwd": outside}),
                &ctx,
            )
            .await
            .expect("call shell with out-of-workspace cwd");
        assert!(result.is_error);
        assert!(result.content.contains("inside workspace_root"));
        assert!(result.content.contains("\"termination\":\"not_started\""));

        let result = ShellTool
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
