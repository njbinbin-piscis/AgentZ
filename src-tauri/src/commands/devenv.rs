//! Lightweight development-environment clues.
//!
//! On project open / first turn of a session we detect the project's tech
//! stacks from marker files, probe the matching toolchain (PATH first, then
//! the usual per-user tool directories), and hand the result to the agent as a
//! hint plus an implicit todo. The agent decides what is relevant and does the
//! actual configuration itself; nothing here installs anything.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use piscis_core::host::EventSink;
use piscis_kernel::agent::messages::AgentEvent;
use piscis_kernel::agent::plan::{PlanStore, PlanTodoItem};

pub const DEVENV_TODO_ID: &str = "devenv-setup";
const CACHE_TTL: Duration = Duration::from_secs(600);
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const SCAN_DEPTH: usize = 2;
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "dist",
    "build",
    ".venv",
    "venv",
    "vendor",
    ".agentz",
    ".idea",
    ".gradle",
    "__pycache__",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolStatus {
    pub id: String,
    pub stack: String,
    pub required: bool,
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub required_version: Option<String>,
    pub version_ok: Option<bool>,
    pub reason: String,
    pub install_hint: String,
}

impl ToolStatus {
    fn is_problem(&self) -> bool {
        !self.found || self.version_ok == Some(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DevEnvReport {
    pub os: String,
    pub stacks: Vec<String>,
    pub tools: Vec<ToolStatus>,
}

impl DevEnvReport {
    pub fn problems(&self, dismissed: &HashSet<String>) -> Vec<&ToolStatus> {
        self.tools
            .iter()
            .filter(|t| t.is_problem() && !dismissed.contains(&t.id))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DevEnvState {
    #[serde(default)]
    dismissed: BTreeSet<String>,
}

fn state_path(project: &Path) -> PathBuf {
    project.join(".agentz").join("devenv.json")
}

pub fn load_dismissed(project: &Path) -> HashSet<String> {
    std::fs::read_to_string(state_path(project))
        .ok()
        .and_then(|raw| serde_json::from_str::<DevEnvState>(&raw).ok())
        .map(|s| s.dismissed.into_iter().collect())
        .unwrap_or_default()
}

pub fn dismiss(project: &Path, ids: &[String]) -> Result<(), String> {
    let mut state: DevEnvState = std::fs::read_to_string(state_path(project))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    state.dismissed.extend(ids.iter().cloned());
    let path = state_path(project);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

// ── stack detection ────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
struct Detected {
    stacks: BTreeSet<&'static str>,
    /// Markers worth reading for version requirements, by stack.
    rust_toolchain: Option<PathBuf>,
    package_json: Option<PathBuf>,
    nvmrc: Option<PathBuf>,
    python_version: Option<PathBuf>,
    go_mod: Option<PathBuf>,
    global_json: Option<PathBuf>,
    has_gradlew: bool,
    has_mvnw: bool,
    has_gradle: bool,
    has_maven: bool,
    has_cmake: bool,
    has_makefile: bool,
    has_yarn_lock: bool,
    has_pnpm_lock: bool,
    has_bun_lock: bool,
    has_tauri: bool,
    has_flutter: bool,
}

fn detect(root: &Path) -> Detected {
    let mut d = Detected::default();
    walk(root, 0, &mut d);
    d
}

fn walk(dir: &Path, depth: usize, d: &mut Detected) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            if depth < SCAN_DEPTH && !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                walk(&path, depth + 1, d);
            }
            continue;
        }
        let lower = name.to_lowercase();
        match lower.as_str() {
            "cargo.toml" => {
                d.stacks.insert("rust");
            }
            "rust-toolchain" | "rust-toolchain.toml" => {
                d.rust_toolchain.get_or_insert(path);
            }
            "package.json" => {
                d.stacks.insert("node");
                d.package_json.get_or_insert(path);
            }
            "tsconfig.json" => {
                d.stacks.insert("node");
            }
            ".nvmrc" | ".node-version" => {
                d.nvmrc.get_or_insert(path);
            }
            "yarn.lock" => d.has_yarn_lock = true,
            "pnpm-lock.yaml" => d.has_pnpm_lock = true,
            "bun.lockb" | "bun.lock" => d.has_bun_lock = true,
            "pyproject.toml" | "requirements.txt" | "pipfile" | "setup.py" | "uv.lock" => {
                d.stacks.insert("python");
            }
            ".python-version" => {
                d.stacks.insert("python");
                d.python_version.get_or_insert(path);
            }
            "go.mod" => {
                d.stacks.insert("go");
                d.go_mod.get_or_insert(path);
            }
            "pom.xml" => {
                d.stacks.insert("java");
                d.has_maven = true;
            }
            "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => {
                d.stacks.insert("java");
                d.has_gradle = true;
            }
            "gradlew" | "gradlew.bat" => d.has_gradlew = true,
            "mvnw" | "mvnw.cmd" => d.has_mvnw = true,
            "global.json" => {
                d.stacks.insert("dotnet");
                d.global_json.get_or_insert(path);
            }
            "cmakelists.txt" => {
                d.stacks.insert("cpp");
                d.has_cmake = true;
            }
            "makefile" => {
                // Plain Makefiles are too common to imply a C/C++ project.
                d.has_makefile = true;
            }
            "compile_commands.json" => {
                d.stacks.insert("cpp");
            }
            "tauri.conf.json" => {
                d.has_tauri = true;
                d.stacks.insert("tauri");
            }
            "composer.json" => {
                d.stacks.insert("php");
            }
            "gemfile" => {
                d.stacks.insert("ruby");
            }
            "pubspec.yaml" => {
                d.stacks.insert("dart");
                if std::fs::read_to_string(&path)
                    .map(|s| s.contains("flutter:"))
                    .unwrap_or(false)
                {
                    d.has_flutter = true;
                }
            }
            _ => {
                if lower.ends_with(".csproj") || lower.ends_with(".sln") || lower.ends_with(".fsproj")
                {
                    d.stacks.insert("dotnet");
                }
            }
        }
    }
}

// ── executable lookup ──────────────────────────────────────────────────────

fn exe_suffixes() -> &'static [&'static str] {
    if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ".com"]
    } else {
        &[""]
    }
}

fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from)
}

#[cfg(windows)]
fn glob_children(base: &Path, tail: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(read) = std::fs::read_dir(base) {
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.push(if tail.is_empty() { p } else { p.join(tail) });
            }
        }
    }
    out
}

/// Directories where developer tools usually live but which a GUI-launched
/// process often lacks on `PATH`.
pub fn known_tool_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(c) = env_dir("CARGO_HOME") {
        dirs.push(c.join("bin"));
    }
    if let Some(j) = env_dir("JAVA_HOME") {
        dirs.push(j.join("bin"));
    }
    if let Some(g) = env_dir("GOROOT") {
        dirs.push(g.join("bin"));
    }
    if let Some(g) = env_dir("GOPATH") {
        dirs.push(g.join("bin"));
    }
    let home = env_dir("USERPROFILE").or_else(|| env_dir("HOME"));
    if let Some(h) = &home {
        dirs.push(h.join(".cargo").join("bin"));
        dirs.push(h.join(".local").join("bin"));
        dirs.push(h.join(".npm-global").join("bin"));
        dirs.push(h.join("go").join("bin"));
        dirs.push(h.join(".dotnet").join("tools"));
        dirs.push(h.join(".pub-cache").join("bin"));
        dirs.push(h.join("flutter").join("bin"));
        dirs.push(h.join(".bun").join("bin"));
        dirs.push(h.join("scoop").join("shims"));
    }
    if let Some(a) = env_dir("APPDATA") {
        dirs.push(a.join("npm"));
        dirs.push(a.join("Composer").join("vendor").join("bin"));
    }
    if let Some(l) = env_dir("LOCALAPPDATA") {
        dirs.push(l.join("pnpm"));
        dirs.push(l.join("Microsoft").join("WinGet").join("Links"));
    }
    #[cfg(windows)]
    {
        for pf in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(p) = env_dir(pf) {
                dirs.push(p.join("nodejs"));
                dirs.push(p.join("dotnet"));
                dirs.push(p.join("CMake").join("bin"));
                dirs.push(p.join("LLVM").join("bin"));
                dirs.push(p.join("Go").join("bin"));
                dirs.push(p.join("Git").join("cmd"));
                dirs.extend(glob_children(&p.join("Java"), "bin"));
                dirs.extend(glob_children(&p.join("Eclipse Adoptium"), "bin"));
                dirs.extend(glob_children(&p.join("Microsoft"), "bin"));
                dirs.extend(glob_children(&p.join("PHP"), ""));
            }
        }
        dirs.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
    }
    #[cfg(not(windows))]
    {
        for p in [
            "/usr/local/bin",
            "/opt/homebrew/bin",
            "/usr/local/go/bin",
            "/snap/bin",
            "/usr/local/share/dotnet",
        ] {
            dirs.push(PathBuf::from(p));
        }
    }
    dirs
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend(known_tool_dirs());
    dirs
}

