# AZ-030 Windows 主密钥保护与迁移

2026-10-06。测试范围为当前 Windows 用户、本机临时目录及虚构凭据。

## 实现

`vendor/piscis-engine/piscis-kernel/src/security/secrets.rs` 使用当前用户 DPAPI 封装32字节主密钥，磁盘格式为 `AGENTZ-KEY-V1:DPAPI\0` 加保护数据，不设置机器级保护标志。旧32字节密钥保持值不变，封装后使用同步临时文件原子替换；失败保留原文件。新密钥以 noclobber 发布，系统保护失败不发布明文密钥。

`store/settings.rs` 保存 `agentz_secret_format:1`；新格式加载时缺失密钥、未知版本、保护失败或凭据解密失败均返回错误。加载不再吞掉保护错误，也不以默认配置替换损坏 JSON 或复制原始配置为未经保护的备份。宿主 data_scope 加载入口传播这些错误。

旧未标记配置可能混合明文与密文，仍保留旧字段识别兼容策略；这无法证明所有旧未标记损坏密文都能可靠识别。系统主密钥保护错误仍会传播。非 Windows 明确返回保护不可用；macOS/Linux 由 AZ-077/AZ-078 独立实现验收。

## 已执行验证

- 产品内核 `cargo test -p piscis-kernel --lib secrets_ --locked -- --nocapture`：13 passed。覆盖真实 DPAPI 旧密钥迁移、原密文解密与重开、损坏保护数据、文件锁阻止迁移、注入系统保护失败、缺失密钥不生成替代、新格式严格解密及既有原子保存失败夹具。
- 固定候选同一 secrets_ 测试：13 passed；pool_services 过滤为0项，不计为已验收。
- 宿主 `cargo test --manifest-path src-tauri/Cargo.toml --test approval_execution --locked`：18 passed、1 ignored。
- 完整产品内核 `cargo test -p piscis-kernel --locked -- --test-threads=1 --quiet`：358 passed、1 failed、1 ignored；失败为 AppContainer 后代退出 marker 断言。直接运行相同测试二进制的该单测重跑为1 passed。完整回归未通过，已新增 AZ-083 独立定位夹具与真实回收行为，不能用单测通过覆盖首次失败。
- 固定候选 Clippy `--lib -- -D warnings` 加已记录的四项基线例外 `manual_find/collapsible_match/chunks_exact_to_as_chunks/redundant_closure`：通过。
- 两个修改的 Rust 源文件 rustfmt --check：通过。

- 产品内核 Clippy `--lib -- -D warnings`：通过，无例外。

- 宿主 Clippy `--lib -- -D warnings` 加既有三项例外 `collapsible_if/cmp_owned/redundant_closure`：通过。
- 台账 validate 与隔离夹具 verify：通过。

Windows 参考实现按专项及宿主集成证据完成自检；完整回归失败仍按 AZ-083 跟踪，不作为通过项。

后续 AZ-083 修复非交互延时夹具后完整复跑：359 passed、0 failed、1 ignored，pool25 passed，doc test1 ignored。见 [AZ-083 记录](AZ-083-descendant-handshake.md)；首次失败未删除。

## 固定来源

基线 `527e55496caf4437e4265e02869f0a8c764549f6`，候选按 AZ-029、AZ-079、AZ-081、AZ-082、AZ-030 顺序重建。AZ-030 patch SHA-256：`abeef043e7dd62a147772afb7b4c24268338b54b412dcb890f0de4807f6f8e54`；候选源码 archive SHA-256：`a3cd6275bf53ce2046251db50977a963d5364839d19c87793de463b834a90f84`。脚本完成正向检查、应用和反向检查。候选未包含 AZ-034，因此不是整个 vendor 的字节等价验收。

## 验收限制与回退

没有运行多用户、重启、实际用户凭据、原生 GUI、macOS/Linux、发行安装或旧版回滚演练。DPAPI 当前用户绑定不防御已控制同一用户进程的攻击者。

恢复夹具证明同用户恢复匹配的保护密钥及配置后可重新解密，不能冒称完整发行备份恢复验收。旧二进制不能读取新密钥格式；按 [ADR-004](ADR-004-protected-downgrade.md)，AZ-063/AZ-065 在发行前实现并验证受保护配对快照及跨格式降级拒绝。
