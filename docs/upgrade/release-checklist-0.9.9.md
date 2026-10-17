# AgentZ 0.9.9 发布清单

2026-10-17。目标：修复 0.9.8 中 Linux 与 macOS 构建失败，发布完整的公开版本。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README 均为 0.9.9。
- [x] CHANGELOG 增加 0.9.9 条目。
- [x] 0.9.8 的失败原因已定位：vendor `piscis-kernel/src/tools/shell.rs` 的 Windows 专属导入在非 Windows 平台未使用，发布构建把告警当错误。修复见 `2f8e737`。

## 本机验证

- [x] `scripts/upgrade-version-check.mjs` 通过。
- [x] `cargo check`（Windows）通过。
- [ ] Linux 与 macOS 编译：本机无法安装对应 Rust 标准库，依赖发布流水线验证。

## 未完成（按用户豁免）

- [ ] 原生桌面旅程（共享、快捷键、OAuth 浏览器回调）。
- [ ] 干净机器的安装、启动、卸载。
- [ ] Windows 安装包签名（AZ-064）。
- [ ] macOS 与 Linux 真实机器验证（AZ-058、AZ-077、AZ-078）。
- [ ] 独立复核。台账中相关任务未标记为 done。

## 发布方式

- 通过 tag `v0.9.9` 触发 `release.yml`。该流水线为 Windows、Linux、macOS 构建安装包。
- 未签名、未验证的安装包会在 Release 说明中标注。
