//! LSP process lifecycle manager.
//!
//! Manages language server processes per project+language combination:
//! - Auto-detects installed language servers
//! - Spawns processes with proper args and stdio piped
//! - Tracks session state (process handle, port)
//! - Provides cleanup on drop

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::lsp::client::LspClient;

/// Unique key for an LSP session: project_dir + language.
type SessionKey = String;

/// Represents one language for which LSP is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageSupport {
    /// Monaco language ID (e.g. "rust", "typescript", "python")
    pub language_id: String,
    /// Human-readable name
    pub name: String,
    /// File extensions (e.g. [".rs"], [".ts", ".tsx"])
    pub extensions: Vec<String>,
    /// LSP server command (e.g. "rust-analyzer")
    pub server_command: String,
    /// Extra args for the server
    pub server_args: Vec<String>,
    /// Whether the server binary was detected on this machine
    pub available: bool,
}

/// Running LSP session.
struct LspSession {
    #[allow(dead_code)]
    project_dir: String,
    #[allow(dead_code)]
    language: String,
    /// WebSocket port the bridge is listening on
    port: u16,
    /// Set once the bridge task ended (it serves exactly one WS client).
    done: Arc<AtomicBool>,
    /// The child process handle — keeps the process alive.
    /// Dropping this kills the process.
    #[allow(dead_code)]
    child: Child,
}

/// Global LSP process manager.
pub struct LspManager {
    sessions: Mutex<HashMap<SessionKey, Arc<LspSession>>>,
    clients: Mutex<HashMap<SessionKey, Arc<LspClient>>>,
    bridge_app_name: String,
}

impl LspManager {
    pub fn new() -> Self {
        Self::with_bridge_name("Piscis")
    }

