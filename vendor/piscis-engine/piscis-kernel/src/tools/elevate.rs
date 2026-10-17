/// Cross-platform elevated command execution helpers.
///
/// Windows: ShellExecute `runas` triggers the native UAC consent dialog.
/// macOS: AppleScript `do shell script ... with administrator privileges`
/// opens the system admin-password prompt.
/// Linux: `pkexec` asks polkit to show an authentication dialog; availability
/// depends on the desktop environment / installed polkit agent.
use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;
#[cfg(target_os = "windows")]
use tokio::time::sleep;
use tokio::time::timeout;

#[cfg(target_os = "linux")]
use crate::proc::std_command;
#[cfg(not(target_os = "windows"))]
use crate::proc::tokio_command;
#[cfg(target_os = "windows")]
use base64::Engine as _;
#[cfg(target_os = "windows")]
use serde::Serialize;
#[cfg(target_os = "windows")]
use std::path::Path;

#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::ShellExecuteW;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

pub struct ElevatedResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[cfg(target_os = "windows")]
#[derive(Serialize)]
struct WindowsElevationEnvironment<'a> {
    key: &'a str,
    value: &'a str,
}

#[cfg(target_os = "windows")]
#[derive(Serialize)]
struct WindowsElevationContext<'a> {
    cwd: &'a str,
    env: Vec<WindowsElevationEnvironment<'a>>,
}

#[cfg(target_os = "windows")]
struct WindowsElevationPayload {
    context_json: Vec<u8>,
    inner_script: String,
}

#[cfg(target_os = "windows")]
fn build_windows_elevation_payload(
    command: &str,
    cwd: &Path,
    env: &[(String, String)],
    context_path: &Path,
) -> Result<WindowsElevationPayload> {
    let cwd = cwd
        .to_str()
        .filter(|value| !value.contains('\0'))
        .ok_or_else(|| anyhow::anyhow!("Invalid Windows elevation working directory"))?;
    let context_path = context_path
        .to_str()
        .filter(|value| !value.contains('\0'))
        .ok_or_else(|| anyhow::anyhow!("Invalid Windows elevation context path"))?;
    if env.iter().any(|(key, value)| {
        key.is_empty() || key.contains('=') || key.contains('\0') || value.contains('\0')
    }) {
        return Err(anyhow::anyhow!("Invalid Windows elevation environment"));
    }

    let context = WindowsElevationContext {
        cwd,
        env: env
            .iter()
            .map(|(key, value)| WindowsElevationEnvironment { key, value })
            .collect(),
    };
    let context_json = serde_json::to_vec(&context)?;
    let encoded_context_path =
        base64::engine::general_purpose::STANDARD.encode(context_path.as_bytes());
    let inner_script = format!(
        "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8\n\
$OutputEncoding=[System.Text.Encoding]::UTF8\n\
chcp 65001 | Out-Null\n\
$contextPath=[System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('{encoded_context_path}'))\n\
$contextJson=[System.IO.File]::ReadAllText($contextPath,[System.Text.Encoding]::UTF8)\n\
$context=$contextJson | ConvertFrom-Json\n\
$workingDirectory=[string]$context.cwd\n\
[System.Environment]::CurrentDirectory=$workingDirectory\n\
Set-Location -LiteralPath $workingDirectory\n\
Add-Type -TypeDefinition @'\n\
using System;\n\
using System.ComponentModel;\n\
using System.Runtime.InteropServices;\n\
\n\
public static class PiscisNativeEnvironment\n\
{{\n\
    [DllImport(\"kernel32.dll\", CharSet = CharSet.Unicode, SetLastError = true)]\n\
    [return: MarshalAs(UnmanagedType.Bool)]\n\
    private static extern bool SetEnvironmentVariableW(string name, string value);\n\
\n\
    public static void Set(string name, string value)\n\
    {{\n\
        if (!SetEnvironmentVariableW(name, value))\n\
        {{\n\
            throw new Win32Exception(Marshal.GetLastWin32Error());\n\
        }}\n\
    }}\n\
}}\n\
'@\n\
foreach($entry in @($context.env)){{[PiscisNativeEnvironment]::Set([string]$entry.key,[string]$entry.value)}}\n\
{command}\n"
    );

    Ok(WindowsElevationPayload {
        context_json,
        inner_script,
    })
}

