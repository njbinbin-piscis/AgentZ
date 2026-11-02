# AgentZ 1.0.41 发布检查清单

2026-11-02。基于 1.0.40：OpenCode Go 服务商、多实例安全（项目锁、后台租约、设置冲突、数据库等待）、通知去重与清空。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`README.zh-CN.md`、`RELEASE.md`、`RELEASE.zh-CN.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.41.md` 已更新。
- [x] 引擎引用更新为 `v0.8.72`，与 vendor 内核一致。
- [x] 内核 `opencode_provider` 单测、收件箱 / `os_lock` / `project_lock` 单测通过；`cargo check -p agentz-desktop --tests`、`tsc`、`vitest` 通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] 选择 OpenCode Go 并填入 Key 后，发消息不再返回 `MissingSessionID`。
- [ ] 开两个窗口打开同一项目，第二个被拒绝并提示。
- [ ] 一个窗口运行后台任务时，打开第二个窗口不会把它标为已中断。
- [ ] 两个窗口先后保存设置，后保存者收到冲突提示并看到最新值。
- [ ] 通知下拉每个任务只有一行；"清空"可清空；中文界面下标题、刷新、清空和铃铛提示均为中文。