    pub fn with_bridge_name(app_name: impl Into<String>) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            clients: Mutex::new(HashMap::new()),
            bridge_app_name: app_name.into(),
        }
    }

    /// Build a session key from project dir + language.
    fn session_key(project_dir: &str, language: &str) -> SessionKey {
        format!("{}|{}", project_dir, language)
    }

    /// List all supported languages with auto-detection of installed servers.
    pub fn supported_languages() -> Vec<LanguageSupport> {
        fn is_on_path(cmd: &str) -> bool {
            locate_executable(cmd).is_some()
        }
        vec![
            LanguageSupport {
                language_id: "rust".into(),
                name: "Rust".into(),
                extensions: vec![".rs".into()],
                server_command: "rust-analyzer".into(),
                server_args: vec![],
                // The rustup proxy exists on PATH even when the component is not
                // installed, so a PATH hit alone is not enough.
                available: locate_executable("rust-analyzer")
                    .map(|p| runs_ok(&p, "--version"))
                    .unwrap_or(false),
            },
            LanguageSupport {
                language_id: "typescript".into(),
                name: "TypeScript / JavaScript".into(),
                extensions: vec![
                    ".ts".into(),
                    ".tsx".into(),
                    ".js".into(),
                    ".jsx".into(),
                    ".mjs".into(),
                    ".cjs".into(),
                ],
                server_command: "typescript-language-server".into(),
                server_args: vec!["--stdio".into()],
                available: is_on_path("typescript-language-server"),
            },
            LanguageSupport {
                language_id: "python".into(),
                name: "Python".into(),
                extensions: vec![".py".into(), ".pyi".into()],
                server_command: python_lsp_server().map(|(c, _)| c).unwrap_or_else(|| "pyright-langserver".into()),
                server_args: python_lsp_server().map(|(_, a)| a).unwrap_or_else(|| vec!["--stdio".into()]),
                available: python_lsp_server().is_some(),
            },
            LanguageSupport {
                language_id: "cpp".into(),
                name: "C / C++".into(),
                extensions: vec![
                    ".c".into(),
                    ".h".into(),
                    ".cpp".into(),
                    ".cc".into(),
                    ".cxx".into(),
                    ".hpp".into(),
                    ".hxx".into(),
                ],
                server_command: "clangd".into(),
                server_args: vec![],
                available: is_on_path("clangd"),
            },
        ]
    }

    /// Detect which language to use for a file path.
    pub fn language_for_file(path: &str) -> Option<String> {
        let lower = path.to_lowercase();
        for lang in Self::supported_languages() {
            if lang.available
                && lang
                    .extensions
                    .iter()
                    .any(|ext| lower.ends_with(ext.as_str()))
            {
                return Some(lang.language_id.clone());
            }
        }
        None
    }

    /// Language for a path by extension alone, regardless of whether a server
    /// binary is installed (used to give an accurate "not installed" message).
    pub fn language_for_extension(path: &str) -> Option<String> {
        let lower = path.to_lowercase();
        Self::supported_languages()
            .into_iter()
            .find(|l| l.extensions.iter().any(|e| lower.ends_with(e.as_str())))
            .map(|l| l.language_id)
    }

    /// LSP `languageId` for a file (`.tsx` is `typescriptreact`, etc.).
    pub fn lsp_language_id(path: &str, language: &str) -> String {
        let lower = path.to_lowercase();
        let by_ext = [
            (".tsx", "typescriptreact"),
            (".ts", "typescript"),
            (".jsx", "javascriptreact"),
            (".js", "javascript"),
            (".mjs", "javascript"),
            (".cjs", "javascript"),
            (".hpp", "cpp"),
            (".hxx", "cpp"),
            (".cc", "cpp"),
            (".cxx", "cpp"),
            (".cpp", "cpp"),
            (".h", "c"),
            (".c", "c"),
        ];
        for (ext, id) in by_ext {
            if lower.ends_with(ext) {
                return id.to_string();
            }
        }
        language.to_string()
    }

    /// Actionable install hint when a language's server binary is missing.
    pub fn install_hint(language: &str) -> &'static str {
        match language {
            "rust" => "install rust-analyzer (`rustup component add rust-analyzer`)",
            "typescript" => {
                "install it with `npm i -g typescript-language-server typescript` (or add both as devDependencies of the project)"
            }
            "python" => "install it with `npm i -g pyright` or `pip install pyright`",
            "cpp" => "install clangd (LLVM)",
            _ => "install the matching language server",
        }
    }

    /// Resolve a server executable: absolute existing path first, then
    /// project-local `node_modules/.bin`, then `PATH`. Returns a path usable
    /// for spawning.
    pub fn resolve_command(cmd: &str, project_root: &str) -> Option<String> {
        let cmd_path = std::path::Path::new(cmd);
        if cmd_path.is_absolute() && cmd_path.is_file() {
            return Some(cmd.to_string());
        }
        let bin = std::path::Path::new(project_root)
            .join("node_modules")
            .join(".bin");
        let candidates: &[&str] = if cfg!(windows) {
            &[".cmd", ".exe", ""]
        } else {
            &[""]
        };
        for ext in candidates {
            let p = bin.join(format!("{cmd}{ext}"));
            if p.is_file() {
                return Some(p.to_string_lossy().to_string());
            }
        }
        locate_executable(cmd)
    }

    /// Persistent, properly-initialized stdio client for agent tools. Respawns
    /// automatically when the previous server process died.
    pub async fn client(
        &self,
        project_root: &str,
        language: &str,
    ) -> Result<Arc<LspClient>, String> {
        let key = Self::session_key(project_root, language);
        let mut clients = self.clients.lock().await;
        if let Some(c) = clients.get(&key) {
            if c.is_alive() {
                return Ok(c.clone());
            }
        }
        let previous = clients.get(&key).cloned();
        let lang = Self::supported_languages()
            .into_iter()
            .find(|l| l.language_id == language)
            .ok_or_else(|| format!("unsupported language: {language}"))?;
        let cmd = Self::resolve_command(&lang.server_command, project_root)
            .filter(|c| language != "rust" || runs_ok(c, "--version"))
            .ok_or_else(|| {
                format!(
                    "language server '{}' is not installed — {}",
                    lang.server_command,
                    Self::install_hint(language)
                )
            })?;
        let mut last_err = String::new();
        let (cmd, args) = server_launch(&cmd, &lang.server_args);
        for attempt in 0..2 {
            match LspClient::spawn(&cmd, &args, project_root, language).await {
                Ok(c) => {
                    if let Some(previous) = &previous {
                        c.restore_documents(previous).await?;
                    }
                    clients.insert(key, c.clone());
                    return Ok(c);
                }
                Err(e) => {
                    warn!("LSP spawn attempt {} failed: {}", attempt + 1, e);
                    last_err = e;
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
            }
        }
        Err(last_err)
    }

    /// Get the command and args for a given language.
    fn server_info(language: &str) -> Option<LanguageSupport> {
        Self::supported_languages()
            .into_iter()
            .find(|l| l.language_id == language && l.available)
    }

    /// Start an LSP server for the given project directory and language.
    ///
    /// Returns the WebSocket port the bridge is listening on.
    /// If a session already exists for this project+language, returns its port.
    pub async fn start(&self, project_dir: &str, language: &str) -> Result<u16, String> {
        let key = Self::session_key(project_dir, language);

        // Return existing session
        {
            let sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get(&key) {
                if !session.done.load(Ordering::SeqCst) {
                    info!(
                        "LSP session {}/{} already running on port {}",
                        project_dir, language, session.port
                    );
                    return Ok(session.port);
                }
            }
            drop(sessions);
            self.sessions.lock().await.remove(&key);
        }

        // Find server info
        let info = Self::server_info(language)
            .ok_or_else(|| format!("No available LSP server for language: {}", language))?;

        // Bind the TCP listener before returning so the WebSocket client can
        // connect immediately without a bind race.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("Failed to bind a TCP port for LSP bridge: {}", e))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("Failed to read bound LSP bridge port: {}", e))?
            .port();

        info!(
            "Starting LSP server '{}' for {}/{} on port {}",
            info.server_command, project_dir, language, port
        );

        // Spawn the LSP server process
        let command = Self::resolve_command(&info.server_command, project_dir)
            .ok_or_else(|| format!("LSP executable unavailable: {}", info.server_command))?;
        let (command, args) = server_launch(&command, &info.server_args);
        let mut child = piscis_kernel::proc::tokio_command(&command)
            .args(&args)
            .current_dir(project_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Failed to spawn '{}': {}", info.server_command, e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to take LSP stdin handle")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to take LSP stdout handle")?;
        let stderr = child.stderr.take();

        // Spawn the WebSocket bridge
        let language_clone = language.to_string();
        let project_clone = project_dir.to_string();
        let server_name = info.server_command.clone();
        let bridge_name = self.bridge_app_name.clone();
        let done = Arc::new(AtomicBool::new(false));
        let done_task = done.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::lsp::bridge::run_lsp_bridge_with_listener(
                listener,
                stdin,
                stdout,
                stderr,
                &language_clone,
                &project_clone,
                &bridge_name,
            )
            .await
            {
                warn!("LSP bridge for {} exited: {}", server_name, e);
            }
            done_task.store(true, Ordering::SeqCst);
        });

        // Store session
        let session = Arc::new(LspSession {
            project_dir: project_dir.to_string(),
            language: language.to_string(),
            port,
            done,
            child,
        });

        {
            let mut sessions = self.sessions.lock().await;
            sessions.insert(key, session);
        }

        Ok(port)
    }

    /// Stop an LSP session for the given project + language.
    pub async fn stop(&self, project_dir: &str, language: &str) -> Result<(), String> {
        let key = Self::session_key(project_dir, language);
        let session = {
            let mut sessions = self.sessions.lock().await;
            sessions.remove(&key)
        };

        match session {
            Some(_s) => {
                info!(
                    "Stopped LSP session {}/{} (process will be killed on drop)",
                    project_dir, language
                );
                Ok(())
            }
            None => Err(format!(
                "No active LSP session for {}/{}",
                project_dir, language
            )),
        }
    }

    /// Stop all active LSP sessions.
    pub async fn stop_all(&self) {
        self.clients.lock().await.clear();
        let mut sessions = self.sessions.lock().await;
        let count = sessions.len();
        sessions.clear();
        info!("Stopped all {} LSP session(s)", count);
    }
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Known npm TypeScript layouts can launch Node directly, avoiding a .cmd
/// wrapper whose death does not close the server's inherited pipes.
fn server_launch(command: &str, args: &[String]) -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        let shim = std::path::Path::new(command);
        if shim.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .eq_ignore_ascii_case("typescript-language-server.cmd")
        }) {
            if let Some(directory) = shim.parent() {
                let mut candidates =
                    vec![directory.join("node_modules/typescript-language-server/lib/cli.mjs")];
                if let Some(parent) = directory.parent() {
                    candidates.push(parent.join("typescript-language-server/lib/cli.mjs"));
                }
                if let Some(entry) = candidates.into_iter().find(|path| path.is_file()) {
                    let adjacent_node = directory.join("node.exe");
                    let node = if adjacent_node.is_file() {
                        Some(adjacent_node.to_string_lossy().to_string())
                    } else {
                        locate_executable("node")
                    };
                    if let Some(node) = node {
                        let mut launch_args = vec![entry.to_string_lossy().to_string()];
                        launch_args.extend_from_slice(args);
                        return (node, launch_args);
                    }
                }
            }
        }
    }
    (command.into(), args.to_vec())
}

