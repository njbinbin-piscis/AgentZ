# AgentZ 1.0.16 发布清单

2026-10-25。目标：在 1.0.15 基线之上发布 1.0.16，加入会话导出导入与文件修改卡片，并修复工具参数与输出处理导致的 Agent 中止。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.16`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.16.md` 与本清单已同步。
- [x] `cargo test --lib`（piscis-kernel）：422 项通过，含 JSON 修复与输出清理新测试。
- [x] `cargo test --test tool_compat_cjk`（piscis-kernel）：12 项通过，含 `code_run` 引号/特殊字符/中文用例与 PowerShell 向原生程序传引号。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml`：386 + 18 + 2 通过，含会话导出导入往返与路径改写测试。
- [x] `npx tsc --noEmit` 通过；`npx vitest run`：59 个文件、343 项通过。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 待真机冒烟（发布后补记）

- [ ] 在一台电脑导出会话并提交，另一台电脑 pull 后导入，继续对话时 Agent 能读到原上下文。
- [ ] Agent 修改文件后对话中出现修改卡片，展开 diff 着色正确，点文件名在编辑器打开。
- [ ] 标签较多时，鼠标移入标签栏显示滚动条，移出后隐藏。
