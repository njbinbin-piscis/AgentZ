# AgentZ 0.9.10 发布说明

发布日期：2026-10-17。版本号与 `CHANGELOG.md` 的 0.9.10 条目一致。

## 修复

- Linux 与 macOS 的发布构建恢复可编译。0.9.9 中两处发布构建仍失败：
  - vendor 中 `piscis-kernel` 的 shell 导入，已在 0.9.9 修复。
  - 桌面端 `task_view_read.rs` 的 `io::Read` 与 `MAX_BYTES`，以及 `ide.rs` 中两个不需要 `mut` 的变量，在非 Windows 平台上未使用。本版本仅对这些项做了平台门控，行为不变。
- 0.9.8、0.9.9 只发布了 Windows 安装包。

## 功能

与 0.9.8 相同，见 `release-notes-0.9.8.md`：CodeZ/WorkZ 会话共享、模式切换快捷键、连接器 OAuth2 授权、诊断关联、备份版本校验。

## 本次发布包含的安装包

- Windows x64 与 arm64（NSIS，未签名）。
- Linux x86_64 与 aarch64：`.deb`、`.AppImage`、`.rpm`（aarch64 为 `.deb` 与 `.AppImage`）。
- macOS universal 与 aarch64：`.dmg`，以及 `.app.tar.gz`（未签名、未公证）。

以上安装包由发布流水线构建成功。Linux 与 macOS 安装包未在真实机器上安装或运行过。

## 已知限制

- Linux 与 macOS 的编译只由发布流水线验证，本机无法构建这些目标。
- macOS 安装包未签名、未公证，首次打开会被系统拦截，需要手动允许。
- 所有安装包都未在干净机器上做安装、启动、卸载验收。
- 上述新功能的原生桌面旅程没有人工跑过，仅有自动化测试覆盖。
