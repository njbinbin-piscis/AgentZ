//! Remote workspace targets for the extension host.
//!
//! The same agentz-server bundle (`extension-host/dist/host.js`) runs on the
//! local machine, an SSH host, a Docker container or a WSL distro; only the
//! transport differs. Every transport is a child process whose stdio carries
//! the NDJSON stream, so the broker in `commands::ext_host` is transport-agnostic.
//!
//! SSH uses the system `ssh` client so `~/.ssh/config`, agents, ProxyJump and
//! ControlMaster all behave exactly like in a terminal.

pub mod forward;
pub mod lsp;
pub mod mirror;
#[cfg(test)]
mod live_tests;
pub mod ssh_setup;
pub mod vfs;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub const SERVER_DIR: &str = ".agentz-server";

static MANAGER: std::sync::OnceLock<std::sync::Arc<crate::commands::ext_host::ExtHostManager>> =
    std::sync::OnceLock::new();

/// Make the extension-host broker reachable from code without `AppState`
/// (git helpers, agent tools). The first registered instance wins.
pub fn register_manager(
    m: std::sync::Arc<crate::commands::ext_host::ExtHostManager>,
) -> std::sync::Arc<crate::commands::ext_host::ExtHostManager> {
    let _ = MANAGER.set(m.clone());
    m
}

pub fn manager() -> Option<std::sync::Arc<crate::commands::ext_host::ExtHostManager>> {
    MANAGER.get().cloned()
}

/// If `path` is an `agentz-remote://` URI for the connected remote, return the
/// remote path; error if it names a remote that is not connected.
pub async fn resolve(path: &str) -> Result<Option<String>, String> {
    let Some(parsed) = vfs::parse(path) else { return Ok(None) };
    let mgr = manager().ok_or("remote broker unavailable")?;
    mgr.wait_for_authority(&parsed.authority).await?;
    Ok(Some(parsed.path))
}

pub fn is_remote(path: &str) -> bool {
    vfs::parse(path).is_some()
}

fn git_repo_cache() -> &'static std::sync::Mutex<HashMap<String, Vec<String>>> {
    static C: std::sync::OnceLock<std::sync::Mutex<HashMap<String, Vec<String>>>> = std::sync::OnceLock::new();
    C.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn cache_key(workspace: &str) -> String {
    workspace.replace('\\', "/").trim_end_matches('/').to_string()
}

/// Rediscover git repos under a remote workspace (async; feeds the sync
/// `git_workspace::discover_git_repos`).
pub async fn refresh_git_repos(workspace: &str) -> Result<(), String> {
    let Some(root) = resolve(workspace).await? else { return Ok(()) };
    let v = call("git.discover", serde_json::json!({ "path": root, "maxDepth": 4 })).await?;
    let repos: Vec<String> = serde_json::from_value(v).map_err(|e| e.to_string())?;
    git_repo_cache().lock().unwrap().insert(cache_key(workspace), repos);
    Ok(())
}

/// Cached workspace-relative repo roots; `None` until first refresh.
pub fn cached_git_repos(workspace: &str) -> Option<Vec<String>> {
    git_repo_cache().lock().unwrap().get(&cache_key(workspace)).cloned()
}