/// Find an executable on `PATH`, then in well-known per-user tool directories.
/// GUI-launched apps often inherit a trimmed `PATH` that lacks `~/.cargo/bin`
/// or the npm global bin dir even though the tools are installed.
fn locate_executable(cmd: &str) -> Option<String> {
    #[cfg(windows)]
    let probe = "where";
    #[cfg(not(windows))]
    let probe = "which";
    if let Ok(out) = piscis_kernel::proc::std_command(probe)
        .arg(cmd)
        .stderr(std::process::Stdio::null())
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            let hits: Vec<&str> = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect();
            // npm installs an extensionless shell shim next to the `.cmd`
            // one; only the latter is spawnable on Windows.
            let spawnable = |l: &str| {
                !cfg!(windows) || {
                    let lower = l.to_lowercase();
                    lower.ends_with(".exe") || lower.ends_with(".cmd") || lower.ends_with(".bat")
                }
            };
            if let Some(line) = hits
                .iter()
                .copied()
                .find(|l| spawnable(l))
                .or_else(|| hits.first().copied())
            {
                return Some(line.to_string());
            }
        }
    }
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    for dir in known_tool_dirs() {
        for ext in exts {
            let p = dir.join(format!("{cmd}{ext}"));
            if p.is_file() {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }
    None
}

fn known_tool_dirs() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut dirs: Vec<PathBuf> = Vec::new();
    let env_dir = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if let Some(c) = env_dir("CARGO_HOME") {
        dirs.push(c.join("bin"));
    }
    let home = env_dir("USERPROFILE").or_else(|| env_dir("HOME"));
    if let Some(h) = &home {
        dirs.push(h.join(".cargo").join("bin"));
        dirs.push(h.join(".local").join("bin"));
        dirs.push(h.join(".npm-global").join("bin"));
        dirs.push(h.join("go").join("bin"));
    }
    if let Some(a) = env_dir("APPDATA") {
        dirs.push(a.join("npm"));
    }
    if let Some(l) = env_dir("LOCALAPPDATA") {
        dirs.push(l.join("pnpm"));
    }
    #[cfg(not(windows))]
    {
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
    }
    dirs
}

