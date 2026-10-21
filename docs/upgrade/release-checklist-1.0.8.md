# AgentZ 1.0.8 发布清单

2026-10-21。目标：在 1.0.7 基线之上发布 1.0.8 维护更新，修复坚果云同步目录下 `code_run` / `shell` 沙箱闪退，并提高浅色主题终端输入对比度。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.8`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.8.md` 与 `docs/upgrade/release-checklist-1.0.8.md` 已同步生成。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml -p piscis-kernel --lib windows_sandbox`（17 项）通过。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml -p piscis-kernel --lib code_run`（3 项）与 `shell`（6 项）通过。
- [x] `npx vitest run src/workspaces/codez/Terminal.lifecycle.test.tsx`（7 项）通过。
