# AgentZ 1.0.37 发布检查清单

2026-11-01。基于 1.0.36：会话级浏览器、通知带会话标题、BotZ 加入房间下拉框。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.37.md` 已更新。
- [x] `tsc --noEmit`、`cargo test browser::`（4 项通过，1 项需本机 Chromium 已忽略）。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
- [x] 通知预览与收件箱测试通过（Rust 2 项，前端 2 项）。

## 安装版验收

- [ ] 两个 CodeZ 会话分别打开网页，互不跳转、互不点击。
- [ ] WorkZ 任务右侧打开浏览器，Agent 的 `browser` 操作落在该任务页面上。
- [ ] 任务完成后，通知里能看到会话标题和最后回复。
- [ ] BotZ 里「加入房间」下拉框能选中房间。
