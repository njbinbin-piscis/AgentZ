# AgentZ 0.9.9 发布说明

发布日期：2026-10-17。版本号与 `CHANGELOG.md` 的 0.9.9 条目一致。

## 修复

- Linux 与 macOS 的发布构建恢复可编译。vendor 中 `piscis-kernel` 的 shell 工具导入只在 Windows 下编译，与其使用处一致。0.9.8 只发布了 Windows 安装包。

## 功能

与 0.9.8 相同，见 `release-notes-0.9.8.md`：CodeZ/WorkZ 会话共享、模式切换快捷键、连接器 OAuth2 授权、诊断关联、备份版本校验。

## 已知限制

- Linux 与 macOS 的编译修复只由发布流水线验证，本机未构建。
- Windows 安装包未签名，未在干净机器上做安装、启动、卸载验收。
- 上述新功能的原生桌面旅程没有人工跑过，仅有自动化测试覆盖。