pub fn locate(cmd: &str) -> Option<PathBuf> {
    locate_in(cmd, &search_dirs())
}

fn locate_in(cmd: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        for ext in exe_suffixes() {
            let p = dir.join(format!("{cmd}{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn run_version(path: &Path, args: &[&str]) -> Option<String> {
    let path = path.to_path_buf();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = piscis_kernel::proc::std_command(&path)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .output();
        let _ = tx.send(out);
    });
    let out = rx.recv_timeout(PROBE_TIMEOUT).ok()?.ok()?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() && parse_version(&text).is_none() {
        return None;
    }
    parse_version(&text).or(Some(String::new()))
}

/// First `N(.N){0,3}` token in the text.
pub fn parse_version(text: &str) -> Option<String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() && (i == 0 || !(bytes[i - 1].is_ascii_digit() || bytes[i - 1] == '.')) {
            let mut j = i;
            let mut dots = 0;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || (bytes[j] == '.' && dots < 3)) {
                if bytes[j] == '.' {
                    if j + 1 >= bytes.len() || !bytes[j + 1].is_ascii_digit() {
                        break;
                    }
                    dots += 1;
                }
                j += 1;
            }
            let tok: String = bytes[i..j].iter().collect();
            if tok.contains('.') {
                return Some(tok);
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

fn version_tuple(v: &str) -> Vec<u32> {
    v.trim_start_matches(['v', '='])
        .split('.')
        .map(|p| {
            p.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(0)
        })
        .collect()
}

/// `found >= required`, comparing only as many components as `required` has.
pub fn version_satisfies(found: &str, required: &str) -> bool {
    let f = version_tuple(found);
    let r = version_tuple(required);
    for (i, rv) in r.iter().enumerate() {
        let fv = f.get(i).copied().unwrap_or(0);
        if fv > *rv {
            return true;
        }
        if fv < *rv {
            return false;
        }
    }
    true
}

// ── toolchain table ────────────────────────────────────────────────────────

struct ToolSpec {
    id: &'static str,
    stack: &'static str,
    /// Executable names tried in order.
    exes: &'static [&'static str],
    version_args: &'static [&'static str],
    required: bool,
    reason: &'static str,
    required_version: Option<String>,
}

fn spec(
    id: &'static str,
    stack: &'static str,
    exes: &'static [&'static str],
    version_args: &'static [&'static str],
    required: bool,
    reason: &'static str,
) -> ToolSpec {
    ToolSpec {
        id,
        stack,
        exes,
        version_args,
        required,
        reason,
        required_version: None,
    }
}

fn read_trim(path: &Option<PathBuf>) -> Option<String> {
    path.as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
}

fn rust_channel(d: &Detected) -> Option<String> {
    let raw = read_trim(&d.rust_toolchain)?;
    for line in raw.lines() {
        let l = line.trim();
        let candidate = l
            .strip_prefix("channel")
            .map(|r| r.trim_start_matches([' ', '=', '"']).trim_end_matches('"').trim())
            .unwrap_or(l);
        if candidate.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Some(candidate.to_string());
        }
    }
    None
}

