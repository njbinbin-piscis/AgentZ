# AgentZ 1.0.31 发布检查单

2026-10-28。基于 1.0.30：修复 `@` 引用中文大文件导致的 panic 卡死；MCP 缓存键稳定。

## 发布前检查

- [x] 版本号已更新：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.31.md` 已更新。
- [x] `cargo check` 通过；截断回归测试与 MCP 缓存键测试通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装后人工验证

- [ ] `@` 引用超过 12 KB 的中文文档后发送，正常回复。
- [ ] 连续两个回合只启动一次 REA 进程。
