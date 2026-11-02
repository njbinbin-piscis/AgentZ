# AgentZ 1.0.43 发布检查清单

2026-11-02。基于 1.0.42：HTML"在内置浏览器打开"按钮、子代理回退主模型。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`README.zh-CN.md`、`RELEASE.md`、`RELEASE.zh-CN.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.43.md` 已更新。
- [x] `tsc`、`cargo check -p agentz-desktop --tests`、`browserTab`/i18n 单测通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] 打开 `.html` 文件，点"在内置浏览器打开"，浏览器标签页加载该文件且脚本可运行。
- [ ] 将 flash 模型的 Key 设为无效，让主代理调用子代理，子代理改用主模型完成任务。
- [ ] 子代理改过文件后失败时不会自动重试。
