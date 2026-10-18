# AgentZ 0.9.14 发布清单

2026-10-18。目标：修复 Windows 上 WorkZ 会话列表重复显示同一个项目。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README、RELEASE 均为 0.9.14。
- [x] CHANGELOG 增加 0.9.14 条目。
- [x] `projects.rs` 加载、添加、移除时去掉 `\\?\` 前缀并合并重复项，新增回归测试。
- [x] WorkZ 读取已保存的范围、最近目录、最后任务时去掉前缀。
- [x] `cargo test -p agentz-desktop --lib commands::projects`：3 个测试通过。

## 发布决策

- 2026-10-18，用户要求检查后发布新版本。验收门禁仍未通过，沿用 0.9.12、0.9.13 的做法：不接入 `release-readiness`，通过 tag `v0.9.14` 直接构建安装包。
- 该决定只覆盖 0.9.14，不改变 `RELEASE.md` 对后续版本的门禁要求。

## 未完成（已知）

- [ ] 用户验收：在装有旧版 `workz.json` 的 Windows 机器上确认列表只显示一个项目。
- [ ] 发布门禁、安装包签名（AZ-064）、macOS 与 Linux 真机验证。

## 发布方式

- 通过 tag `v0.9.14` 触发 `release.yml`。
