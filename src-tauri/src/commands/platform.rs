//! Small platform-integration commands shared across workspaces.

use std::io::Write;
use std::path::Path;

use tauri::AppHandle;

use super::data_scope::resolve_global_config_dir;

/// Open a path with the OS default handler (file → its registered program, dir →
/// file manager). A file with no registered program is revealed in the file
/// manager instead (its folder, file selected) so the click still goes somewhere
/// useful.
#[tauri::command]
pub async fn open_path(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Path not found: {path}"));
    }

    #[cfg(target_os = "windows")]
    {
        // Mixed separators ("C:\proj/src/a.docx") make the shell fall back to the
        // default Documents folder, so hand it a clean native path.
        let native = path.replace('/', "\\");
        let target = native.clone();
        let outcome = tokio::task::spawn_blocking(move || shell_execute_open(&target))
            .await
            .map_err(|e| format!("Failed to open '{path}': {e}"))?;
        return match outcome {
            Ok(()) => Ok(()),
            // No program is registered for this file type: show it in its folder.
            Err(_) if p.is_file() => reveal_in_folder(native).await,
            Err(code) => Err(format!("Failed to open '{path}' (shell code {code})")),
        };
    }

    #[cfg(target_os = "macos")]
    {
        let status = piscis_kernel::proc::tokio_command("open")
            .arg(&path)
            .status()
            .await
            .map_err(|e| format!("Failed to open '{path}': {e}"))?;
        if status.success() {
            return Ok(());
        }
        if p.is_file() {
            return reveal_in_folder(path).await;
        }
        return Err(format!("Failed to open '{path}'"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        piscis_kernel::proc::tokio_command("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open '{path}': {e}"))?;
        Ok(())
    }
}

/// `ShellExecuteW(open)`: success is any value above 32; otherwise the value is
/// the error code (31 = no association).
#[cfg(target_os = "windows")]
fn shell_execute_open(path: &str) -> Result<(), i32> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;

    const SW_SHOWNORMAL: i32 = 1;
    let wide: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    // SAFETY: both buffers are NUL-terminated and outlive the call; the other
    // pointers are documented as optional.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if result > 32 {
        Ok(())
    } else {
        Err(result as i32)
    }
}
/// Reveal a file or folder in the system file manager (select/highlight when supported).
#[tauri::command]
pub async fn reveal_in_folder(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Path not found: {path}"));
    }

    // A folder is opened (its contents shown); a file is highlighted in its parent.
    #[cfg(target_os = "windows")]
    {
        let native = p.to_string_lossy().replace('/', "\\");
        let mut cmd = piscis_kernel::proc::tokio_command("explorer");
        if p.is_dir() {
            cmd.arg(native);
        } else {
            // No manual quotes: Rust quotes an argument containing spaces itself, and
            // embedded quotes get backslash-escaped, which makes explorer ignore the
            // selection and fall back to its default folder.
            cmd.arg(format!("/select,{native}"));
        }
        cmd.spawn()
            .map_err(|e| format!("Failed to reveal '{path}': {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = piscis_kernel::proc::tokio_command("open");
        if !p.is_dir() {
            cmd.arg("-R");
        }
        cmd.arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to reveal '{path}': {e}"))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let open_target = if p.is_file() {
            p.parent()
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or(path.clone())
        } else {
            path.clone()
        };
        piscis_kernel::proc::tokio_command("xdg-open")
            .arg(&open_target)
            .spawn()
            .map_err(|e| format!("Failed to reveal '{path}': {e}"))?;
    }

    Ok(())
}

/// Whether the webview is likely software-rendered (no usable GPU). WebKit masks
/// the WebGL renderer string, so this is decided from the host side instead.
#[tauri::command]
pub fn platform_software_rendering() -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::env::var("LIBGL_ALWAYS_SOFTWARE").is_ok_and(|v| v != "0") {
            return true;
        }
        // Virtual / framebuffer-only DRM drivers: 3D is absent or emulated.
        const SOFT_DRIVERS: &[&str] = &[
            "vmwgfx",
            "qxl",
            "bochs-drm",
            "bochs",
            "cirrus",
            "simpledrm",
            "vboxvideo",
            "hyperv_drm",
        ];
        let drivers: Vec<String> = std::fs::read_dir("/sys/class/drm")
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.starts_with("card") && !name.contains('-')
            })
            .filter_map(|e| std::fs::read_link(e.path().join("device/driver")).ok())
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        drivers.is_empty() || drivers.iter().all(|d| SOFT_DRIVERS.contains(&d.as_str()))
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        false
    }
}

/// Append a frontend composer-debug line to `{config_dir}/logs/composer.log`.
/// Survives UI freezes — read the file after force-quitting the app.
#[tauri::command]
pub async fn composer_debug_log(app: AppHandle, line: String) -> Result<(), String> {
    let log_dir = resolve_global_config_dir(&app)?.join("logs");
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&log_dir).map_err(|e| format!("create log dir: {e}"))?;
        let path = log_dir.join("composer.log");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("open composer.log: {e}"))?;
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
        writeln!(f, "{now} {line}").map_err(|e| format!("write composer.log: {e}"))
    })
    .await
    .map_err(|e| format!("composer log task: {e}"))?
}

const FRESH_ENV: &str = "AGENTZ_FRESH";

/// True when this process was started as an extra window and should not
/// reopen the project the other window already has.
#[tauri::command]
pub fn app_launch_fresh() -> bool {
    std::env::var(FRESH_ENV).ok().is_some_and(|v| v == "1")
}

/// Start another AgentZ process. It does not reopen the current project.
#[tauri::command]
pub fn app_open_instance() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    cmd.env(FRESH_ENV, "1");
    if std::env::args().any(|arg| arg == "--software-rendering") {
        cmd.arg("--software-rendering");
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|e| format!("could not start AgentZ: {e}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
