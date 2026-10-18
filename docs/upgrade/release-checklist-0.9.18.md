# AgentZ 0.9.18 发布清单

2026-10-19。目标：WorkZ 消息气泡改为相对宽度，Mermaid 随气泡缩放。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README、RELEASE 均为 0.9.18。
- [x] CHANGELOG 增加 0.9.18 条目。
- [x] `Agent.css`：Agent 气泡宽 80%；用户消息最宽 90% 且靠右。
- [x] `Markdown.css`：Mermaid SVG 宽度 100%。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 发布决策

- 2026-10-19，用户要求提交并发布。验收门禁仍未通过，沿用 0.9.12–0.9.17：不接入 `release-readiness`，通过 tag `v0.9.18` 直接构建安装包。
- 该决定只覆盖 0.9.18，不改变 `RELEASE.md` 对后续版本的门禁要求。

## 未完成（已知）

- [ ] 用户验收：最大化后 Agent 气泡与 Mermaid 变宽；用户消息仍靠右且左侧留白。
- [ ] 发布门禁、安装包签名（AZ-064）、macOS 与 Linux 真机验证。

## 发布方式

- 通过 tag `v0.9.18` 触发 `release.yml`。
