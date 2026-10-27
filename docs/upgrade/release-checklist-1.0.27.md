# AgentZ 1.0.27 发布检查单

2026-10-27。相对 1.0.26:后台续跑回合对界面可见,发送被占用时自动排队/重试。

## 自动检查

- [x] 版本号同步到 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`(仅 `agentz-desktop`)、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.27.md` 已更新。
- [x] `cargo check` 与 `tsc --noEmit` 通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 需要人工验证

- [ ] 后台任务完成触发续跑期间,界面显示运行中;此时发送消息会排队而不是报错。
- [ ] 续跑期间点击停止能取消该回合。
