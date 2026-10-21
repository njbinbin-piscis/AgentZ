# AgentZ 1.0.10 发布清单

2026-10-22。目标：在 1.0.9 基线之上发布 1.0.10，引入后台长任务（显式后台 + 前台超时自动转后台 + 完成自动续跑原会话）、进程等待/输出/终止工具与 CodeZ/WorkZ/状态栏进度 UI，并包含 CodeZ 多会话并行隔离修复。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.10`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.10.md` 与 `docs/upgrade/release-checklist-1.0.10.md` 已同步生成。
- [x] `cargo test -p piscis-kernel --lib`（407 项）通过：含注册表单测、沙箱后台测试（轮次取消后存活、kill 后进程树清空、自动转后台）与工具面端到端测试；并行运行时回环网络用例偶发失败、复跑全绿（既有 flake）。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml --lib`（382 项）通过：含宿主续跑枢纽单测（去重/预算/取消）；`cargo check -p piscis-kernel --target x86_64-unknown-linux-gnu` 交叉编译检查通过（zig cc 交叉工具链）。
- [x] 前端 `npx tsc --noEmit` 与相关 vitest（i18n 对齐、CodeZ、WorkZ、状态栏、诊断，共 115 项）通过；`node scripts/upgrade-version-check.mjs` 通过。

## 待真机冒烟（发布后补记）

- [ ] 后台 `cargo test` 类长命令、dev server 就绪通知、编译失败自动激活原会话续修、切走/新建会话不串扰、停止与删除会话联动。
