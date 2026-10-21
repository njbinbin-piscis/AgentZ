# AgentZ 1.0.9 发布清单

2026-10-21。目标：在 1.0.8 基线之上发布 1.0.9，将沙箱强度绑定到安全级别（严格/均衡/开发者），并将非模型配置迁入设置面板「通用」标签页。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.9`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.9.md` 与 `docs/upgrade/release-checklist-1.0.9.md` 已同步生成。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml -p piscis-kernel --lib windows_sandbox`（19 项）、`code_run`（3 项）、`shell`（6 项）、`sandbox_mode_maps`（1 项）通过。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml --lib agents`（2 项）通过。
- [x] `npx tsc --noEmit` 与 `npx vitest run src/i18n/i18n.test.ts` 通过。
