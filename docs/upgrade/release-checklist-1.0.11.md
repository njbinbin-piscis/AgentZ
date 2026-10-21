# AgentZ 1.0.11 发布清单

2026-10-22。目标：在 1.0.10 基线之上发布 1.0.11，修复编辑器 Ctrl+S 保存误关标签与模板对话框 radio 排版，落地工具输出均衡机制并新增 `wait` 工具。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.11`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.11.md` 与 `docs/upgrade/release-checklist-1.0.11.md` 已同步生成。
- [x] `cargo test -p piscis-kernel --lib`（411 项）通过：含输出均衡（超长行省略、按行头尾、落盘保留）与 `wait` 工具（bg_id/会话范围/纯延时）新单测，及 shell 背景端到端回归。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml --lib`（382 项）通过。
- [x] 前端 `npx tsc --noEmit` 与相关 vitest（CodeZ、i18n、组件，共 113 项）通过；`node scripts/upgrade-version-check.mjs` 通过。

## 待真机冒烟（发布后补记）

- [ ] 编辑器：修改后 Ctrl+S、未修改 Ctrl+S、真删除文件、外部修改重载；模板对话框 radio 与文字同行。
- [ ] 后台 `cargo test` 类长命令 + `wait` 等待、dev server 就绪通知、编译失败自动激活原会话续修、切走/新建会话不串扰、停止与删除会话联动。