fn node_requirement(d: &Detected) -> Option<String> {
    if let Some(raw) = read_trim(&d.nvmrc) {
        let v = raw.trim().trim_start_matches('v').to_string();
        if v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Some(v);
        }
    }
    let raw = read_trim(&d.package_json)?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let range = json.get("engines")?.get("node")?.as_str()?;
    let digits: String = range
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!digits.is_empty()).then_some(digits)
}

fn python_requirement(d: &Detected) -> Option<String> {
    let raw = read_trim(&d.python_version)?;
    let v = raw.lines().next()?.trim().to_string();
    v.chars().next().is_some_and(|c| c.is_ascii_digit()).then_some(v)
}

fn go_requirement(d: &Detected) -> Option<String> {
    let raw = read_trim(&d.go_mod)?;
    raw.lines()
        .find_map(|l| l.trim().strip_prefix("go ").map(|v| v.trim().to_string()))
}

fn dotnet_requirement(d: &Detected) -> Option<String> {
    let raw = read_trim(&d.global_json)?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    json.get("sdk")?.get("version")?.as_str().map(str::to_string)
}

fn specs_for(d: &Detected) -> Vec<ToolSpec> {
    let mut v: Vec<ToolSpec> = Vec::new();
    let has = |s: &str| d.stacks.contains(s);
    if has("rust") || has("tauri") {
        let mut rustc = spec("rustc", "rust", &["rustc"], &["--version"], true, "Cargo.toml");
        rustc.required_version = rust_channel(d);
        v.push(rustc);
        v.push(spec("cargo", "rust", &["cargo"], &["--version"], true, "Cargo.toml"));
        v.push(spec(
            "rust-analyzer",
            "rust",
            &["rust-analyzer"],
            &["--version"],
            false,
            "code navigation (lsp tool)",
        ));
    }
    if has("node") || has("tauri") {
        let mut node = spec("node", "node", &["node"], &["--version"], true, "package.json");
        node.required_version = node_requirement(d);
        v.push(node);
        if d.has_pnpm_lock {
            v.push(spec("pnpm", "node", &["pnpm"], &["--version"], true, "pnpm-lock.yaml"));
        } else if d.has_yarn_lock {
            v.push(spec("yarn", "node", &["yarn"], &["--version"], true, "yarn.lock"));
        } else if d.has_bun_lock {
            v.push(spec("bun", "node", &["bun"], &["--version"], true, "bun lockfile"));
        } else {
            v.push(spec("npm", "node", &["npm"], &["--version"], true, "package.json"));
        }
        v.push(spec(
            "typescript-language-server",
            "node",
            &["typescript-language-server"],
            &["--version"],
            false,
            "code navigation (lsp tool)",
        ));
    }
    if has("python") {
        let mut py = spec(
            "python",
            "python",
            &["python3", "python", "py"],
            &["--version"],
            true,
            "Python project files",
        );
        py.required_version = python_requirement(d);
        v.push(py);
        v.push(spec(
            "pyright-langserver",
            "python",
            &["pyright-langserver"],
            &["--version"],
            false,
            "code navigation (lsp tool)",
        ));
    }
    if has("go") {
        let mut go = spec("go", "go", &["go"], &["version"], true, "go.mod");
        go.required_version = go_requirement(d);
        v.push(go);
        v.push(spec("gopls", "go", &["gopls"], &["version"], false, "code navigation (lsp tool)"));
    }
    if has("java") {
        v.push(spec("java", "java", &["java"], &["-version"], true, "Maven/Gradle project"));
        v.push(spec("javac", "java", &["javac"], &["-version"], true, "JDK (not just a JRE)"));
        if d.has_maven && !d.has_mvnw {
            v.push(spec("mvn", "java", &["mvn"], &["-version"], true, "pom.xml without wrapper"));
        }
        if d.has_gradle && !d.has_gradlew {
            v.push(spec("gradle", "java", &["gradle"], &["--version"], true, "Gradle build without wrapper"));
        }
    }
    if has("dotnet") {
        let mut dn = spec("dotnet", "dotnet", &["dotnet"], &["--version"], true, ".NET project files");
        dn.required_version = dotnet_requirement(d);
        v.push(dn);
    }
    if has("cpp") {
        if d.has_cmake {
            v.push(spec("cmake", "cpp", &["cmake"], &["--version"], true, "CMakeLists.txt"));
        }
        v.push(spec(
            "c-compiler",
            "cpp",
            &["gcc", "clang", "cc", "cl"],
            &["--version"],
            true,
            "C/C++ sources",
        ));
        v.push(spec("clangd", "cpp", &["clangd"], &["--version"], false, "code navigation (lsp tool)"));
    }
    if has("php") {
        v.push(spec("php", "php", &["php"], &["--version"], true, "composer.json"));
        v.push(spec("composer", "php", &["composer"], &["--version"], true, "composer.json"));
    }
    if has("ruby") {
        v.push(spec("ruby", "ruby", &["ruby"], &["--version"], true, "Gemfile"));
        v.push(spec("bundle", "ruby", &["bundle"], &["--version"], true, "Gemfile"));
    }
    if has("dart") {
        if d.has_flutter {
            v.push(spec("flutter", "dart", &["flutter"], &["--version"], true, "pubspec.yaml (flutter)"));
        } else {
            v.push(spec("dart", "dart", &["dart"], &["--version"], true, "pubspec.yaml"));
        }
    }
    v
}

