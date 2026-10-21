# AgentZ 1.0.5 发布清单

2026-10-21。目标：在 1.0.4 基线之上发布 1.0.5 维护更新，修复编辑器语言服务握手失败与误报「语言服务不可用」。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.5`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.5.md` 与 `docs/upgrade/release-checklist-1.0.5.md` 已同步生成。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
- [x] `cargo test -p piscis-ide-tools --lib` 通过（含 LSP 桥多字节帧测试）。
- [x] `npx vitest run src/services/tauri/lsp.test.ts src/services/tauri/lspSession.test.ts` 通过。
- [x] `npx tsc --noEmit` 通过。
