# AgentZ 0.9.10 发布清单

2026-10-17。目标：让 Linux 与 macOS 发布构建通过。0.9.9 已公开发布，但只有 Windows 安装包，本版本不改写它。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README 均为 0.9.10。
- [x] CHANGELOG 增加 0.9.10 条目，并修正 0.9.9 条目的描述。
- [x] 0.9.9 流水线的失败原因已定位：`agentz-desktop` 库在非 Windows 平台有 4 个未使用项（`task_view_read.rs` 的 `io::Read`、`MAX_BYTES`，`ide.rs` 的两个 `mut`），发布构建把告警当作错误。
- [x] 上述 4 处已做平台门控或 `cfg_attr`，行为不变。
- [x] `cargo test --lib task_view_read`（Windows）6 个测试通过。

## 本机验证

- [x] `scripts/upgrade-version-check.mjs` 通过。
- [ ] Linux 与 macOS 编译：本机无法安装对应 Rust 标准库，依赖发布流水线验证。

## 未完成（按用户豁免）

- [ ] 原生桌面旅程（共享、快捷键、OAuth 浏览器回调）。
- [ ] 干净机器的安装、启动、卸载。
- [ ] Windows 安装包签名（AZ-064）。
- [ ] macOS 与 Linux 真实机器验证（AZ-058、AZ-077、AZ-078）。
- [ ] 独立复核。台账中相关任务未标记为 done。

## 发布方式

- 通过 tag `v0.9.10` 触发 `release.yml`，构建 Windows、Linux、macOS 安装包。
- 未签名、未验证的安装包会在 Release 说明中标注。
