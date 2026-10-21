# AgentZ 1.0.6 发布清单

2026-10-21。目标：在 1.0.5 基线之上发布 1.0.6 维护更新，修复 `WorkZ` / `CodeZ` 输入框下拉列表显示不全或被遮挡的问题，以及编辑器字号不随字体设置缩放且无法调节的问题。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.6`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.6.md` 与 `docs/upgrade/release-checklist-1.0.6.md` 已同步生成。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
- [x] `npx vitest run src/components/DropdownSelect.test.tsx` 通过（含下拉定位防越界、Portal 与编辑器/终端字号缩放测试）。
- [x] `npx tsc --noEmit` 通过。
