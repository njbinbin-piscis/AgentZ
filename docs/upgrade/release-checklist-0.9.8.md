# AgentZ 0.9.8 发布清单

2026-10-17。目标：按用户决定发布一个新的公开版本；验收豁免由用户作出，并非独立复核结论。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README 均为 0.9.8。
- [x] `scripts/upgrade-version-check.mjs` 通过（见下方说明）。
- [x] CHANGELOG 增加 0.9.8 条目，并列出已知限制。
- [x] 远端 v0.9.7 已存在且公开发布（指向 `55f3732`），本版本没有覆盖或改写该 tag。
- [x] 本地 main 已基于 `origin/main`（`55f3732`）变基。

## 验证（本地，2026-10-17）

- [x] TypeScript 类型检查通过。
- [x] vitest：53 个文件，343 个测试通过。
- [x] `npm run build` 通过。
- [x] Rust lib 测试：353 通过，0 失败。
- [x] 备份加密验证脚本通过。
- [x] 连接器 OAuth 相关 Rust 测试 17 个通过（含新增的 8 个）。

## 未完成（按用户豁免）

- [ ] 原生桌面旅程（CodeZ/WorkZ 共享、模式快捷键、OAuth 浏览器回调）。
- [ ] 干净机器的安装、启动、卸载。
- [ ] Windows 安装包签名（AZ-064）。
- [ ] macOS 与 Linux 真实机器验证（AZ-058、AZ-077、AZ-078）。
- [ ] 独立复核。台账中相关任务未标记为 done。

## 发布方式

- 发布通过 `release.yml` 由 tag `v0.9.8` 触发，该流水线为 Windows、Linux、macOS 构建安装包。
- 未签名、未验证的安装包会在 Release 说明中明确标注。
