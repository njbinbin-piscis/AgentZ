# AgentZ 1.0.3 发布清单

2026-10-21。目标：在 1.0.2 基线之上发布 1.0.3 维护更新，修复项目模板规则生效问题并增强 Agent 编码工具自愈能力。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.3`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.3.md` 与 `docs/upgrade/release-checklist-1.0.3.md` 已同步生成。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
- [x] `node scripts/verify-preinstall.mjs` 通过（skills=254, agents=233, commands=336, templates=9）。
- [x] `npx tsc --noEmit` 通过。
- [x] `cargo test`（`piscis-kernel` 工具集与 `agentz-desktop` `project_templates` 单元测试）通过。