fn install_hint(id: &str) -> String {
    let (win, mac, linux): (&str, &str, &str) = match id {
        "rustc" | "cargo" => (
            "winget install Rustlang.Rustup (then `rustup default stable`)",
            "brew install rustup && rustup-init",
            "curl https://sh.rustup.rs -sSf | sh",
        ),
        "rust-analyzer" => (
            "rustup component add rust-analyzer",
            "rustup component add rust-analyzer",
            "rustup component add rust-analyzer",
        ),
        "node" | "npm" => (
            "winget install OpenJS.NodeJS.LTS",
            "brew install node",
            "install Node LTS via your package manager or nvm",
        ),
        "pnpm" => ("npm i -g pnpm", "npm i -g pnpm (or brew install pnpm)", "npm i -g pnpm"),
        "yarn" => ("npm i -g yarn", "npm i -g yarn", "npm i -g yarn"),
        "bun" => (
            "winget install Oven-sh.Bun",
            "brew install oven-sh/bun/bun",
            "curl -fsSL https://bun.sh/install | bash",
        ),
        "typescript-language-server" => (
            "npm i -g typescript-language-server typescript",
            "npm i -g typescript-language-server typescript",
            "npm i -g typescript-language-server typescript",
        ),
        "python" => (
            "winget install Python.Python.3.12",
            "brew install python",
            "apt install python3 python3-pip / dnf install python3",
        ),
        "pyright-langserver" => ("npm i -g pyright", "npm i -g pyright", "npm i -g pyright"),
        "go" => ("winget install GoLang.Go", "brew install go", "apt install golang-go / dnf install golang"),
        "gopls" => (
            "go install golang.org/x/tools/gopls@latest",
            "go install golang.org/x/tools/gopls@latest",
            "go install golang.org/x/tools/gopls@latest",
        ),
        "java" | "javac" => (
            "winget install EclipseAdoptium.Temurin.21.JDK",
            "brew install openjdk",
            "apt install default-jdk / dnf install java-latest-openjdk-devel",
        ),
        "mvn" => ("winget install Apache.Maven", "brew install maven", "apt install maven"),
        "gradle" => ("winget install Gradle.Gradle", "brew install gradle", "sdk install gradle (SDKMAN)"),
        "dotnet" => (
            "winget install Microsoft.DotNet.SDK.8",
            "brew install --cask dotnet-sdk",
            "apt install dotnet-sdk-8.0 / dnf install dotnet-sdk-8.0",
        ),
        "cmake" => ("winget install Kitware.CMake", "brew install cmake", "apt install cmake"),
        "c-compiler" => (
            "winget install Microsoft.VisualStudio.2022.BuildTools (C++ workload) or install LLVM",
            "xcode-select --install",
            "apt install build-essential / dnf groupinstall 'Development Tools'",
        ),
        "clangd" => ("winget install LLVM.LLVM", "brew install llvm", "apt install clangd"),
        "php" => ("winget install PHP.PHP", "brew install php", "apt install php-cli"),
        "composer" => ("winget install Composer.Composer", "brew install composer", "apt install composer"),
        "ruby" | "bundle" => ("winget install RubyInstallerTeam.Ruby", "brew install ruby", "apt install ruby-full"),
        "dart" => ("winget install Google.DartSDK", "brew install dart-sdk", "apt install dart"),
        "flutter" => (
            "winget install Google.Flutter (or git clone flutter)",
            "brew install --cask flutter",
            "snap install flutter --classic",
        ),
        _ => ("see the tool's official install guide", "see the tool's official install guide", "see the tool's official install guide"),
    };
    if cfg!(windows) {
        win.to_string()
    } else if cfg!(target_os = "macos") {
        mac.to_string()
    } else {
        linux.to_string()
    }
}

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn probe(spec: ToolSpec) -> ToolStatus {
    let mut path: Option<PathBuf> = None;
    for exe in spec.exes {
        if let Some(p) = locate(exe) {
            path = Some(p);
            break;
        }
    }
    let mut status = ToolStatus {
        id: spec.id.to_string(),
        stack: spec.stack.to_string(),
        required: spec.required,
        found: false,
        path: path.as_ref().map(|p| p.to_string_lossy().to_string()),
        version: None,
        required_version: spec.required_version.clone(),
        version_ok: None,
        reason: spec.reason.to_string(),
        install_hint: install_hint(spec.id),
    };
    let Some(p) = path else {
        return status;
    };
    // A rustup proxy exists even when the component is not installed, so the
    // tool only counts as present when it actually runs.
    match run_version(&p, spec.version_args) {
        Some(ver) => {
            status.found = true;
            if !ver.is_empty() {
                status.version = Some(ver.clone());
                if let Some(req) = &spec.required_version {
                    status.version_ok = Some(version_satisfies(&ver, req));
                }
            }
        }
        None => {
            status.found = false;
        }
    }
    status
}

