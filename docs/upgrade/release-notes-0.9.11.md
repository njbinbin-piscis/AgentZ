# AgentZ 0.9.11 发布说明

发布日期：2026-10-17。版本号与 `CHANGELOG.md` 的 0.9.11 条目一致。

## 安全

- 连接器 OAuth 令牌（`access_token`、`refresh_token`）不再以明文写入 `credentials.json`，改为密文保存。
- 密钥由系统保护：Windows 使用 DPAPI，macOS 使用 Keychain，Linux 使用 Secret Service。这与应用内设置中的密钥使用同一套机制（`SecretStore`），没有引入新的凭据存储方式。
- 诊断导出会对已解封的令牌明文做脱敏。
- 失败关闭：系统凭据库不可用时，OAuth 令牌写入直接报错，文件保持原样，不会退回明文。

## 迁移

- 0.9.8 至 0.9.10 写入的明文令牌仍可读取。
- 明文令牌会在该连接器的令牌被更新、清除或保存凭据时改写为密文。在此之前，旧明文仍留在磁盘上。
- 不需要手动操作。

## 已知限制

- API key 类连接器（`api_key`）仍以明文保存，本版本未改动。
- Linux 上 OAuth 令牌保存需要运行中的 Secret Service（如 GNOME Keyring、KWallet）。该路径只有 CI 验证，没有在真实 Linux 桌面上验证。
- 凭据存储相关的单元测试在 Linux CI 上被跳过，因为 CI 环境没有 Secret Service。Windows 上这些测试已通过。
- 安装包未签名。macOS 安装包未公证，首次打开需要手动允许。
- 0.9.8 至 0.9.10 的已知限制仍然适用。
