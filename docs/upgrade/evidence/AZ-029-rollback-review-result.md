# AZ-029 双版本回滚结果

2026-10-05，Codex。读取当前台账和启动证据；未发现适用AGENTS.md，无运行中cargo/rustc/link，复用上轮夹具，无重复构建。

双版本夹具构建4m17s并运行退出0，日志`.cache/upgrade-secret-rollback-review.log`，exec session23536已结束。实际依赖：固定远端v0.8.68/527e554与本地候选piscis-kernel；独立lock保留。未改变AgentZ固定依赖。

临时目录、虚构凭据实测结果：旧版保存后，旧版与候选均能读取；候选保存后两项凭据均不以原字符串出现于配置；旧版仍能读取API字段，但视觉字段不等于原凭据。这确认直接降级不兼容，而不是回滚验收通过。把同目录升级前配置原始字节恢复且保留原密钥文件后，旧版API与视觉字段均恢复，配置字节一致。

夹具源码及Cargo.lock见docs/upgrade/fixtures/AZ-029-rollback。可重跑命令：cargo run --locked --manifest-path docs/upgrade/fixtures/AZ-029-rollback/Cargo.toml --target-dir target。local候选路径依赖要求保留当前piscis-engine修复源码；这不是可分发产品依赖。恢复夹具使用旧格式配置，其视觉字段原本是明文，不能把该备份当作已获得系统凭据保护；实际备份保护与迁移仍待后续任务。

边界：仅验证两个字段及同一密钥文件保留，不覆盖全部嵌套秘密、密钥丢失、真实用户升级、断电、跨平台或原生产品。AZ-029保持review；固定可分发依赖未接入，禁止把本结果当作发布许可。未发布、部署或对外发送。
