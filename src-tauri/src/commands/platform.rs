//! Small platform-integration commands shared across workspaces.

use std::io::Write;
use std::path::Path;

use tauri::AppHandle;

use super::data_scope::resolve_global_config_dir;

/// Open a path with the OS default handler (file → default app, dir → file manager).
#[tauri::command]
pub async fn open_path(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Path not found: {path}"));
    }

    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";

    piscis_kernel::proc::tokio_command(program)
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Failed to open '{path}': {e}"))?;
    Ok(())
}

/// Reveal a file or folder in the system file manager (select/highlight when supported).
#[tauri::command]
pub async fn reveal_in_folder(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Path not found: {path}"));
    }

    #[cfg(target_os = "windows")]
    {
        let arg = format!("/select,\"{}\"", p.to_string_lossy().replace('/', "\\"));
        piscis_kernel::proc::tokio_command("explorer")
            .arg(arg)
            .spawn()
            .map_err(|e| format!("Failed to reveal '{path}': {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        piscis_kernel::proc::tokio_command("open")
            .arg("-R")
            .arg(&path)
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
