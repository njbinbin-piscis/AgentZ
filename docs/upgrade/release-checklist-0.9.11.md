# AgentZ 0.9.11 发布清单

2026-10-17。目标：连接器 OAuth 令牌改为系统保护的密文保存。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README 均为 0.9.11。
- [x] CHANGELOG 增加 0.9.11 条目。
- [x] OAuth 令牌写入改为 `SecretStore` 密文（`seal_credentials` / `write_credentials`），读取时解封（`unseal_credentials`），无前缀的旧明文仍可读。
- [x] 保存凭据（`connectors_save_credentials`）与 OAuth 写入路径统一经过封装逻辑。创建 API key 连接器的写入不在此范围内。
- [x] 诊断导出对解封后的令牌做脱敏（`export_secret_values`）。
- [x] 无法解封的密文失败关闭：读取得到空凭据，更新拒绝改写文件。
- [x] `cargo test -p agentz-desktop --lib -- connectors diagnostics`（Windows）35 个测试通过。
- [x] `cargo fmt` 已执行。

## 本机验证

- [x] Windows 上密文落盘与往返解封测试通过。
- [ ] Linux 上 Secret Service 路径：CI 环境没有 Secret Service，相关测试标记为 ignore。
- [ ] clippy `-D warnings`：本次改动的文件无新增告警。仓库中其它 11 个文件仍有 18 处既有告警，未在本版本修复。

## 未完成（已知）

- [ ] API key 类连接器仍为明文。
- [ ] 旧明文令牌在下一次写入前仍留在磁盘上。
- [ ] 原生桌面旅程（OAuth 浏览器回调后的密文读取）。
- [ ] 安装包签名（AZ-064）。
- [ ] 独立复核。台账中 AZ-051 未标记为 done。

## 发布方式

- 通过 tag `v0.9.11` 触发 `release.yml`，构建 Windows、Linux、macOS 安装包。
- 未签名、未验证的安装包会在 Release 说明中标注。
