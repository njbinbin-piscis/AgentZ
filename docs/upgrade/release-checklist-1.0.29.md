# AgentZ 1.0.29 发布检查单

2026-10-28。基于 1.0.28：修复后台任务注册表死锁；允许读取计划文件。

## 发布前检查

- [x] 版本号已更新：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.29.md` 已更新。
- [x] 新增并发死锁测试与计划路径测试通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装后人工验证

- [ ] 启动后台任务并打开后台面板，连续对话不卡死，停止有效。
- [ ] 计划模式生成计划后，agent 能用文件工具读取 `.agentz/plans/*.md`。
