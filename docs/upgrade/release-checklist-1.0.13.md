# AgentZ 1.0.13 发布清单

2026-10-24。目标：在 1.0.12 基线之上发布 1.0.13，让编辑器标签显示完整文件名，多出来的标签横向滚动。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.13`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.13.md` 与 `docs/upgrade/release-checklist-1.0.13.md` 已同步生成。
- [x] 本次只改 `src/workspaces/codez/IDE.css`，没有 Rust 或 TypeScript 行为变化，未重跑 cargo / tsc / vitest。`node scripts/upgrade-version-check.mjs` 通过。
- [x] 用与正式样式相同的规则，在 420px 宽的标签条上核对：五个文件名均按全文占宽（无省略号），`scrollWidth` 666 大于 `clientWidth` 420，滚到末尾后后面的文件名完整可见。

## 待真机冒烟（发布后补记）

- [ ] 桌面应用里打开多个长文件名标签，滚轮与底部滚动条可用，切换当前标签后该标签滚入可见区域。