/// Call an agentz-server method on the connected remote.
pub async fn call(method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    manager()
        .ok_or("remote broker unavailable")?
        .request(method, params)
        .await
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RemoteTarget {
    Local,
    Ssh {
        host: String,
    },
    Docker {
        container: String,
        #[serde(default)]
        user: Option<String>,
    },
    Wsl {
        distro: String,
    },
}

impl RemoteTarget {
    pub fn is_local(&self) -> bool {
        matches!(self, RemoteTarget::Local)
    }

    /// Authority component of `agentz-remote://<authority>/path` URIs.
    pub fn authority(&self) -> String {
        match self {
            RemoteTarget::Local => "local".into(),
            RemoteTarget::Ssh { host } => format!("ssh-remote+{host}"),
            RemoteTarget::Docker { container, .. } => format!("docker+{container}"),
            RemoteTarget::Wsl { distro } => format!("wsl+{distro}"),
        }
    }

    /// Build a command that runs `script` under `sh -c` on the target.
    pub fn sh(&self, script: &str) -> Result<Command, String> {
        let mut cmd = match self {
            RemoteTarget::Local => return Err("sh transport is not used for local targets".into()),
            RemoteTarget::Ssh { host } => {
                let mut c = piscis_kernel::proc::tokio_command("ssh");
                // accept-new: trust-on-first-use like VS Code, but still refuse
                // a *changed* host key.
                c.args([
                    "-T",
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "StrictHostKeyChecking=accept-new",
                    "-o",
                    "ServerAliveInterval=15",
                ])
                    .args(ssh_mux_args())
                    .arg(host)
                    .arg(format!("sh -c {}", shell_quote(script)));
                c
            }
            RemoteTarget::Docker { container, user } => {
                let mut c = piscis_kernel::proc::tokio_command("docker");
                c.args(["exec", "-i"]);
                if let Some(u) = user.as_deref().filter(|u| !u.is_empty()) {
                    c.args(["-u", u]);
                }
                c.arg(container).args(["sh", "-c", script]);
                c
            }
            RemoteTarget::Wsl { distro } => {
                let mut c = piscis_kernel::proc::tokio_command("wsl.exe");
                c.args(["-d", distro, "--", "sh", "-c", script]);
                c
            }
        };
        cmd.kill_on_drop(true);
        Ok(cmd)
    }

    /// argv for an interactive login shell in `cwd`, meant to run inside a
    /// local PTY: the client allocates the remote TTY and forwards resizes.
    pub fn interactive_shell_argv(&self, cwd: &str) -> Vec<String> {
        let login = "command -v bash >/dev/null 2>&1 && exec bash -l || exec \"${SHELL:-sh}\" -l";
        match self {
            RemoteTarget::Local => vec![],
            RemoteTarget::Ssh { host } => {
                let mut v: Vec<String> = vec!["ssh".into(), "-tt".into(), "-o".into(), "ServerAliveInterval=15".into()];
                v.extend(ssh_mux_args());
                v.push(host.clone());
                v.push(format!("cd {} 2>/dev/null; {login}", shell_quote(cwd)));
                v
            }
            RemoteTarget::Docker { container, user } => {
                let mut v: Vec<String> = vec!["docker".into(), "exec".into(), "-it".into()];
                if let Some(u) = user.as_deref().filter(|u| !u.is_empty()) {
                    v.extend(["-u".into(), u.to_string()]);
                }
                v.extend(["-w".into(), cwd.to_string(), container.clone(), "sh".into(), "-c".into(), login.into()]);
                v
            }
            RemoteTarget::Wsl { distro } => vec![
                "wsl.exe".into(),
                "-d".into(),
                distro.clone(),
                "--cd".into(),
                cwd.to_string(),
            ],
        }
    }

    pub async fn run(&self, script: &str) -> Result<String, String> {
        let out = self
            .sh(script)?
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| format!("{}: transport failed: {e}", self.authority()))?;
        if !out.status.success() {
            return Err(format!(
                "{}: `{}` failed: {}",
                self.authority(),
                script,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Run `script` with `data` streamed to its stdin (used for uploads).
    pub async fn run_with_stdin(&self, script: &str, data: &[u8]) -> Result<(), String> {
        let mut child = self
            .sh(script)?
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{}: transport failed: {e}", self.authority()))?;
        let mut stdin = child.stdin.take().ok_or("no stdin")?;
        stdin.write_all(data).await.map_err(|e| e.to_string())?;
        stdin.shutdown().await.map_err(|e| e.to_string())?;
        drop(stdin);
        let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "{}: upload failed: {}",
                self.authority(),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(())
    }
}

/// Connection sharing so terminals / probes reuse one authenticated session.
/// Windows OpenSSH has no ControlMaster support.
fn ssh_mux_args() -> Vec<String> {
    if cfg!(windows) {
        return vec![];
    }
    let Some(home) = dirs_home() else { return vec![] };
    let dir = home.join(".ssh").join("agentz-mux");
    let _ = std::fs::create_dir_all(&dir);
    vec![
        "-o".into(),
        "ControlMaster=auto".into(),
        "-o".into(),
        format!("ControlPath={}/%C", dir.display()),
        "-o".into(),
        "ControlPersist=10m".into(),
    ]
}

/// Shell prelude for spawning dev tools (language servers, debug adapters):
/// per-user install dirs that non-interactive shells miss, plus the Node we
/// deployed (appended, so a system node wins) for `#!/usr/bin/env node` tools.
pub const TOOL_PATH_SETUP: &str = "PATH=\"$PWD/node_modules/.bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/go/bin:$HOME/.npm-global/bin:/usr/local/bin:$PATH\"; \
     for d in \"$HOME\"/.agentz-server/node-*/bin; do [ -d \"$d\" ] && PATH=\"$PATH:$d\"; done; export PATH";

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn fnv1a(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

#[derive(Debug, Clone, Serialize)]
pub struct DeployedServer {
    pub home: String,
    pub node: String,
    pub server_js: String,
    pub arch: String,
}

/// Upload host.js (content-addressed) and, if the target has no usable Node,
/// a pinned Linux Node tarball from the app resources.
pub async fn deploy(
    target: &RemoteTarget,
    host_js: &Path,
    node_resources: Option<&Path>,
) -> Result<DeployedServer, String> {
    let bytes = std::fs::read(host_js).map_err(|e| format!("read {}: {e}", host_js.display()))?;
    let version = fnv1a(&bytes);

    // One round trip: home, arch, whether this build is already uploaded, and
    // the first Node >= 18 found (bundled runtime preferred over system node).
    let probe = target
        .run(&format!(
            "d=\"$HOME/{SERVER_DIR}\"; mkdir -p \"$d/{version}\" && echo \"$HOME\" && uname -m && \
             (test -f \"$d/{version}/server.js\" && echo present || echo missing) && \
             for n in $(ls \"$d\"/node-*/bin/node 2>/dev/null) $(command -v node 2>/dev/null); do \
               \"$n\" -e 'process.exit(+process.versions.node.split(\".\")[0] >= 18 ? 0 : 1)' 2>/dev/null && echo \"$n\" && break; \
             done; true"
        ))
        .await?;
    let mut lines = probe.lines().map(str::trim).filter(|l| !l.is_empty());
    let home = lines.next().ok_or("remote probe returned no $HOME")?.to_string();
    let arch = lines.next().unwrap_or("x86_64").to_string();
    let present = lines.next() == Some("present");
    let node = lines.next().map(str::to_string);

    let server_js = format!("{home}/{SERVER_DIR}/{version}/server.js");
    if !present {
        target
            .run_with_stdin(
                &format!("cat > {p}.tmp && mv {p}.tmp {p}", p = shell_quote(&server_js)),
                &bytes,
            )
            .await?;
    }

    let node = match node {
        Some(n) => n,
        None => upload_node(target, &home, &arch, node_resources).await?,
    };

    Ok(DeployedServer { home, node, server_js, arch })
}

async fn upload_node(
    target: &RemoteTarget,
    home: &str,
    arch: &str,
    node_resources: Option<&Path>,
) -> Result<String, String> {
    let node_arch = match arch {
        "x86_64" | "amd64" => "x64",
        "aarch64" | "arm64" => "arm64",
        other => return Err(format!("no bundled Node for remote arch '{other}'; install node >= 18 on the target")),
    };
    let dir = node_resources.ok_or("remote has no node >= 18 and no bundled Node runtime is available (run `npm run fetch-node` in extension-host)")?;
    let tarball = std::fs::read_dir(dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(&format!("-linux-{node_arch}.tar.xz")))
        })
        .ok_or_else(|| format!("bundled node tarball for linux-{node_arch} not found in {}", dir.display()))?;
    let stem = tarball
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .trim_end_matches(".tar.xz")
        .to_string();
    let data = std::fs::read(&tarball).map_err(|e| e.to_string())?;
    let base = format!("{home}/{SERVER_DIR}");
    target
        .run_with_stdin(
            &format!("cd {} && tar -xJf -", shell_quote(&base)),
            &data,
        )
        .await?;
    Ok(format!("{base}/{stem}/bin/node"))
}

/// Script that starts the server inside `cwd` on the target.
pub fn launch_script(server: &DeployedServer, cwd: &str) -> String {
    format!(
        "cd {} 2>/dev/null || cd \"$HOME\"; exec {} {}",
        shell_quote(cwd),
        shell_quote(&server.node),
        shell_quote(&server.server_js)
    )
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocalExtension {
    pub id: String,
    pub version: String,
    pub extension_path: String,
}

/// Copy workspace-side extensions to the target (tar over the transport),
/// skipping ones already present. Returns id -> remote extension path.
pub async fn sync_extensions(
    target: &RemoteTarget,
    home: &str,
    extensions: &[LocalExtension],
) -> Result<HashMap<String, String>, String> {
    let mut mapped = HashMap::new();
    for ext in extensions {
        let remote_dir = format!("{home}/{SERVER_DIR}/extensions/{}-{}", ext.id, ext.version);
        let present = target
            .run(&format!("test -f {}/package.json && echo yes || true", shell_quote(&remote_dir)))
            .await?;
        if present.trim() != "yes" {
            let archive = tar_dir(Path::new(&ext.extension_path)).await?;
            target
                .run_with_stdin(
                    &format!(
                        "mkdir -p {d} && tar -xf - -C {d}",
                        d = shell_quote(&remote_dir)
                    ),
                    &archive,
                )
                .await?;
        }
        mapped.insert(ext.id.clone(), remote_dir);
    }
    Ok(mapped)
}

async fn tar_dir(dir: &Path) -> Result<Vec<u8>, String> {
    let out = piscis_kernel::proc::tokio_command("tar")
        .arg("-cf")
        .arg("-")
        .arg("-C")
        .arg(dir)
        .arg(".")
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("tar not available: {e}"))?;
    if !out.status.success() {
        return Err(format!("tar {} failed: {}", dir.display(), String::from_utf8_lossy(&out.stderr)));
    }
    Ok(out.stdout)
}

/// Whether an extension should run next to the workspace (VS Code
/// `extensionKind` rules: anything that is not UI-only).
pub fn runs_on_workspace(manifest: &serde_json::Value) -> bool {
    match manifest.get("extensionKind") {
        Some(serde_json::Value::Array(kinds)) => kinds.iter().any(|k| k.as_str() == Some("workspace")),
        Some(serde_json::Value::String(k)) => k == "workspace",
        _ => manifest.get("main").is_some(),
    }
}

pub fn read_manifest(ext_dir: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(ext_dir.join("package.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// `Host` aliases from `~/.ssh/config` (wildcards excluded).
pub fn ssh_config_hosts() -> Vec<String> {
    let Some(home) = dirs_home() else { return vec![] };
    let Ok(text) = std::fs::read_to_string(home.join(".ssh").join("config")) else {
        return vec![];
    };
    let mut hosts = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let mut parts = line.split_whitespace();
        if parts.next().is_some_and(|k| k.eq_ignore_ascii_case("host")) {
            for h in parts {
                if !h.contains(['*', '?', '!']) && !hosts.iter().any(|x| x == h) {
                    hosts.push(h.to_string());
                }
            }
        }
    }
    hosts
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

pub async fn wsl_distros() -> Vec<String> {
    if !cfg!(windows) {
        return vec![];
    }
    let Ok(out) = piscis_kernel::proc::tokio_command("wsl.exe")
        .args(["-l", "-q"])
        .output()
        .await
    else {
        return vec![];
    };
    // With no distro installed wsl.exe prints its help text and exits non-zero.
    if !out.status.success() {
        return vec![];
    }
    // wsl.exe writes UTF-16LE.
    let units: Vec<u16> = out
        .stdout
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
        .lines()
        .map(|l| l.trim().trim_matches('\0').to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub async fn docker_containers() -> Vec<serde_json::Value> {
    let Ok(out) = piscis_kernel::proc::tokio_command("docker")
        .args(["ps", "--format", "{{json .}}"])
        .output()
        .await
    else {
        return vec![];
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| {
            serde_json::json!({
                "id": v.get("ID"),
                "name": v.get("Names"),
                "image": v.get("Image"),
                "status": v.get("Status"),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct DevcontainerUp {
    pub container_id: String,
    pub remote_user: Option<String>,
    pub remote_workspace_folder: String,
    pub extensions: Vec<String>,
}

/// `devcontainer up` via the reference CLI (global install, else `npx`).
pub async fn devcontainer_up(workspace: &str) -> Result<DevcontainerUp, String> {
    let args = ["up", "--workspace-folder", workspace];
    let out = match piscis_kernel::proc::tokio_command("devcontainer").args(args).output().await {
        Ok(o) => o,
        Err(_) => piscis_kernel::proc::tokio_command(if cfg!(windows) { "npx.cmd" } else { "npx" })
            .args(["-y", "@devcontainers/cli"])
            .args(args)
            .output()
            .await
            .map_err(|e| format!("devcontainer CLI unavailable (install Node/npm or `npm i -g @devcontainers/cli`): {e}"))?,
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let result: serde_json::Value = stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str(l.trim()).ok())
        .ok_or_else(|| format!("devcontainer up failed: {}", String::from_utf8_lossy(&out.stderr).trim()))?;
    if result.get("outcome").and_then(|v| v.as_str()) != Some("success") {
        return Err(format!("devcontainer up failed: {result}"));
    }
    let s = |k: &str| result.get(k).and_then(|v| v.as_str()).map(str::to_string);
    Ok(DevcontainerUp {
        container_id: s("containerId").ok_or("devcontainer up: missing containerId")?,
        remote_user: s("remoteUser"),
        remote_workspace_folder: s("remoteWorkspaceFolder").unwrap_or_else(|| "/workspaces".into()),
        extensions: devcontainer_extensions(Path::new(workspace)),
    })
}

/// `customizations.vscode.extensions` from `.devcontainer/devcontainer.json`
/// (or `.devcontainer.json`). JSONC comments are stripped line-wise.
pub fn devcontainer_extensions(workspace: &Path) -> Vec<String> {
    let candidates = [
        workspace.join(".devcontainer").join("devcontainer.json"),
        workspace.join(".devcontainer.json"),
    ];
    let Some(text) = candidates.iter().find_map(|p| std::fs::read_to_string(p).ok()) else {
        return vec![];
    };
    let cleaned: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&cleaned) else {
        return vec![];
    };
    v.pointer("/customizations/vscode/extensions")
        .and_then(|e| e.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_escapes_single_quotes() {
        assert_eq!(shell_quote("a'b"), r"'a'\''b'");
    }

    #[test]
    fn target_deserializes_from_tagged_json() {
        let t: RemoteTarget = serde_json::from_str(r#"{"kind":"ssh","host":"box"}"#).unwrap();
        assert_eq!(t.authority(), "ssh-remote+box");
    }

    #[test]
    fn extension_kind_rules() {
        assert!(!runs_on_workspace(&serde_json::json!({"extensionKind": ["ui"]})));
        assert!(runs_on_workspace(&serde_json::json!({"extensionKind": ["ui", "workspace"]})));
        assert!(runs_on_workspace(&serde_json::json!({"main": "./out/ext.js"})));
    }
}