#[cfg(target_os = "windows")]
fn write_windows_elevation_context(context_path: &Path, context_json: &[u8]) -> Result<()> {
    write_windows_elevation_context_with(context_path, context_json, |path, contents| {
        std::fs::write(path, contents)
    })
}

#[cfg(target_os = "windows")]
fn write_windows_elevation_context_with<W>(
    context_path: &Path,
    context_json: &[u8],
    writer: W,
) -> Result<()>
where
    W: FnOnce(&Path, &[u8]) -> std::io::Result<()>,
{
    if writer(context_path, context_json).is_err() {
        let _ = std::fs::remove_file(context_path);
        return Err(anyhow::anyhow!("Failed to write Windows elevation context"));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
struct ElevatedPaths {
    script_path: PathBuf,
    result_path: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

/// Run a PowerShell command with administrator privileges via UAC.
/// Returns the output after the elevated process completes.
/// `timeout_secs` includes the time the user takes to respond to the UAC dialog.
#[cfg(target_os = "windows")]
pub async fn run_elevated_powershell(
    command: &str,
    arch: &str,
    cwd: &Path,
    env: &[(String, String)],
    timeout_secs: u64,
) -> Result<ElevatedResult> {
    let tmp_dir = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let script_path = tmp_dir.join(format!("piscis_elev_{}.ps1", id));
    let result_path = tmp_dir.join(format!("piscis_elev_{}.result", id));
    let context_path = tmp_dir.join(format!("piscis_elev_{}.context.json", id));

    // Write the wrapper script that captures output and writes to result file.
    //
    // Key design decisions:
    // 1. Write the user command to a separate inner script file, then run it via
    //    Start-Process with stdout/stderr redirected to temp files. This correctly
    //    captures $LASTEXITCODE from native executables (regsvr32, reg, etc.) that
    //    the & { } 2>&1 approach loses.
    // 2. Write result with UTF8NoBOM (New-Object System.Text.UTF8Encoding($false))
    //    to avoid the BOM that Windows [System.Text.Encoding]::UTF8 emits by default,
    //    which would cause serde_json to fail with "expected value at line 1 column 1".
    let result_path_escaped = result_path.to_string_lossy().replace('\\', "\\\\");
    let inner_script_path = tmp_dir.join(format!("piscis_elev_{}_inner.ps1", id));
    let inner_script_path_escaped = inner_script_path.to_string_lossy().replace('\\', "\\\\");

    // Keep cwd and environment as JSON data. The inner script receives only a
    // Base64-encoded context-file path, never executable interpolation of the
    // context contents.
    let payload = build_windows_elevation_payload(command, cwd, env, &context_path)?;
    write_windows_elevation_context(&context_path, &payload.context_json)?;
    if let Err(error) = std::fs::write(&inner_script_path, payload.inner_script.as_bytes()) {
        let _ = std::fs::remove_file(&context_path);
        return Err(error.into());
    }

    // Use the same PowerShell bitness for the inner process as requested by the caller
    let inner_ps_exe = if arch == "x86" {
        r"C:\Windows\SysWOW64\WindowsPowerShell\v1.0\powershell.exe"
    } else {
        "powershell.exe"
    };

    let script_content = format!(
        r#"
$tmpOut = [System.IO.Path]::GetTempFileName()
$tmpErr = [System.IO.Path]::GetTempFileName()
$exitCode = 0

try {{
    $proc = Start-Process -FilePath "{inner_ps_exe}" `
        -ArgumentList @("-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", "{inner_script_path_escaped}") `
        -RedirectStandardOutput $tmpOut `
        -RedirectStandardError $tmpErr `
        -Wait -PassThru -NoNewWindow
    $exitCode = if ($proc.ExitCode -ne $null) {{ $proc.ExitCode }} else {{ 0 }}
}} catch {{
    $exitCode = 1
    $utf8NoBom2 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($tmpErr, $_.Exception.Message, $utf8NoBom2)
}}

$stdout = if (Test-Path $tmpOut) {{ [System.IO.File]::ReadAllText($tmpOut, [System.Text.Encoding]::UTF8).Trim() }} else {{ "" }}
$stderr = if (Test-Path $tmpErr) {{ [System.IO.File]::ReadAllText($tmpErr, [System.Text.Encoding]::UTF8).Trim() }} else {{ "" }}
Remove-Item $tmpOut, $tmpErr, "{inner_script_path_escaped}" -ErrorAction SilentlyContinue

$output = [PSCustomObject]@{{
    exit_code = [int]$exitCode
    stdout = $stdout
    stderr = $stderr
}} | ConvertTo-Json -Compress

$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText("{result_path_escaped}", $output, $utf8NoBom)
"#,
        inner_ps_exe = inner_ps_exe,
        inner_script_path_escaped = inner_script_path_escaped,
        result_path_escaped = result_path_escaped
    );

    if let Err(error) = std::fs::write(&script_path, script_content.as_bytes()) {
        let _ = std::fs::remove_file(&inner_script_path);
        let _ = std::fs::remove_file(&context_path);
        return Err(error.into());
    }

    // Launch elevated via ShellExecuteW runas
    let launch_result = {
        let ps_exe = if arch == "x86" {
            r"C:\Windows\SysWOW64\WindowsPowerShell\v1.0\powershell.exe".to_string()
        } else {
            "powershell.exe".to_string()
        };

        let script_path_str = script_path.to_string_lossy().to_string();
        let ps_args = format!(
            "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\"",
            script_path_str
        );

        launch_elevated_windows(&ps_exe, &ps_args)
    };

    if let Err(e) = launch_result {
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&inner_script_path);
        let _ = std::fs::remove_file(&context_path);
        return Err(e);
    }

    // Poll for result file with timeout
    let poll_result = timeout(
        Duration::from_secs(timeout_secs),
        poll_for_result(&result_path),
    )
    .await;

    // Clean up script files (inner script is also cleaned by the PS script itself,
    // but remove here as a safety net in case the elevated process was killed)
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&inner_script_path);
    let _ = std::fs::remove_file(&context_path);

    match poll_result {
        Err(_) => {
            let _ = std::fs::remove_file(&result_path);
            Err(anyhow::anyhow!(
                "Elevated command timed out after {}s. \
                 The user may have cancelled the UAC dialog, or the command took too long.",
                timeout_secs
            ))
        }
        Ok(Err(e)) => {
            let _ = std::fs::remove_file(&result_path);
            Err(e)
        }
        Ok(Ok(content)) => {
            let _ = std::fs::remove_file(&result_path);
            parse_result(&content)
        }
    }
}

#[cfg(target_os = "macos")]
pub async fn run_elevated_shell(
    command: &str,
    cwd: &std::path::Path,
    env: &[(String, String)],
    timeout_secs: u64,
) -> Result<ElevatedResult> {
    let paths = write_unix_wrapper(command, cwd, env)?;
    let shell_cmd = format!(
        "/bin/sh {}",
        shell_quote(&paths.script_path.to_string_lossy())
    );
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        apple_script_escape(&shell_cmd)
    );

    let result = timeout(
        Duration::from_secs(timeout_secs),
        tokio_command("osascript").args(["-e", &script]).output(),
    )
    .await;

    finalize_unix_result(paths, result, "macOS administrator prompt", timeout_secs)
}

