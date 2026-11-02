# AgentZ 1.0.44 发布检查清单

2026-11-02。基于 1.0.43：超时的 `chat_ui` 表单自动收起。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`README.zh-CN.md`、`RELEASE.md`、`RELEASE.zh-CN.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.44.md` 已更新。
- [x] `tsc` 与 `useInteractiveCards` 单测通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] 让 Agent 弹出 `chat_ui` 表单后不操作，5 分钟后表单从会话底部消失，Agent 继续执行。
- [ ] 在 5 分钟内提交的表单行为不变。