pub fn scan_blocking(project: &Path) -> DevEnvReport {
    let detected = detect(project);
    let specs = specs_for(&detected);
    let handles: Vec<_> = specs
        .into_iter()
        .map(|s| std::thread::spawn(move || probe(s)))
        .collect();
    let tools = handles.into_iter().filter_map(|h| h.join().ok()).collect();
    DevEnvReport {
        os: os_name().to_string(),
        stacks: detected.stacks.iter().map(|s| s.to_string()).collect(),
        tools,
    }
}

type CacheEntry = (Instant, DevEnvReport);

fn cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    static CACHE: once_cell::sync::Lazy<Mutex<HashMap<String, CacheEntry>>> =
        once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));
    &CACHE
}

/// Cached scan (10 min TTL); `force` bypasses and refreshes the cache.
pub async fn scan(project: &Path, force: bool) -> DevEnvReport {
    let key = project.to_string_lossy().to_string();
    if !force {
        if let Ok(guard) = cache().lock() {
            if let Some((at, report)) = guard.get(&key) {
                if at.elapsed() < CACHE_TTL {
                    return report.clone();
                }
            }
        }
    }
    let path = project.to_path_buf();
    let report = tokio::task::spawn_blocking(move || scan_blocking(&path))
        .await
        .unwrap_or_default();
    if let Ok(mut guard) = cache().lock() {
        guard.insert(key, (Instant::now(), report.clone()));
    }
    report
}

pub fn render_report(report: &DevEnvReport, dismissed: &HashSet<String>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "OS: {}\nDetected stacks: {}\n",
        report.os,
        if report.stacks.is_empty() {
            "none".to_string()
        } else {
            report.stacks.join(", ")
        }
    ));
    for t in &report.tools {
        let state = if t.is_problem() {
            if !t.found {
                "MISSING"
            } else {
                "VERSION MISMATCH"
            }
        } else {
            "ok"
        };
        let dismissed_tag = if dismissed.contains(&t.id) { " (dismissed)" } else { "" };
        out.push_str(&format!(
            "- {} [{}{}]{} {}{}{}\n",
            t.id,
            state,
            if t.required { ", required" } else { ", optional" },
            dismissed_tag,
            t.version.clone().unwrap_or_default(),
            t.required_version
                .as_ref()
                .map(|r| format!(" (needs >= {r})"))
                .unwrap_or_default(),
            t.path
                .as_ref()
                .map(|p| format!(" @ {p}"))
                .unwrap_or_default()
        ));
        if t.is_problem() && !dismissed.contains(&t.id) {
            out.push_str(&format!("    needed for: {}; install: {}\n", t.reason, t.install_hint));
        }
    }
    out
}

