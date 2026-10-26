# AgentZ 1.0.22 发布检查清单

2026-10-26，从 1.0.21 升级到 1.0.22，范围为后台任务终端标签与中文文案修复。

## 已完成

- [x] 版本号：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 已更新为 `1.0.22`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.22.md` 已写入本版内容。
- [x] `npx tsc --noEmit`、全量 vitest 与 `node scripts/upgrade-version-check.mjs` 见发布记录。

## 人工验收（发布后）

- [ ] 启动一个后台任务，点击“在终端显示”，底部面板出现“后台输出”标签并实时显示输出。
- [ ] 任务结束后标签停止刷新，标题行显示退出码。
- [ ] 中文界面状态栏显示“本会话 / 本项目”，没有问号。
