# AgentZ 1.0.18 发布清单

2026-10-26。目标：在 1.0.17 基线之上发布 1.0.18，提高文件修改 diff 的可读性，并在后台任务输出被重定向时给出提示。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.18`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.18.md` 与本清单已同步。
- [x] `npx tsc --noEmit` 通过；`npx vitest run src/workspaces/codez src/components`：25 个文件、134 项通过，含重定向识别测试。
- [x] `cargo test --lib`（piscis-kernel）：428 项通过；`sandboxed_command_with_network_allowed_reaches_local_listener` 在本机失败（沙箱联网访问本机端口），与本次仅修改工具说明文字无关。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 待真机冒烟（发布后补记）

- [ ] Agent 修改文件后，对话中 diff 的新增、删除行在深色和浅色主题下都清晰可读。
- [ ] 运行带 `> file 2>&1` 的后台命令，「查看输出」显示重定向提示，点击文件名在编辑器打开。