/// Prompt section handed to the agent. `None` when nothing is wrong.
pub fn hint_section(report: &DevEnvReport, dismissed: &HashSet<String>) -> Option<String> {
    let problems = report.problems(dismissed);
    if problems.is_empty() {
        return None;
    }
    let mut s = String::from("## Development environment clues\n");
    s.push_str(&format!(
        "OS: {}. Detected stacks: {}.\nMissing or outdated tools:\n",
        report.os,
        report.stacks.join(", ")
    ));
    for t in &problems {
        let what = if t.found { "version too old" } else { "not found" };
        s.push_str(&format!(
            "- {} ({}, {}): needed for {}{}; install: {}\n",
            t.id,
            if t.required { "required" } else { "optional" },
            what,
            t.reason,
            t.required_version
                .as_ref()
                .map(|r| format!(", needs >= {r}"))
                .unwrap_or_default(),
            t.install_hint
        ));
    }
    s.push_str(
        "\nThese are clues, not commands. Judge for yourself whether they matter for the user's \
         current request (for example, ignore them for a casual greeting or a question that needs \
         no toolchain). When the task needs a tool, configure it yourself with `shell` (the user \
         confirms commands), then call `devenv` with action `resolve` to re-check and clear the \
         implicit todo. Use `devenv` action `dismiss` for items the user does not want.\n",
    );
    Some(s)
}

pub fn todo_content(report: &DevEnvReport, dismissed: &HashSet<String>, zh: bool) -> Option<String> {
    let problems = report.problems(dismissed);
    if problems.is_empty() {
        return None;
    }
    let names: Vec<&str> = problems.iter().map(|t| t.id.as_str()).collect();
    Some(if zh {
        format!(
            "环境线索：{} 缺失或版本过旧（与当前任务相关时再配置，完成后调用 devenv resolve）",
            names.join("、")
        )
    } else {
        format!(
            "Environment clue: {} missing or outdated (configure only if the task needs it, then call devenv resolve)",
            names.join(", ")
        )
    })
}

fn injected_sessions() -> &'static Mutex<HashSet<String>> {
    static SET: once_cell::sync::Lazy<Mutex<HashSet<String>>> =
        once_cell::sync::Lazy::new(|| Mutex::new(HashSet::new()));
    &SET
}

fn emit_plan(sink: &Arc<dyn EventSink>, session_id: &str, items: Vec<PlanTodoItem>) {
    let payload = serde_json::to_value(AgentEvent::PlanUpdate { items }).unwrap_or_default();
    sink.emit_session(session_id, "agent_event", payload);
}

/// Per-turn hook for the agent chat: returns the prompt clue section and, the
/// first time a session sees problems, adds the implicit todo to its plan.
pub async fn context_for_turn(
    project: &Path,
    session_id: &str,
    plan_store: &PlanStore,
    sink: &Arc<dyn EventSink>,
    zh: bool,
) -> Option<String> {
    let report = scan(project, false).await;
    let dismissed = load_dismissed(project);
    let hint = hint_section(&report, &dismissed)?;
    let first = injected_sessions()
        .lock()
        .map(|mut set| set.insert(session_id.to_string()))
        .unwrap_or(false);
    if first {
        if let Some(content) = todo_content(&report, &dismissed, zh) {
            let items = {
                let mut state = plan_store.lock().await;
                let entry = state.entry(session_id.to_string()).or_default();
                if !entry.iter().any(|i| i.id == DEVENV_TODO_ID) {
                    entry.push(PlanTodoItem {
                        id: DEVENV_TODO_ID.to_string(),
                        content,
                        status: "pending".to_string(),
                    });
                }
                entry.clone()
            };
            emit_plan(sink, session_id, items);
        }
    }
    Some(hint)
}

