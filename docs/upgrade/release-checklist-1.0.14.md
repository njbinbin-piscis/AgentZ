# AgentZ 1.0.14 发布清单

2026-10-24。目标：在 1.0.13 基线之上发布 1.0.14，补上空文件夹的 Git 初始化，并消除执行记录库锁定导致的整轮失败。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.14`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.14.md` 与 `docs/upgrade/release-checklist-1.0.14.md` 已同步生成。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml --lib git_init_tests`：空目录可以 `git init`，已位于上层仓库内的子目录会被拒绝。
- [x] `cargo test -p agentz-task-state --lib`：165 通过。`open_reader_does_not_block_event_append` 通过。另有 1 项 `merge` 测试因本机 Git 作者名与夹具不一致失败，与本次 `runs.db` 改动无关。
- [x] 前端 vitest：`GitPanel.test.tsx` 与 `i18n.test.ts` 通过。`node scripts/upgrade-version-check.mjs` 通过。

## 待真机冒烟（发布后补记）

- [ ] 无仓库文件夹点「初始化新仓库」后出现分支列表；已在上层仓库内的子文件夹会提示路径而不是再建一层。
- [ ] 连续写执行记录的 Agent 轮次不再因 `database is locked` 中止。
