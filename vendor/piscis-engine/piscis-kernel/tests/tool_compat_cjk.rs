//! Encoding / path compatibility of the built-in file and shell tools on a
//! workspace whose path contains CJK characters and spaces. Run on every OS;
//! the shell cases exercise each Windows sandbox mode.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use piscis_kernel::agent::tool::{SandboxMode, Tool, ToolContext, ToolSettings};
use piscis_kernel::tools::file_list::FileListTool;
use piscis_kernel::tools::file_read::FileReadTool;
use piscis_kernel::tools::file_search::FileSearchTool;
use serde_json::json;

const NEEDLE: &str = "查找目标Needle";

fn ctx(root: std::path::PathBuf, mode: SandboxMode) -> ToolContext {
    ToolContext {
        session_id: "cjk-compat".into(),
        workspace_root: root,
        bypass_permissions: false,
        settings: Arc::new(ToolSettings {
            sandbox_mode: mode,
            ..ToolSettings::default()
        }),
        max_iterations: None,
        memory_owner_id: "test".into(),
        pool_session_id: None,
        tool_use_id: None,
        cancel: Arc::new(AtomicBool::new(false)),
        loop_halt: None,
    }
}

/// `<tmp>/项目 空格/源码/` with UTF-8, UTF-8 BOM, UTF-16LE and GBK files.
fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("项目 空格");
    let src = root.join("源码");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("utf8.txt"), format!("第一行\n{NEEDLE} utf8\n")).unwrap();
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend(format!("{NEEDLE} bom\n").as_bytes());
    std::fs::write(src.join("bom.txt"), bom).unwrap();
    let mut u16le = vec![0xFF, 0xFE];
    for unit in format!("{NEEDLE} utf16\r\n").encode_utf16() {
        u16le.extend(unit.to_le_bytes());
    }
    std::fs::write(src.join("utf16.txt"), u16le).unwrap();
    let gbk_text = format!("{NEEDLE} gbk\r\n");
    let (gbk, _, _) = encoding_rs::GBK.encode(&gbk_text);
    std::fs::write(src.join("gbk.txt"), gbk.as_ref()).unwrap();
    std::fs::write(src.join("中文文件名.rs"), format!("// {NEEDLE} name\n")).unwrap();
    (temp, root)
}

