//! Remote development commands: target discovery, devcontainers, extension
//! sync and `agentz-remote://` file operations served by the attached
//! agentz-server.

use std::collections::HashMap;
use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::State;

use crate::remote::{self, vfs, DevcontainerUp, LocalExtension, RemoteTarget};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct RemoteTargets {
    pub ssh_hosts: Vec<String>,
    pub wsl_distros: Vec<String>,
    pub containers: Vec<Value>,
}

#[tauri::command]
pub async fn remote_list_targets() -> Result<RemoteTargets, String> {
    let (wsl_distros, containers) = tokio::join!(remote::wsl_distros(), remote::docker_containers());
    Ok(RemoteTargets {
        ssh_hosts: remote::ssh_config_hosts(),
        wsl_distros,
        containers,
    })
}

/// Probe a target without starting the extension host (connectivity check).
#[tauri::command]
pub async fn remote_probe(target: RemoteTarget) -> Result<String, String> {
    if target.is_local() {
        return Ok(std::env::consts::OS.to_string());
    }
    target.run("uname -sm && echo \"$HOME\"").await
}

#[derive(Debug, Serialize)]
pub struct RemoteDirListing {
    /// Absolute path that was listed (`~` / empty resolve to `$HOME`).
    pub path: String,
    pub dirs: Vec<String>,
}

/// Sub-directories of `path` on a target, over the plain transport — usable
/// before agentz-server is deployed (folder picker in the connect dialog).
#[tauri::command]
pub async fn remote_list_dirs(
    target: RemoteTarget,
    path: Option<String>,
) -> Result<RemoteDirListing, String> {
    if target.is_local() {
        return Err("local targets use the native folder dialog".into());
    }
    let path = path.unwrap_or_default();
    let cd = match path.trim() {
        "" | "~" => "cd".to_string(),
        p => format!("cd -- {}", remote::shell_quote(p)),
    };
    let out = target
        .run(&format!("{cd} && pwd && ls -1Ap 2>/dev/null"))
        .await?;
    let mut lines = out.lines();
    let path = lines.next().unwrap_or("/").trim().to_string();
    let mut dirs: Vec<String> = lines
        .filter_map(|l| l.strip_suffix('/'))
        .filter(|d| !d.is_empty())
        .map(str::to_string)
        .collect();
    dirs.sort_by_key(|d| (d.starts_with('.'), d.to_lowercase()));
    Ok(RemoteDirListing { path, dirs })
}

/// Append a `Host` alias to `~/.ssh/config` (for non-default ports, which the
/// `ssh-remote+<host>` authority cannot carry). Returns the alias; an existing
/// alias is reused untouched.
#[tauri::command]
pub async fn remote_ssh_add_host(
    hostname: String,
    user: Option<String>,
    port: u16,
) -> Result<String, String> {
    remote::ssh_add_host(&hostname, user.as_deref(), port)
}

/// Password-once setup of key authentication for an SSH host.
#[tauri::command]
pub async fn remote_ssh_setup_key(host: String, password: String) -> Result<String, String> {
    remote::ssh_setup::setup_key_auth(&host, &password).await
}

/// Listening TCP ports on the connected remote.
#[tauri::command]
pub async fn remote_ports_detect(state: State<'_, AppState>) -> Result<Value, String> {
    state.ext_host.request("ports.list", json!({})).await
}

#[tauri::command]
pub async fn remote_forward_start(
    state: State<'_, AppState>,
    remote_port: u16,
    local_port: Option<u16>,
) -> Result<remote::forward::Forward, String> {
    let target = state.ext_host.target().await.ok_or("no remote connection")?;
    remote::forward::start(&target, remote_port, local_port).await
}

#[tauri::command]
pub async fn remote_forward_stop(remote_port: u16) -> Result<(), String> {
    remote::forward::stop(remote_port);
    Ok(())
}

#[tauri::command]
pub async fn remote_forward_list() -> Result<Vec<remote::forward::Forward>, String> {
    Ok(remote::forward::list())
}

#[tauri::command]
pub async fn remote_devcontainer_up(workspace: String) -> Result<DevcontainerUp, String> {
    remote::devcontainer_up(&workspace).await
}

