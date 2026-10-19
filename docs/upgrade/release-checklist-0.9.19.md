# AgentZ 0.9.19 发布清单

2026-10-19。目标：符号级代码图谱、双轨 Wiki，以及带源码锚点的仓库记忆。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README、RELEASE 均为 0.9.19。
- [x] CHANGELOG 增加 0.9.19 条目。
- [x] `logical_module`、Tauri `ipc_calls`、PageRank、AST `explore_graph` 与笔记 stale 单测通过（`cargo test -p agentz-desktop --lib graph`）。
- [x] `npx tsc --noEmit -p .` 通过。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 发布决策

- 2026-10-19，用户要求提交并发布。验收门禁仍未通过，沿用 0.9.12–0.9.18：不接入 `release-readiness`，通过 tag `v0.9.19` 直接构建安装包。
- 该决定只覆盖 0.9.19，不改变 `RELEASE.md` 对后续版本的门禁要求。

## 未完成（已知）

- [ ] 用户验收：图谱探索、DeepWiki 增量缓存、笔记在源码改动后标 stale、Wiki 菜单三个入口。
- [ ] 发布门禁、安装包签名（AZ-064）、macOS 与 Linux 真机验证。

## 发布方式

- 通过 tag `v0.9.19` 触发 `release.yml`。
