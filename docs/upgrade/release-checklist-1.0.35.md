# AgentZ 1.0.35 发布检查清单

2026-10-31。基于 1.0.34：后台续跑使用会话模型、会话级模型选择隔离。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.35.md` 已更新。
- [x] `tsc --noEmit`、`cargo test background_continuation`（5 项）、`sessionProfile` 与 `keepLiveToolSteps` 测试（6 项）通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] 会话 A 选模型 X、会话 B 选模型 Y，来回切换，各自恢复自己的选择。
- [ ] 在会话 B 选非主模型后启动后台任务，任务结束后的自动续跑使用该模型。
- [ ] 只改选择不发送，再让后台任务结束，续跑使用新选择。
