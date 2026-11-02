# AgentZ 1.0.45 发布检查清单

2026-11-02。基于 1.0.44：对话表单内嵌与恢复、超时修复、焦点刷新不弹提示。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`README.zh-CN.md`、`RELEASE.md`、`RELEASE.zh-CN.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.45.md` 已更新。
- [x] `tsc`、`cargo check -p agentz-desktop`、相关 vitest 通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] `chat_ui` 表单显示在提问位置，之后的消息出现在它下面，滚动时跟随消息。
- [ ] 提交后表单变灰并保留所填内容；不操作等超时后变灰并提示未回答。
- [ ] 恢复历史会话，表单为灰色只读，按钮不可点。
- [ ] 表单等待期间切到别的会话再切回，表单仍可操作。
- [ ] 切换窗口不再弹"设置已保存"；在设置里保存时仍有提示。
