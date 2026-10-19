# AgentZ 1.0.2 发布清单

2026-10-20。目标：在 1.0.1 基线之上发布 1.0.2 维护更新，修复 rust-analyzer 扩展激活失败。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.2`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.2.md` 与 `docs/upgrade/release-checklist-1.0.2.md` 已同步生成。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
- [x] `npm --prefix extension-host run typecheck` 通过。
- [x] `npm --prefix extension-host run smoke` 通过。
- [x] 已安装的 `rust-lang.rust-analyzer`、`loong-vsx.rust-analyzer`、`chenxc.node-run-button` 在本地扩展宿主中激活成功。
