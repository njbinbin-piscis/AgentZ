# AgentZ 1.0.7 发布清单

2026-10-21。目标：在 1.0.6 基线之上发布 1.0.7 维护更新，修复 Windows 沙箱在云盘同步目录（坚果云 Nutstore / OneDrive）下报 `NTSTATUS 0xC0000022` 及开发工具链（`node` 等）调用失败的问题，并闭环全局 UI 缩放下所有弹窗、右键菜单与浮层的防越界定位。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.7`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.7.md` 与 `docs/upgrade/release-checklist-1.0.7.md` 已同步生成。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
- [x] `cargo test -p piscis-kernel --lib proc::windows_sandbox --offline`（16 项测试）与 `cargo test -p piscis-kernel --lib tools::shell --offline`（3 项测试）通过。
- [x] `npx vitest run src/components/DropdownSelect.test.tsx` 与 `npx tsc --noEmit` 通过。
