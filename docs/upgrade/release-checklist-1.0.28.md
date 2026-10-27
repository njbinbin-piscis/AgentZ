# AgentZ 1.0.28 发布检查单

2026-10-28。基于 1.0.27：后台任务重构（回合内通知、service 模式、超时与退出清理）。

## 发布前检查

- [x] 版本号已更新：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.28.md` 已更新。
- [x] `cargo check --tests` 与 `tsc --noEmit` 通过；后台任务相关单元测试通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装后人工验证

- [ ] 回合运行中后台任务完成：通知在当前回合内处理，不出现 `task queued/running` 错误。
- [ ] 启动 dev server（`service: true`）后 agent 继续对话、做浏览器测试不卡死。
- [ ] 退出应用后后台 server 进程被结束。
