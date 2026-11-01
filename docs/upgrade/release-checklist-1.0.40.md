# AgentZ 1.0.40 发布检查清单

2026-11-02。基于 1.0.39：自动续跑停止条件与设置项，分叉菜单与完整历史复制。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.40.md` 已更新。
- [x] 引擎引用更新为 `v0.8.71`，与 vendor 内核一致。
- [x] `awaits_user` 单测通过；`cargo check -p agentz-desktop`、`tsc`、`AssistantMessageList` 测试通过。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] agent 回复"要我继续吗？"后本轮停止，不再自动继续。
- [ ] 关闭"计划未完成时自动续跑"后，有未完成待办时模型停下即结束。
- [ ] "从此处分叉"弹出两项菜单，样式跟随主题。
- [ ] "复制完整历史"的新会话包含分叉点之前的全部消息和工具记录。