#[tokio::test]
async fn grep_finds_cjk_text_in_every_common_encoding_under_cjk_path() {
    let (_t, root) = fixture();
    let c = ctx(root.clone(), SandboxMode::default());
    let r = FileSearchTool
        .call(json!({"action": "grep", "pattern": NEEDLE}), &c)
        .await
        .unwrap();
    assert!(!r.is_error, "{}", r.content);
    for tag in ["utf8", "bom", "utf16", "gbk", "name"] {
        assert!(r.content.contains(&format!("{NEEDLE} {tag}")), "missing {tag}:\n{}", r.content);
    }

    let abs = root.join("源码").to_string_lossy().to_string();
    let r = FileSearchTool
        .call(json!({"action": "grep", "pattern": "gbk", "path": abs}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("gbk.txt"), "{}", r.content);

    let slash = root.join("源码").to_string_lossy().replace('\\', "/");
    let r = FileSearchTool
        .call(json!({"action": "grep", "pattern": "utf8", "path": slash}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("utf8.txt"), "{}", r.content);
}

#[tokio::test]
async fn glob_and_list_handle_cjk_names() {
    let (_t, root) = fixture();
    let c = ctx(root.clone(), SandboxMode::default());
    let r = FileSearchTool
        .call(json!({"action": "glob", "pattern": "**/中文*.rs"}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("中文文件名.rs"), "{}", r.content);
    let r = FileSearchTool
        .call(json!({"action": "glob", "pattern": "源码/*.txt"}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("gbk.txt"), "{}", r.content);
    let r = FileListTool
        .call(json!({"path": "源码"}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("中文文件名.rs"), "{}", r.content);
}

#[tokio::test]
async fn read_decodes_each_encoding() {
    let (_t, root) = fixture();
    let c = ctx(root, SandboxMode::default());
    for f in ["utf8.txt", "bom.txt", "utf16.txt", "gbk.txt", "中文文件名.rs"] {
        let r = FileReadTool
            .call(json!({"path": format!("源码/{f}")}), &c)
            .await
            .unwrap();
        assert!(r.content.contains(NEEDLE), "{f}:\n{}", r.content);
    }
}

#[cfg(windows)]
mod windows_shell {
    use super::*;
    use piscis_kernel::tools::shell::ShellTool;

    async fn run(mode: SandboxMode, interpreter: &str, command: &str) -> String {
        let (_t, root) = fixture();
        let c = ctx(root, mode);
        let r = ShellTool::default()
            .call(
                json!({"command": command, "interpreter": interpreter, "timeout": 60, "cwd": "源码"}),
                &c,
            )
            .await
            .unwrap();
        // Console decoding of test stderr is code-page dependent; keep a
        // byte-exact UTF-8 copy for inspection.
        if let Ok(dir) = std::env::var("CJK_PROBE_DIR") {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(std::path::Path::new(&dir).join("shell.txt"))
                .unwrap();
            writeln!(f, "##### {mode:?} {interpreter} `{command}`\n{}\n", r.content).unwrap();
        }
        r.content
    }

    const ALL_MODES: [SandboxMode; 3] =
        [SandboxMode::Isolated, SandboxMode::Networked, SandboxMode::Disabled];
    /// Strict-mode AppContainers cannot list ancestors of the workspace, which
    /// cmd path canonicalization, PowerShell locations and child-process
    /// launches require.
    const USER_MODES: [SandboxMode; 2] = [SandboxMode::Networked, SandboxMode::Disabled];

    async fn every_mode(interpreter: &str, command: &str, expect: &[&str]) {
        in_modes(&ALL_MODES, interpreter, command, expect).await
    }

    async fn in_modes(modes: &[SandboxMode], interpreter: &str, command: &str, expect: &[&str]) {
        let mut failures = Vec::new();
        for &mode in modes {
            let out = run(mode, interpreter, command).await;
            let missing: Vec<&&str> = expect.iter().filter(|e| !out.contains(**e)).collect();
            if !missing.is_empty() || out.contains('\u{FFFD}') {
                failures.push(format!("== {mode:?}: missing {missing:?}\n{out}"));
            }
        }
        assert!(failures.is_empty(), "{interpreter} `{command}`\n{}", failures.join("\n"));
    }

    #[tokio::test]
    async fn cmd_prints_cjk_and_keeps_quoting() {
        every_mode(
            "cmd",
            "echo \"quoted & kept\" & set X=1&& echo !X!%X%",
            &["\"quoted & kept\"", "!X!%X%"],
        )
        .await;
        // The AppContainer console ignores `chcp`, so strict-mode cmd
        // built-ins stay on the ANSI code page.
        in_modes(&USER_MODES, "cmd", "echo 中文输出 & echo \"引号 & 保留\"", &["中文输出", "\"引号 & 保留\""])
            .await;
        in_modes(&USER_MODES, "cmd", "cd & dir /b", &["源码", "中文文件名.rs"]).await;
    }

    #[tokio::test]
    async fn cmd_redirect_writes_utf8_files() {
        in_modes(
            &USER_MODES,
            "cmd",
            "echo 写入文件> out.txt & powershell -NoProfile -Command \"[IO.File]::ReadAllText('out.txt', [Text.Encoding]::UTF8)\"",
            &["写入文件"],
        )
        .await;
    }

    #[tokio::test]
    async fn powershell_prints_cjk_and_cwd() {
        every_mode("powershell", "Write-Output '中文输出'", &["中文输出"]).await;
        in_modes(
            &USER_MODES,
            "powershell",
            "(Get-Location).Path; Get-ChildItem -Name",
            &["源码", "中文文件名.rs"],
        )
        .await;
    }

    #[tokio::test]
    async fn powershell_reads_utf8_file_content() {
        in_modes(
            &USER_MODES,
            "powershell",
            "Get-Content -Encoding UTF8 utf8.txt; Select-String -Path *.txt -Pattern utf8 | % { $_.Line }",
            &[NEEDLE],
        )
        .await;
    }

    #[tokio::test]
    async fn native_exe_output_through_powershell_is_utf8() {
        in_modes(&USER_MODES, "powershell", "cmd /c echo 原生程序输出", &["原生程序输出"]).await;
        in_modes(&USER_MODES, "powershell", "git --version; where.exe cmd", &["git version"]).await;
    }

    #[tokio::test]
    async fn long_output_is_truncated_on_char_boundaries_and_spilled() {
        let cmd = "1..6000 | % { \"第${_}行 中文内容填充 abcdefghijklmnopqrstuvwxyz\" }";
        for mode in [SandboxMode::Networked, SandboxMode::Disabled] {
            let out = run(mode, "powershell", cmd).await;
            assert!(out.contains("第1行"), "{out}");
            assert!(out.contains("第6000行"), "{out}");
            assert!(out.contains("omitted"), "{out}");
            assert!(out.contains("Full output saved to"), "{out}");
            assert!(out.len() < 64 * 1024, "presentation too large: {}", out.len());
            assert!(!out.contains('\u{FFFD}'), "{out}");
        }
    }

    #[tokio::test]
    async fn single_huge_line_is_elided() {
        let out = run(SandboxMode::Networked, "powershell", "'汉' * 200000").await;
        assert!(out.contains("elided"), "{}", &out[..out.len().min(2000)]);
        assert!(out.len() < 64 * 1024);
        assert!(!out.contains('\u{FFFD}'));
    }
}
