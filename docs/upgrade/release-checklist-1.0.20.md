# AgentZ 1.0.20 发布清单

2026-10-26。目标：在 1.0.19 基线之上发布 1.0.20，把文件修改 diff 作为过程信息处理（仅最新一轮显示、5 行高滚动、配色可读）。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.20`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.20.md` 与本清单已同步。
- [x] `npx tsc --noEmit` 通过；`npx vitest run`：66 个文件、373 项通过，含「仅最新一轮显示编辑卡片、恢复历史不显示」用例。
- [x] 本版本未改动 Rust 代码。
- [x] `node scripts/upgrade-version-check.mjs` 通过（见发布记录）。

## 待真机冒烟（发布后补记）

- [ ] Agent 修改文件后，当前轮显示编辑卡片，展开后为 5 行高滚动区域。
- [ ] 发送下一个任务后，上一轮的编辑卡片与 diff 消失；切换或恢复历史会话后不显示。
- [ ] 深色与浅色主题下新增（绿）、删除（红）行清晰可读。
