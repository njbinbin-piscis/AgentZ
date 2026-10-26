# AgentZ 1.0.19 发布清单

2026-10-26。目标：在 1.0.18 基线之上发布 1.0.19，修复沙箱本机回环测试的偶发失败（仅测试代码）。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 均统一为 `1.0.19`。
- [x] `CHANGELOG.md`、`docs/upgrade/release-notes-1.0.19.md` 与本清单已同步。
- [x] `cargo test --lib proc::windows_sandbox`（piscis-kernel）：19 项通过，含此前失败的 `sandboxed_command_with_network_allowed_reaches_local_listener`。
- [x] main 上 CI 通过（提交 `c5cf588`）；v1.0.18 Release 构建成功。
- [x] `node scripts/upgrade-version-check.mjs` 通过。