/// Copy the given local extensions to the active remote, keeping only those
/// that run on the workspace side. Returns id -> remote extension path.
#[tauri::command]
pub async fn remote_sync_extensions(
    state: State<'_, AppState>,
    extensions: Vec<LocalExtension>,
) -> Result<HashMap<String, String>, String> {
    let mgr = state.ext_host.clone();
    let target = mgr.target().await.ok_or("extension host is not running")?;
    if target.is_local() {
        return Ok(extensions.into_iter().map(|e| (e.id, e.extension_path)).collect());
    }
    let server = mgr.server().await.ok_or("remote server not deployed")?;
    let workspace_side: Vec<LocalExtension> = extensions
        .into_iter()
        .filter(|e| {
            remote::read_manifest(Path::new(&e.extension_path))
                .is_some_and(|m| remote::runs_on_workspace(&m))
        })
        .collect();
    remote::sync_extensions(&target, &server.home, &workspace_side).await
}

/// Generic passthrough to the agentz-server control plane.
#[tauri::command]
pub async fn remote_request(
    state: State<'_, AppState>,
    method: String,
    params: Option<Value>,
) -> Result<Value, String> {
    state
        .ext_host
        .request(&method, params.unwrap_or_else(|| json!({})))
        .await
}

/// Resolve a URI to a path on the attached server, verifying the authority.
pub async fn resolve_remote(_state: &AppState, uri: &str) -> Result<Option<String>, String> {
    remote::resolve(uri).await
}

pub async fn read_remote_text(state: &AppState, path: &str) -> Result<(Vec<u8>, u64), String> {
    let v = state.ext_host.request("fs.readFile", json!({ "path": path })).await?;
    let b64 = v.get("base64").and_then(|b| b.as_str()).ok_or("bad fs.readFile reply")?;
    let bytes = STANDARD.decode(b64).map_err(|e| e.to_string())?;
    let len = bytes.len() as u64;
    Ok((bytes, len))
}

pub async fn write_remote(state: &AppState, path: &str, content: &[u8]) -> Result<(), String> {
    state
        .ext_host
        .request("fs.writeFile", json!({ "path": path, "base64": STANDARD.encode(content) }))
        .await
        .map(|_| ())
}

#[derive(Debug, Serialize)]
pub struct RemoteDirEntry {
    pub name: String,
    pub uri: String,
    pub is_dir: bool,
}

#[tauri::command]
pub async fn remote_fs_list(state: State<'_, AppState>, uri: String) -> Result<Vec<RemoteDirEntry>, String> {
    let parsed = vfs::parse(&uri).ok_or("not an agentz-remote URI")?;
    let path = resolve_remote(&state, &uri).await?.unwrap_or_default();
    let v = state.ext_host.request("fs.readDir", json!({ "path": path })).await?;
    let base = path.trim_end_matches('/');
    let mut out: Vec<RemoteDirEntry> = v
        .as_array()
        .ok_or("bad fs.readDir reply")?
        .iter()
        .filter_map(|e| {
            let name = e.get("name")?.as_str()?.to_string();
            Some(RemoteDirEntry {
                uri: vfs::format(&parsed.authority, &format!("{base}/{name}")),
                is_dir: e.get("isDirectory").and_then(|d| d.as_bool()).unwrap_or(false),
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

#[tauri::command]
pub async fn remote_fs_action(
    state: State<'_, AppState>,
    action: String,
    uri: String,
    to: Option<String>,
) -> Result<(), String> {
    let path = resolve_remote(&state, &uri).await?.ok_or("not an agentz-remote URI")?;
    let (method, params) = match action.as_str() {
        "create_file" => ("fs.writeFile", json!({ "path": path, "text": "" })),
        "create_dir" => ("fs.mkdir", json!({ "path": path })),
        "delete" => ("fs.delete", json!({ "path": path, "recursive": true })),
        "rename" => {
            let to = to.ok_or("rename requires `to`")?;
            let to_path = resolve_remote(&state, &to).await?.unwrap_or(to);
            ("fs.rename", json!({ "from": path, "to": to_path }))
        }
        other => return Err(format!("unknown action: {other}")),
    };
    state.ext_host.request(method, params).await.map(|_| ())
}