/// Remove the implicit todo from a session's plan (after resolve/dismiss).
pub async fn clear_todo(plan_store: &PlanStore, sink: &Arc<dyn EventSink>, session_id: &str) {
    let items = {
        let mut state = plan_store.lock().await;
        match state.get_mut(session_id) {
            Some(list) if list.iter().any(|i| i.id == DEVENV_TODO_ID) => {
                list.retain(|i| i.id != DEVENV_TODO_ID);
                list.clone()
            }
            _ => return,
        }
    };
    emit_plan(sink, session_id, items);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions_from_tool_output() {
        assert_eq!(parse_version("rustc 1.98.1 (48a229cea 2026-09-01)").as_deref(), Some("1.98.1"));
        assert_eq!(parse_version("v22.3.0").as_deref(), Some("22.3.0"));
        assert_eq!(parse_version("go version go1.22.4 windows/amd64").as_deref(), Some("1.22.4"));
        assert_eq!(parse_version("openjdk version \"21.0.2\" 2024-01-16").as_deref(), Some("21.0.2"));
        assert_eq!(parse_version("no digits"), None);
    }

    #[test]
    fn version_comparison_uses_required_precision() {
        assert!(version_satisfies("22.3.0", "20"));
        assert!(version_satisfies("1.78.0", "1.75"));
        assert!(!version_satisfies("1.70.0", "1.75"));
        assert!(!version_satisfies("18.19.0", "20"));
        assert!(version_satisfies("3.12.1", "3.12"));
    }

    #[test]
    fn detects_stacks_and_requirements() {
        let dir = std::env::temp_dir().join(format!("agentz-devenv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("web")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules").join("x")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname=\"x\"").unwrap();
        std::fs::write(
            dir.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.80.0\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("web").join("package.json"), r#"{"engines":{"node":">=20.1"}}"#).unwrap();
        std::fs::write(dir.join("node_modules").join("x").join("go.mod"), "module x\ngo 1.22").unwrap();
        std::fs::write(dir.join("web").join("pnpm-lock.yaml"), "").unwrap();

        let d = detect(&dir);
        assert!(d.stacks.contains("rust"));
        assert!(d.stacks.contains("node"));
        assert!(!d.stacks.contains("go"), "node_modules must be skipped");
        assert_eq!(rust_channel(&d).as_deref(), Some("1.80.0"));
        assert_eq!(node_requirement(&d).as_deref(), Some("20.1"));
        let ids: Vec<&str> = specs_for(&d).iter().map(|s| s.id).collect();
        assert!(ids.contains(&"pnpm"));
        assert!(ids.contains(&"cargo"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn locate_finds_executables_in_extra_dirs() {
        let dir = std::env::temp_dir().join(format!("agentz-devenv-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name = if cfg!(windows) { "faketool.exe" } else { "faketool" };
        std::fs::write(dir.join(name), "").unwrap();
        assert!(locate_in("faketool", std::slice::from_ref(&dir)).is_some());
        assert!(locate_in("nothere", std::slice::from_ref(&dir)).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hint_lists_only_non_dismissed_problems() {
        let tool = |id: &str, found: bool| ToolStatus {
            id: id.into(),
            stack: "rust".into(),
            required: true,
            found,
            path: None,
            version: None,
            required_version: None,
            version_ok: None,
            reason: "Cargo.toml".into(),
            install_hint: "x".into(),
        };
        let report = DevEnvReport {
            os: "linux".into(),
            stacks: vec!["rust".into()],
            tools: vec![tool("cargo", false), tool("rustc", true)],
        };
        let none: HashSet<String> = HashSet::new();
        let hint = hint_section(&report, &none).unwrap();
        assert!(hint.contains("cargo"));
        assert!(!hint.contains("rustc ("));
        let dismissed: HashSet<String> = ["cargo".to_string()].into_iter().collect();
        assert!(hint_section(&report, &dismissed).is_none());
    }
}
