# AZ-079 宿主候选验证结果

2026-10-05，Codex。主实现仍为 AZ-079；GUI 按用户指示暂跳过，不计验收通过。

cycle034 / session8545 退出101：target/debug/agentz-desktop.exe 被运行中的候选实例占用，拒绝删除，测试未执行。Cargo.lock 的 finally 恢复及 hash 核对完成，远端固定依赖仍为 v0.8.68。

仅停止本轮候选 target/debug/agentz-desktop.exe（PID15760），重跑同一命令：cargo test -p agentz-desktop --config .cache/upgrade-kernel-local.toml --test approval_execution --test task_journal_routing -- --include-ignored。cycle035 / session67219 退出0，审批宿主18/18、任务 journal 路由2/2通过；新候选用例通过真实 ApprovalHooks/FileJournal 连续两次写入后撤销到 original，并验证重复撤销不重放。日志 .cache/upgrade-undo-host-cycle-035.log。命令 finally 恢复 Cargo.lock 原字节并检查 hash，通过。

本次使用邻仓本地内核候选，包含 AZ-029 和 AZ-079 修改。核心 Undo 4/4另见 AZ-079-undo-core.md。未完成可分发固定依赖接入、原生产品点击、真实模型和跨平台验收；不能据此认定远端 v0.8.68 的生产 Undo 已修复。AZ-079 保持 doing，下一步复核可移植固定依赖接入方案及验证入口。未发布、部署或对外发送。
