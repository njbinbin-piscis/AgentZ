# AZ-078 Linux Secret Service 适配

2026-10-06；代码准备评审，真实 Linux 验收尚未运行。

## 实现

`security/linux_keyring.rs` 使用当前用户D-Bus Secret Service的DH加密会话；仅访问现有默认collection，不回退到session collection、明文会话或磁盘明文密钥。锁定collection/item、缺失或重复条目、错误密钥长度均失败关闭。

主密钥保存在系统凭据条目中，磁盘只保存 `AGENTZ-KEY-V1:SECRET-SERVICE\0` 和随机UUID引用。创建不替换同属性条目，读回核验密钥后才发布引用。发布失败使用已有公共清理编排；创建回包丢失时也按本次身份尝试清理，失败报告可定位的非秘密身份。旧32字节密钥迁移保持原密钥值。

已核查上游库：其创建/删除遇到Prompt路径会执行提示。AgentZ固定vendor的secret-service 5.1.0，并显式启用 `agentz-no-prompts`，异步及阻塞提示入口在构建proxy或调用Prompt前返回错误；不调用unlock/create_collection。库中的DH及AES IV随机源错误改为返回Crypto错误，避免崩溃。该改动不消除D-Bus连接/方法的等待，也不证明服务端不会在其内部显示UI。[Secret Service collection规范](https://specifications.freedesktop.org/secret-service/latest-single/#org.freedesktop.Secret.Collection)

## 来源与补丁

原crate archive SHA-256：`9a62d7f86047af0077255a29494136b9aaaf697c76ff70b8e49cded4e2623c14`，源commit `7e8f960c767d34d961d82913a2eb0ce3bce09701`，保留MIT/Apache许可证。

补丁 `docs/upgrade/patches/AZ-078-secret-service-5.1.0.patch` SHA-256：`c179551741b95825ab778338dc8e1ae135caaa435b8e64fe6bdf9ba27bf15f5e`，在vendor目录的反向检查通过。Linux适配源码SHA-256：`b81db6dde01397d72969d5e15c24c2e53cce86c9026c7f8e61c4d6d182b7791b`。原固定内核candidate脚本未包含本次平台增量，不声称其已验证这些改动。

## 实际验证

- Windows `cargo test -p piscis-kernel --lib secrets_ --locked -- --nocapture`：16 passed；包括迁移/原子发布失败清理与跨平台引用拒绝。
- Windows产品内核严格Clippy：通过。Linux分支不在该构建范围。
- 隔离Linux目标适配及测试类型检查：`cargo +1.98.1 check --manifest-path .cache/az078-adapter-check-f4ada78c-916a-42fb-a44e-e3c6229203ea/Cargo.toml --target x86_64-unknown-linux-gnu --tests --locked --offline` exit0。最终创建失败清理调整后再次通过。包含实际linux_keyring/secrets源码及修改后的Secret Service依赖；不是整个内核/桌面应用的Linux构建，不链接或运行Linux程序。
- 初次隔离检查因离线缺少host编译依赖uds_windows失败；补取锁定依赖后重跑通过，初始result.json保留失败，check-retry.log记录成功。
- 修改Rust源码rustfmt --check、台账validate通过；新增脚本node --check通过。

## 平台验收入口与限制

两项Linux运行测试默认ignored：`linux_secret_service_roundtrip_revocation_and_migration` 显式使用临时条目并清理；`linux_secret_service_unavailable_preserves_legacy_key` 要求以新测试进程配置 `DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/agentz-test-bus`，验证不可用时旧密钥保持、新密钥不生成。它们只被类型检查，没有运行。

还需真实Linux用户会话验证锁定、损坏、Prompt拒绝、撤销和迁移，并核验无残留条目。未发现可用WSL发行版，未安装Linux系统或改动主机虚拟化功能。旧常规secret测试尚未全部加入系统条目清理，不将整个测试集直接运行在用户Keyring。

未做多个并发旧密钥迁移、恶意服务、压力竞态、真实Agent/GUI、发行和备份恢复验收；尤其跨机器恢复不能仅恢复磁盘UUID引用。AZ-063/AZ-065仍需落实对应系统凭据的受保护备份恢复契约。本任务保留review，不记done。

## 2026-10-16 本地清理幂等夹具

临时条目运行夹具现在连续两次调用 `remove`，要求首次撤销与重试均成功；该分支源码原已通过“零或一个条目”的检查处理缺失引用。本轮适配源码 SHA-256 为 `f9c60e4c252f8f2efc136711edc86317c25b114d912e43d98ff9cd8071e3445c`。隔离 `x86_64-unknown-linux-gnu` 源码及测试类型检查 exit 0，`rustfmt --check` 通过；夹具仍默认 ignored，未在真实 Linux Secret Service 会话运行。D-Bus 服务等待上界和创建结果丢失后的受控重试仍未实现。
