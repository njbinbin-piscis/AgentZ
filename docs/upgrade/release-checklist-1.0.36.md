# AgentZ 1.0.36 发布检查清单

2026-10-31。基于 1.0.35：后台任务通知不再被模型复述给用户。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.36.md` 已更新。
- [x] `cargo test background_continuation`（5 项）通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] 一轮对话进行中，后台服务变为就绪或结束，助手回复里不再出现“该通知为……回执”之类的说明。
- [ ] 通知与当前请求有关时，助手仍然会根据任务结果继续下一步。
