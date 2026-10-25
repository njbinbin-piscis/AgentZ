# AgentZ 1.0.15 发布清单

2026-10-25。目标：在 1.0.14 基线之上发布 1.0.15，按 VS Code / Cursor 远程开发模型补齐远程工作区，并修复 Windows 下工具的输出截断、编码与中文路径问题。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.15`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.15.md` 与本清单已同步。
- [x] `cargo test --test tool_compat_cjk`（piscis-kernel）：10 项通过。在 `<tmp>/项目 空格/源码/` 下覆盖 grep / glob / list / read 对 UTF-8、UTF-8 BOM、UTF-16LE、GBK 的处理，以及严格、联网、关闭三种沙箱模式下的 cmd 与 PowerShell 中文输出、引号传递、重定向文件编码、原生程序输出、长输出首尾保留与落盘、超长单行省略。
- [x] `cargo test --lib`（piscis-kernel）：全部通过。两项本机回环 curl 测试（`sandboxed_command_with_network_allowed_reaches_local_listener`、`sandboxed_command_networked_mode_reaches_loopback_and_pins_cwd`）在并行负载下偶发失败，单独重跑通过，与 1.0.12 记录一致。
- [x] `cargo test`（piscis-ide-tools）：9 通过。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml`：384 + 18 + 2 通过。
- [x] `cargo test`（agentz-task-state）：165 通过；1 项 `merge` 测试因本机 Git 作者名与夹具 `Merge Fixture` 不一致失败，与 1.0.14 相同，与本次改动无关。
- [x] `npx tsc --noEmit` 通过；`npx vitest run`：58 个文件、340 项通过。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 待真机冒烟（发布后补记）

- [ ] 连接 SSH 远端项目，生成快速 Wiki，确认远端 `.agentz/REPO_WIKI.md` 出现且能在编辑器中打开。
- [ ] 远端项目打开代码地图，确认提交热力有数据。
- [ ] 远程会话里让 Agent 调用 `read_lints` 和 `lsp`，确认结果来自远端。
- [ ] 中文系统代码页（936）的 Windows 上，cmd 输出 GBK 与 UTF-8 混合内容时显示正常。
