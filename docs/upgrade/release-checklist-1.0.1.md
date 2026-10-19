# AgentZ 1.0.1 发布清单

2026-10-20。目标：在 1.0.0 基线之上发布 1.0.1 维护更新，修复状态栏国际化提示与 VS Code 扩展系统交互闭环。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.1`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.1.md` 与 `docs/upgrade/release-checklist-1.0.1.md` 已同步生成。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
- [x] `node scripts/upgrade-plan.mjs validate` 通过。
- [x] `npm --prefix extension-host run smoke` 构建与冒烟测试通过。
- [x] `npx tsc --noEmit` 与 `npm run lint` 零错误通过。
- [x] 状态栏与国际化单测（`IdeStatusBar.test.tsx`、`i18n.test.ts`）通过。
