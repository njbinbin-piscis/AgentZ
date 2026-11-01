# AgentZ 1.0.38 发布检查清单

2026-11-01。基于 1.0.37：弹出层背景、后台任务面板、远程后台命令、远程服务关闭、目录列表提示。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.38.md` 已更新。
- [x] `tsc --noEmit`；`file_list` 测试 2 项；远程后台脚本测试 2 项。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] CodeZ 会话列表、状态下拉、任务面板比主背景稍亮，能分开。
- [ ] CodeZ 状态栏和 WorkZ 任务栏都能打开后台任务列表，并看到来源标记。
- [ ] `dirs_only` 列出只有文件的中文目录时，显示「无子目录（文件数未知）」。
- [ ] 关闭已连接远程项目的 AgentZ 后，远端 agentz-server 退出。
