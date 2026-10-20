# AgentZ 1.0.4 发布清单

2026-10-21。目标：在 1.0.3 基线之上发布 1.0.4 维护更新，补齐 Python (`ms-python.python`) 与 Python Debugger (`ms-python.debugpy`) 扩展所需的 Proposed API 与运行时宿主接口。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.4`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.4.md` 与 `docs/upgrade/release-checklist-1.0.4.md` 已同步生成。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
- [x] `npm --prefix extension-host run typecheck` 与 `npm --prefix extension-host run smoke` 通过。
- [x] `npx tsc --noEmit` 通过。