#[cfg(target_os = "linux")]
pub async fn run_elevated_shell(
    command: &str,
    cwd: &std::path::Path,
    env: &[(String, String)],
    timeout_secs: u64,
) -> Result<ElevatedResult> {
    if !command_exists("pkexec") {
        return Err(anyhow::anyhow!(
            "pkexec is not available. Install polkit/pkexec or rerun the command manually with sudo in a terminal."
        ));
    }

    let paths = write_unix_wrapper(command, cwd, env)?;
    let result = timeout(
        Duration::from_secs(timeout_secs),
        tokio_command("pkexec")
            .args(["/bin/sh", &paths.script_path.to_string_lossy()])
            .output(),
    )
    .await;

    finalize_unix_result(paths, result, "polkit authentication", timeout_secs)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub async fn run_elevated_shell(
    _command: &str,
    _cwd: &std::path::Path,
    _env: &[(String, String)],
    _timeout_secs: u64,
) -> Result<ElevatedResult> {
    Err(anyhow::anyhow!(
        "Elevated shell execution is not implemented for this platform"
    ))
}

#[cfg(target_os = "windows")]
async fn poll_for_result(result_path: &PathBuf) -> Result<String> {
    // Poll every 500ms until the result file appears
    loop {
        if result_path.exists() {
            // Small delay to ensure the file write is complete
            sleep(Duration::from_millis(100)).await;
            let content = std::fs::read_to_string(result_path)?;
            if !content.is_empty() {
                return Ok(content);
            }
        }
        sleep(Duration::from_millis(500)).await;
    }
}

#[cfg(not(target_os = "windows"))]
fn write_unix_wrapper(
    command: &str,
    cwd: &std::path::Path,
    env: &[(String, String)],
) -> Result<ElevatedPaths> {
    let tmp_dir = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let script_path = tmp_dir.join(format!("piscis_elev_{}.sh", id));
    let result_path = tmp_dir.join(format!("piscis_elev_{}.exit", id));
    let stdout_path = tmp_dir.join(format!("piscis_elev_{}.out", id));
    let stderr_path = tmp_dir.join(format!("piscis_elev_{}.err", id));

    let exports = env
        .iter()
        .map(|(key, value)| format!("export {}={}\n", key, shell_quote(value)))
        .collect::<String>();

    let script = format!(
        "#!/bin/sh\ncd {} || exit 1\n{} /bin/sh -lc {} > {} 2> {}\nprintf '%s' $? > {}\n",
        shell_quote(&cwd.to_string_lossy()),
        exports,
        shell_quote(command),
        shell_quote(&stdout_path.to_string_lossy()),
        shell_quote(&stderr_path.to_string_lossy()),
        shell_quote(&result_path.to_string_lossy()),
    );

    std::fs::write(&script_path, script.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&script_path, perms)?;
    }

    Ok(ElevatedPaths {
        script_path,
        result_path,
        stdout_path,
        stderr_path,
    })
}

