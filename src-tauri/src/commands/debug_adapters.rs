//! On-demand installation of debug adapters that users do not already have.
use std::time::Duration;
use tauri::AppHandle;

const JS_DEBUG_VERSION: &str = "v1.140.0";

/// Returns the entry point of the requested adapter, downloading it on first use.
/// Only `js-debug` (Node.js debugging) is installable.
#[tauri::command]
pub async fn debug_adapter_ensure(app: AppHandle, name: String) -> Result<String, String> {
    if name != "js-debug" {
        return Err(format!("unknown debug adapter: {name}"));
    }
    let base = crate::commands::data_scope::resolve_global_config_dir(&app)
        .map_err(|e| e.to_string())?
        .join("debug-adapters");
    let root = base.join(format!("js-debug-{JS_DEBUG_VERSION}"));
    let entry = root.join("js-debug").join("src").join("dapDebugServer.js");
    if entry.is_file() {
        return Ok(entry.to_string_lossy().into_owned());
    }

    let url = format!(
        "https://github.com/microsoft/vscode-js-debug/releases/download/{v}/js-debug-dap-{v}.tar.gz",
        v = JS_DEBUG_VERSION
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .user_agent("AgentZ")
        .build()
        .map_err(|e| e.to_string())?;
    let bytes = client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("downloading js-debug failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("downloading js-debug failed: {e}"))?;

    // Extract next to the target and rename, so a failed extraction is never mistaken for an install.
    let partial = base.join(format!("js-debug-{JS_DEBUG_VERSION}.partial"));
    let _ = tokio::fs::remove_dir_all(&partial).await;
    tokio::fs::create_dir_all(&partial)
        .await
        .map_err(|e| e.to_string())?;
    let archive = partial.join("download.tar.gz");
    tokio::fs::write(&archive, &bytes)
        .await
        .map_err(|e| e.to_string())?;
    let out = piscis_kernel::proc::tokio_command("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&partial)
        .output()
        .await
        .map_err(|e| format!("could not run tar to unpack js-debug: {e}"))?;
    if !out.status.success() {
        let _ = tokio::fs::remove_dir_all(&partial).await;
        return Err(format!(
            "unpacking js-debug failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let _ = tokio::fs::remove_file(&archive).await;
    let _ = tokio::fs::remove_dir_all(&root).await;
    tokio::fs::rename(&partial, &root)
        .await
        .map_err(|e| e.to_string())?;
    if !entry.is_file() {
        return Err("js-debug archive did not contain dapDebugServer.js".into());
    }
    Ok(entry.to_string_lossy().into_owned())
}
