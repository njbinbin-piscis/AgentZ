# AZ-029 双版本回滚夹具准备

2026-10-05，Codex。读取当前82任务台账及AZ-029原证据，遵循ADR-003不新开doing。开始无cargo/rustc/link，piscis-engine仍仅原三文件修改；未发现适用AGENTS.md，未覆盖这些修改。

夹具保存于docs/upgrade/fixtures/AZ-029-rollback（源码、Cargo.toml及独立Cargo.lock）。双依赖为固定远端v0.8.68与本地候选，使用临时目录与两个固定虚构凭据。验证顺序：旧版保存/读取→候选读取并加密保存→旧版直接降级读取→恢复原配置字节且保留同一密钥文件→旧版重读。程序将直接降级不能恢复视觉key作为待确认风险，不把这个行为记为兼容通过。无凭据输出，不读取用户配置，不修改产品依赖。

已启动cargo run --manifest-path .cache/rollback-review/Cargo.toml --target-dir target，exec session23536，日志.cache/upgrade-secret-rollback-review.log。本记录时仍在构建/依赖获取，未获得运行结果，未记回滚通过。独立lock保留解析结果；AgentZ工作区原Cargo.toml/lock未由夹具重写。

后续先检查此进程与日志，避免重复构建；即使夹具通过，也仅证明两个字段与原始配置/密钥保留路径，不替代固定可分发依赖接入、全部字段、真实升级或跨平台验收。AZ-029仍review，未发布、部署或对外发送。