#[cfg(not(target_os = "windows"))]
fn finalize_unix_result(
    paths: ElevatedPaths,
    launch_result: Result<
        Result<std::process::Output, std::io::Error>,
        tokio::time::error::Elapsed,
    >,
    prompt_name: &str,
    timeout_secs: u64,
) -> Result<ElevatedResult> {
    let result = match launch_result {
        Err(_) => {
            cleanup_unix_paths(&paths);
            return Err(anyhow::anyhow!(
                "Elevated command timed out after {}s while waiting for {}",
                timeout_secs,
                prompt_name
            ));
        }
        Ok(Err(e)) => {
            cleanup_unix_paths(&paths);
            return Err(anyhow::anyhow!("Failed to start elevated command: {}", e));
        }
        Ok(Ok(output)) => output,
    };

    if !paths.result_path.exists() {
        let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
        cleanup_unix_paths(&paths);
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(anyhow::anyhow!(
            "Elevated command did not complete. The user may have cancelled authentication or the system could not show the privilege prompt. {}",
            detail
        ));
    }

    let exit_code = std::fs::read_to_string(&paths.result_path)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(-1);
    let stdout = std::fs::read_to_string(&paths.stdout_path).unwrap_or_default();
    let stderr = std::fs::read_to_string(&paths.stderr_path).unwrap_or_default();
    cleanup_unix_paths(&paths);

    Ok(ElevatedResult {
        exit_code,
        stdout: stdout.trim().to_string(),
        stderr: stderr.trim().to_string(),
    })
}

