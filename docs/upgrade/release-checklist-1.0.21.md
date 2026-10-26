# AgentZ 1.0.21 发布检查清单

2026-10-26，从 1.0.20 升级到 1.0.21，范围为按会话/项目的用量统计与模型价格设置。

## 已完成

- [x] 版本号：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 已更新为 `1.0.21`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.21.md` 已写入本版内容。
- [x] `npx tsc --noEmit` 通过；全量 vitest 通过（68 个文件、382 个用例）。
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 通过；内核 `session_context_state_defaults_and_updates_roundtrip` 通过。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 人工验收（发布后）

- [ ] 在设置中为当前模型填写价格，发起一次对话，状态栏显示本会话与本项目的 token 和花费。
- [ ] 重启应用、切换项目后，累计数字仍在。
- [ ] 未填价格时只显示 token，不显示花费。
