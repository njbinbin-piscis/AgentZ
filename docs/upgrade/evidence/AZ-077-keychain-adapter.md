# AZ-077 原生 Keychain 适配进展

2026-10-06，状态 doing；不是平台验收通过记录。

新增 `security/macos_keychain.rs`，仅macOS编译，直接调用Security/CoreFoundation的SecItem API，不新增Cargo依赖、不把密钥交给子进程。当前选择本地非同步Keychain条目；账号为随机UUID，SecItemAdd不覆盖已有条目。条目写入后读回32字节并比对，失败清理，清理错误包含非秘密条目身份。查询使用 `kSecUseAuthenticationUIFail`，需要交互时失败关闭；该参数已被Apple标记deprecated，后续应迁移到禁用交互的LAContext，当前不声称采用了最新认证接口。[Apple UI策略](https://developer.apple.com/documentation/security/ksecuseauthenticationuifail)、[同步属性](https://developer.apple.com/documentation/security/ksecattrsynchronizable)

磁盘保存 `AGENTZ-KEY-V1:KEYCHAIN\0` 与引用；Windows仍使用DPAPI头。SecretStore接入创建、读取与发布失败清理回调，旧32字节主密钥迁移保持密钥值不变。新增跨平台引用拒绝测试，证明不会调用当前平台后端或生成替代密钥。

CoreFoundation对象以RAII释放；查询字典借用的值在同步OS调用期间保持存活。错误输出不包含主密钥或凭据。API内存与ABI已对照本地core-foundation-sys声明核查，Boolean使用u8。

## 验证

- 当前Windows最终secrets专项：16 passed、0 failed。包括旧DPAPI迁移、保护失败、原子发布失败清理、清理双错误及跨平台引用拒绝。不能算作macOS API验证。
- Windows产品内核严格Clippy：通过（原生macOS分支不在其编译范围）；三个Rust文件rustfmt检查通过。
- 新增默认ignored的macOS临时凭据测试，覆盖读写、撤销、非法引用、旧主密钥迁移、重开与撤销后引用不替换；包含临时条目清理。尚未执行。
- 提供 `node scripts/upgrade-macos-keychain-check.mjs 1.98.1` 隔离类型检查入口，仅包含实际适配源文件与最小依赖，不链接framework、不运行macOS，也不是整个桌面应用交叉构建。

首次交叉检查失败在工具链环境：默认rustc1.99.0，而镜像下载的Apple标准库为1.98.1，E0514；未归为实现通过。已安装独立1.98.1工具链，未切换默认工具链。随后上述入口对aarch64-apple-darwin进行隔离adapter及测试类型检查，exit0；结果保存在 `.cache/az077-adapter-check-87b93610-5931-4e40-a715-accf82e9a5bb/result.json` 与 `check.log`。没有遗留构建。此结果不证明framework链接、实际系统权限或原生API运行通过。

## 剩余

原生macOS读写/锁定/撤销/损坏/迁移及签名身份变化尚未验收；本地login Keychain与data-protection Keychain的发行选择需要结合真实签名配置确认。不得运行整个未清理外部条目的旧secret测试集来冒称平台验收；先显式运行新增临时条目用例并核对清理结果。固定候选脚本尚不包含AZ-077增量。

## 2026-10-16 本地清理边界复核

`SecItemDelete` 对同一随机引用的重试现在把 `errSecItemNotFound`（-25300）视作清理完成；新增临时条目测试在首次删除后再次删除。读取仍对缺失条目失败关闭，现存磁盘引用不会生成替代密钥。适配源码 SHA-256 为 `640f75f66d47fe3f1c54a2b3cca9779b7457620bf371c4ae9205a4dbf0ae83bd`。隔离 Apple 目标源码及测试类型检查 `node scripts/upgrade-macos-keychain-check.mjs 1.98.1` exit 0，`rustfmt --check` 和差异空白检查通过；原生测试仍未运行。

当前查询未设置 `kSecAttrAccessible`，因此还没有固定登录 Keychain、Data Protection Keychain、锁定态和签名身份变化下的恢复策略。发行前须在实际签名应用与测试账号中选择并验证可访问性策略；若现有引用不可读，应保留引用并提示恢复/重授权，不得静默创建新密钥或覆盖原配置。此处是待验证的恢复约束，不是平台验收。
