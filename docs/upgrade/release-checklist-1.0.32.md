# AgentZ 1.0.32 发布检查单

2026-10-28。基于 1.0.31：composer 选择与预览状态持久化。

## 发布前检查

- [x] 版本号已更新：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.32.md` 已更新。
- [x] `tsc --noEmit`、相关前端测试与 `cargo check` 通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装后人工验证

- [ ] 选择非默认模型、助手、技能后重启，composer 保持原选择。
- [ ] Markdown 文件处于预览状态时退出，重开后仍为预览。
