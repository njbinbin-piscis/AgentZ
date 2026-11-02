# AgentZ 1.0.46 发布检查清单

2026-11-03。基于 1.0.45，修复链接点击白屏。

## 发布门禁

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、README、RELEASE。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.46.md` 已更新。
- [x] `tsc` 与 linkRouter 单元测试通过。

## 人工验收

- [ ] CodeZ：点击网络链接，内置浏览器标签打开，界面不白屏。
- [ ] CodeZ / WorkZ：点击项目内文件链接，用预览打开；点击 docx/zip 等，用系统程序打开。
- [ ] WorkZ：点击网络链接，在预览区的内置浏览器中打开。
- [ ] BotZ：网络链接用系统浏览器打开，不白屏。
