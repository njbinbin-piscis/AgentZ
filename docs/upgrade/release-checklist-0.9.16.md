# AgentZ 0.9.16 发布清单

2026-10-18。目标：修复 Mermaid 空框、分叉重名、停止环动画与弹窗主题跟随。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README、RELEASE 均为 0.9.16。
- [x] CHANGELOG 增加 0.9.16 条目。
- [x] `mermaidSafe.ts` / `Markdown.tsx` / `MarkdownPreview.tsx`：保留样式与标签，HTML 解析，唯一渲染 ID。
- [x] `session.rs`：分叉标题 `(N) … (fork [@ checkpoint])`，含单元测试。
- [x] `index.css` / `ChatComposer.css`：`prefers-reduced-motion` 下进度环仍循环；主题色亮边。
- [x] 远程连接、模板、任务评审、技能修订 CSS：去掉无效 `--agentz-*` 回退深色。
- [x] `npx tsc --noEmit -p .` 通过。
- [x] `cargo test -p agentz-desktop --lib commands::session`：通过（含分叉命名测试）。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 发布决策

- 2026-10-18，用户要求提交并发布。验收门禁仍未通过，沿用 0.9.12–0.9.15：不接入 `release-readiness`，通过 tag `v0.9.16` 直接构建安装包。
- 该决定只覆盖 0.9.16，不改变 `RELEASE.md` 对后续版本的门禁要求。

## 未完成（已知）

- [ ] 用户验收：聊天中 Mermaid、分叉标题、停止环、浅色主题下远程对话框。
- [ ] 发布门禁、安装包签名（AZ-064）、macOS 与 Linux 真机验证。

## 发布方式

- 通过 tag `v0.9.16` 触发 `release.yml`。
