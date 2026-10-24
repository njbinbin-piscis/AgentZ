# AgentZ 1.0.12 发布清单

2026-10-24。目标：在 1.0.11 基线之上发布 1.0.12，把右侧变更改为与 Git 分开的 Agent 修改审阅，并修正产物把读取目录和已删除临时文件算进去的问题。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.12`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.12.md` 与 `docs/upgrade/release-checklist-1.0.12.md` 已同步生成。
- [x] `cargo test -p piscis-kernel --lib`：414 项中 412 通过、1 项忽略。`sandboxed_command_with_network_allowed_reaches_local_listener` 首次因本机 AppContainer 回环 curl 退出码 56 失败，立即重跑通过；与本次改动无关。日志净值对账与按路径保留/撤销新单测在通过项中。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml --lib`：382 通过、7 忽略，含非 Git 目录的多轮快照回滚。
- [x] 前端 `npx tsc --noEmit` 与 vitest（57 个文件、338 项，含变更面板与产物收集新单测）通过；`node scripts/upgrade-version-check.mjs` 通过。

## 待真机冒烟（发布后补记）

- [ ] Agent 新建、再改、再删临时文件：变更与产物在刷新后与磁盘一致；已删除条目不能点开。
- [ ] 仅 `file_list` / `file_read` 不产生产物；单文件与全部保留/撤销在刷新和重启后仍生效。
- [ ] Git 源代码管理界面与变更面板互不覆盖。
