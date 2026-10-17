# AZ-077 macOS 凭据适配设计检查

2026-10-06，本轮执行主机为 Windows，AZ-077 保持 doing。

## 已确认的约束

Apple 已将旧 `SecKeychainAddGenericPassword` / `SecKeychainFindGenericPassword` 标为 deprecated，推荐使用 SecItem 系列；实现选用 SecItem，不新接入旧 API。[Apple Keychain items](https://developer.apple.com/documentation/security/keychain-items)

当前产品锁文件已含 security-framework 3.7.0、security-framework-sys 2.17.0 及 Core Foundation 依赖。本地已读其源码：passwords 的 set/get 包装内部使用 SecItem，但默认 get 不指定禁用交互；PasswordOptions 的 push_query 为 crate 内部私有。不能仅调用默认 get 并声称后台交互策略已满足。

因此适配需明确查询认证UI策略、当前用户的本地非同步Keychain、权限不可用错误及对象释放。凭据不能进入shell参数或环境变量；不使用security命令行传递主密钥。

## 待实施的原子步骤

1. 为Keychain引用使用独立平台格式，拒绝将Windows DPAPI blob当作macOS可读数据。
2. 使用随机条目身份保存32字节原密钥，读回核验后才能发布磁盘引用；避免以配置目录字符串作为可覆盖的共享条目身份。
3. 发布失败保留旧密钥，清理本次新增条目；清理失败记录可定位的非秘密身份，不能吞掉错误。并发创建不覆盖已发布配置的Keychain条目。
4. 旧密文维持原密钥；Keychain撤销、锁定、错误长度和缺失引用均失败关闭，不生成替代密钥。
5. 提供仅macOS显式运行的临时条目测试，finally清理；覆盖读写、撤销、损坏及迁移。当前Windows只能验证公共编排与Windows回归，不能关闭该任务。

## 当前验证环境

未发现运行中的cargo/rustc构建；上一轮AZ-030/AZ-083结果已入账，无需重复启动。没有仓库AGENTS.md命中。当前安装Rust目标仅Windows及Linux，未发现macOS目标或真实macOS会话；尚未运行macOS构建或Keychain调用。未改变现有Windows凭据实现，也未写入用户凭据。

AZ-077 的真实平台验收是不可由Windows替身测试代替的剩余条件。本记录是设计检查，不是适配实现或平台验收通过证明。

## 公共事务编排进展

已在 secrets.rs 增加外部凭据引用的失败清理回调：原子发布失败或并发noclobber竞争失败时清理本次创建的外部条目；清理失败同时报告发布错误和清理错误。目录先准备，再创建外部凭据，避免目录创建失败遗留条目。现有Windows保护不创建外部条目，清理为no-op。

新增注入发布失败后清理引用、双错误可见两个测试，最终代码secrets专项复跑15 passed、0 failed；rustfmt --check及台账validate通过。产品内核严格Clippy（--lib --locked -- -D warnings）通过。当前无遗留构建。原生SecItem回调尚未接入，这些结果只验证公共编排。固定候选AZ-030补丁不包含本次AZ-077增量，不应冒称候选已覆盖它。
