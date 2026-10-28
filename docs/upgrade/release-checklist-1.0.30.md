# AgentZ 1.0.30 发布检查单

2026-10-28。基于 1.0.29：MCP 启动超时与工具目录预算。

## 发布前检查

- [x] 版本号已更新：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.30.md` 已更新。
- [x] `cargo check` 通过；MCP 工具预算单元测试通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装后人工验证

- [ ] 启用 REA 等大型 MCP 服务器时，发送消息最迟约 1 分钟内开始回复，不再永久卡住。
- [ ] 停止按钮可结束回合。
