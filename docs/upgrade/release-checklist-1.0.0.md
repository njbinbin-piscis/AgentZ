# AgentZ 1.0.0 发布清单

2026-10-19。目标：完成 AgentZ 1.0 全部 24 项工程与体验任务（`R10-01`–`R10-24`），通过全量本地自动化门禁并签署 1.0.0 正式版基线。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`README.md`、`RELEASE.md` 均统一为 `1.0.0`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.0.md`、`docs/upgrade/release-checklist-1.0.0.md` 与 `docs/upgrade/release-decision-1.0.0.md` 已同步生成。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
- [x] `npx tsc --noEmit` 零错误通过。
- [x] `npx vitest run` 与 `npm run test:e2e` 全量通过。
- [x] `node scripts/perf-harness.mjs` 生产分包与内存/句柄基准通过（主入口 `index-*.js` 约 `706 KB < 1.2 MB`）。
- [x] `node scripts/upgrade-data-inventory.verify.mjs`、`node scripts/upgrade-data-backup.verify.mjs` 与 `npm run test:scripts` 通过。
- [x] `node scripts/upgrade-plan.mjs validate` 与 `node scripts/upgrade-plan.verify.mjs` 零错误通过。
- [x] Rust 核心契约、浏览器世代引用、脱敏导出、预算记账、工作树合并与快照回滚测试通过。

## 外部依赖豁免（ADR-006）

- 按 `docs/upgrade/adr/ADR-006-1.0-engineering-and-ux-baseline.md` 架构决策，商业代码签名证书、跨平台物理真机与外部真实用户 Beta 队列标记为 `waived:ADR-006` 外部基础设施项，不阻塞 1.0.0 本地工程正式基线签署。