#[cfg(not(target_os = "windows"))]
fn cleanup_unix_paths(paths: &ElevatedPaths) {
    let _ = std::fs::remove_file(&paths.script_path);
    let _ = std::fs::remove_file(&paths.result_path);
    let _ = std::fs::remove_file(&paths.stdout_path);
    let _ = std::fs::remove_file(&paths.stderr_path);
}

#[cfg(not(target_os = "windows"))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn apple_script_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "linux")]
fn command_exists(cmd: &str) -> bool {
    std_command("which")
        .arg(cmd)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn parse_result(json_str: &str) -> Result<ElevatedResult> {
    // Strip UTF-8 BOM (U+FEFF) that Windows WriteAllText with UTF8 encoding emits,
    // then also strip whitespace. serde_json rejects any leading non-JSON bytes.
    let stripped = json_str.trim_start_matches('\u{FEFF}').trim();
    let v: serde_json::Value = serde_json::from_str(stripped).map_err(|e| {
        anyhow::anyhow!("Failed to parse elevated result: {} | raw: {}", e, json_str)
    })?;

    Ok(ElevatedResult {
        exit_code: v["exit_code"].as_i64().unwrap_or(-1) as i32,
        stdout: v["stdout"].as_str().unwrap_or("").to_string(),
        stderr: v["stderr"].as_str().unwrap_or("").to_string(),
    })
}

#[cfg(target_os = "windows")]
fn launch_elevated_windows(exe: &str, args: &str) -> Result<()> {
    let verb = "runas\0".encode_utf16().collect::<Vec<u16>>();
    let file: Vec<u16> = exe.encode_utf16().chain(std::iter::once(0)).collect();
    let params: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(params.as_ptr()),
            PCWSTR::null(),
            SW_HIDE,
        )
    };

    // ShellExecuteW returns > 32 on success
    let code = result.0 as usize;
    if code > 32 {
        Ok(())
    } else if code == 5 {
        // ERROR_ACCESS_DENIED — user clicked "No" in UAC dialog
        Err(anyhow::anyhow!(
            "UAC elevation was denied by the user (error code 5). \
             The operation requires administrator privileges. \
             Please try again and click 'Yes' in the UAC dialog."
        ))
    } else {
        Err(anyhow::anyhow!(
            "ShellExecuteW runas failed with code {}. \
             The system may not support UAC elevation in the current context.",
            code
        ))
    }
}

