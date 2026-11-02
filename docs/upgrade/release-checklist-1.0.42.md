# AgentZ 1.0.42 发布检查清单

2026-11-02。基于 1.0.41：HTML 预览允许运行脚本（隔离源，无网络）。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`README.zh-CN.md`、`RELEASE.md`、`RELEASE.zh-CN.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.42.md` 已更新。
- [x] `tsc` 与 `webviewSecurity` 单测通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] 打开带按钮点击、定时器等 JS 交互的 `.html` 文件，预览中可正常操作。
- [ ] 预览页内 `fetch` 外部地址被拒绝，页面无法访问应用主界面。
- [ ] Agent 文件预览里的 HTML 同样可交互。