fn runs_ok(cmd: &str, arg: &str) -> bool {
    piscis_kernel::proc::std_command(cmd)
        .arg(arg)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Resolve an available Python LSP server:
/// 1. Standalone CLI servers on PATH (`pyright-langserver`, `basedpyright-langserver`, `pylsp`, `jedi-language-server`)
/// 2. Bundled `run-jedi-language-server.py` inside an installed `ms-python.python*` VS Code extension
fn python_lsp_server() -> Option<(String, Vec<String>)> {
    let cli_candidates: &[(&str, &[&str])] = &[
        ("pyright-langserver", &["--stdio"]),
        ("basedpyright-langserver", &["--stdio"]),
        ("pylsp", &[]),
        ("jedi-language-server", &[]),
    ];
    for (cmd, args) in cli_candidates {
        if locate_executable(cmd).is_some() {
            return Some((
                (*cmd).to_string(),
                args.iter().map(|s| (*s).to_string()).collect(),
            ));
        }
    }

    let python = locate_python_interpreter()?;
    let script = find_bundled_jedi_script()?;
    Some((python, vec![script]))
}

fn locate_python_interpreter() -> Option<String> {
    for candidate in ["python", "python3"] {
        if let Some(exe) = locate_executable(candidate) {
            if runs_ok(&exe, "--version") {
                return Some(exe);
            }
        }
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from) {
        let py_root = local.join("Programs").join("Python");
        if let Ok(entries) = std::fs::read_dir(&py_root) {
            let mut dirs: Vec<std::path::PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect();
            dirs.sort();
            dirs.reverse();
            for dir in dirs {
                let exe = dir.join("python.exe");
                if exe.is_file() {
                    let s = exe.to_string_lossy().to_string();
                    if runs_ok(&s, "--version") {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}

fn find_bundled_jedi_script() -> Option<String> {
    use std::path::PathBuf;
    let mut ext_roots: Vec<PathBuf> = Vec::new();
    if let Some(cfg) = std::env::var_os("AGENTZ_CONFIG_DIR").map(PathBuf::from) {
        ext_roots.push(cfg.join("extensions"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        ext_roots.push(appdata.join("com.agentz.desktop").join("extensions"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        ext_roots.push(local.join("com.agentz.desktop").join("extensions"));
    }
    if let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
    {
        ext_roots.push(
            home.join(".config")
                .join("com.agentz.desktop")
                .join("extensions"),
        );
        ext_roots.push(
            home.join("Library")
                .join("Application Support")
                .join("com.agentz.desktop")
                .join("extensions"),
        );
        ext_roots.push(home.join(".vscode").join("extensions"));
    }

    for root in ext_roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut folders: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("ms-python.python"))
            })
            .collect();
        folders.sort();
        folders.reverse();
        for folder in folders {
            for candidate in [
                folder
                    .join("extension")
                    .join("python_files")
                    .join("run-jedi-language-server.py"),
                folder
                    .join("python_files")
                    .join("run-jedi-language-server.py"),
            ] {
                if candidate.is_file() {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}