#[cfg(not(target_os = "windows"))]
pub async fn run_elevated_powershell(
    _command: &str,
    _arch: &str,
    _timeout_secs: u64,
) -> Result<ElevatedResult> {
    Err(anyhow::anyhow!(
        "UAC elevation is only supported on Windows"
    ))
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{build_windows_elevation_payload, write_windows_elevation_context_with};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "piscis_elevation_test_{}_工作区_quote'$`_雪",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir(&path).expect("create elevation test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn windows_elevation_inner_wrapper_applies_cwd_and_environment_exactly() {
        let test_directory = TestDirectory::new();
        let context_path = test_directory.0.join("context.json");
        let inner_script_path = test_directory.0.join("inner.ps1");
        let environment = vec![
            ("ORDINARY".to_string(), "plain".to_string()),
            ("HOSTILE".to_string(), "quote'$`\"\r\n第二行".to_string()),
            ("EMPTY".to_string(), String::new()),
        ];
        let command = r#"$processEnvironment = [Environment]::GetEnvironmentVariables('Process')
[ordered]@{
    cwd = (Get-Location).Path
    ordinary = [Environment]::GetEnvironmentVariable('ORDINARY', 'Process')
    hostile = [Environment]::GetEnvironmentVariable('HOSTILE', 'Process')
    empty_exists = $processEnvironment.Contains('EMPTY')
    empty_value = $processEnvironment['EMPTY']
} | ConvertTo-Json -Compress"#;
        let payload = build_windows_elevation_payload(
            command,
            &test_directory.0,
            &environment,
            &context_path,
        )
        .expect("build wrapper payload");
        std::fs::write(&context_path, payload.context_json).expect("write context JSON");
        std::fs::write(&inner_script_path, payload.inner_script).expect("write inner wrapper");

        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&inner_script_path)
            .output()
            .expect("run generated wrapper with Windows PowerShell");
        let stdout = String::from_utf8(output.stdout).expect("wrapper stdout is UTF-8");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "wrapper failed: status={:?}, stderr={stderr}",
            output.status.code()
        );
        let observed: serde_json::Value =
            serde_json::from_str(stdout.trim()).expect("wrapper emits observation JSON");

        assert_eq!(observed["cwd"], test_directory.0.to_string_lossy().as_ref());
        assert_eq!(observed["ordinary"], "plain");
        assert_eq!(observed["hostile"], "quote'$`\"\r\n第二行");
        assert_eq!(observed["empty_exists"], true);
        assert_eq!(observed["empty_value"], "");
    }

    #[test]
    fn windows_elevation_context_write_failure_removes_partial_file_and_redacts_error() {
        let test_directory = TestDirectory::new();
        let context_path = test_directory.0.join("partial.context.json");
        let context_json = b"partial-sensitive-context-rest";
        let partial_secret = "partial-sensitive-context";
        let writer_error_secret = "writer-error-secret";

        let error =
            write_windows_elevation_context_with(&context_path, context_json, |path, contents| {
                std::fs::write(path, &contents[..partial_secret.len()])?;
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    writer_error_secret,
                ))
            })
            .expect_err("partial context write must fail");

        assert!(
            !context_path.exists(),
            "partial context file was not removed"
        );
        let message = error.to_string();
        assert!(!message.contains(partial_secret));
        assert!(!message.contains(writer_error_secret));
    }

    #[test]
    fn windows_elevation_payload_round_trips_cwd_and_hostile_environment_as_data() {
        let cwd = Path::new("C:\\工作区\\quote'$`\nline");
        let context_path = Path::new("C:\\Temp\\context'$`\n雪.json");
        let environment = vec![
            ("ORDINARY".to_string(), "plain".to_string()),
            ("HOSTILE_雪".to_string(), "quote'$`\"\r\n第二行".to_string()),
            ("EMPTY".to_string(), String::new()),
        ];

        let payload =
            build_windows_elevation_payload("Write-Output ready", cwd, &environment, context_path)
                .expect("hostile values remain data");
        let context: serde_json::Value =
            serde_json::from_slice(&payload.context_json).expect("valid context JSON");

        assert_eq!(context["cwd"], cwd.to_string_lossy().as_ref());
        assert_eq!(context["env"][0]["key"], "ORDINARY");
        assert_eq!(context["env"][0]["value"], "plain");
        assert_eq!(context["env"][1]["key"], "HOSTILE_雪");
        assert_eq!(context["env"][1]["value"], "quote'$`\"\r\n第二行");
        assert_eq!(context["env"][2]["key"], "EMPTY");
        assert_eq!(context["env"][2]["value"], "");

        for secret in [
            cwd.to_string_lossy().as_ref(),
            context_path.to_string_lossy().as_ref(),
            "HOSTILE_雪",
            "quote'$`\"\r\n第二行",
        ] {
            assert!(
                !payload.inner_script.contains(secret),
                "context data leaked into executable PowerShell text"
            );
        }
        assert!(payload.inner_script.contains("Set-Location -LiteralPath"));
        assert!(payload.inner_script.contains("SetEnvironmentVariable"));
        assert!(payload.inner_script.ends_with("Write-Output ready\n"));
    }

    #[test]
    fn windows_elevation_payload_rejects_invalid_environment_keys_without_echoing_them() {
        for key in ["BAD=KEY", "BAD\0KEY"] {
            let error = match build_windows_elevation_payload(
                "Write-Output unreachable",
                Path::new("C:\\workspace"),
                &[(key.to_string(), "secret-value".to_string())],
                Path::new("C:\\Temp\\context.json"),
            ) {
                Ok(_) => panic!("invalid Windows environment key must fail"),
                Err(error) => error,
            };
            let message = error.to_string();
            assert!(!message.contains(key));
            assert!(!message.contains("secret-value"));
        }
    }
}
