# AgentZ 1.0.17 发布清单

2026-10-26。目标：在 1.0.16 基线之上发布 1.0.17，加入子代理委派、BotZ 工作区（办公室自有成员）以及运行位置与远程边界提示。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.17`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.17.md` 与本清单已同步。
- [x] `cargo test --lib botz`（src-tauri）：7 项通过，含办公室私有团队、模板迁移与拒绝远程目录。
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- [x] `npx tsc --noEmit` 通过；`npx vitest run`：65 个文件、368 项通过。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 待真机冒烟（发布后补记）

- [ ] 主 Agent 委派子代理，聊天中出现子代理卡片；可写任务在隔离 worktree 中运行，合并前可审阅。
- [ ] 新建办公室直接勾选 Bot；在工具栏增删成员；Bot 卡片「加入办公室」生效。
- [ ] 打开远程项目后标题栏显示 🔌 徽标，切到 WorkZ / BotZ 出现边界横幅，两个按钮可用。
