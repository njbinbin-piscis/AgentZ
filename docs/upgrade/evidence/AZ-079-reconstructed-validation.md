# AZ-079 重建候选验证

Codex，2026-10-05。cycle036首次核心命令退出101：Cargo path patch位于AgentZ工作区下，候选crate错误继承AgentZ工作区依赖，缺少async-trait。原产品锁文件finally恢复并检查hash。失败日志保留.cache/upgrade-undo-reconstructed-036.log。

修正生成脚本，对三个候选包显式声明package.workspace=".."，仅修改生成目录并记录manifest.integration。重新生成.cache/kernel-candidate-6347eb77-8842-49b9-bb43-0e81fd4789d6。固定提交与两补丁hash不变。

cycle037 / session26952：cargo test -p piscis-kernel --config <生成配置> --lib undo_tests 退出0，4/4通过，331项未运行；日志.cache/upgrade-undo-reconstructed-037.log。验证命令finally恢复产品Cargo.lock原字节并核对hash，通过。

cycle038 / session80500正在执行同一重建候选的approval_execution与task_journal_routing宿主测试，含--include-ignored；日志.cache/upgrade-undo-reconstructed-host-038.log。本记录时未完成，不能计通过。命令finally保护产品Cargo.lock，运行期间不得启动其他Cargo更新或构建。

AZ-079保持doing。产品固定依赖接入与原生验收未完成；不发布、不部署。GUI仍按用户指示暂跳过。

2026-10-06补记：session80500退出0，cycle038宿主18/18、journal路由2/2通过；finally锁文件原字节恢复及hash核对通过，产品锁文件仍指向远端v0.8.68。此结果替代上述运行中状态，不代表产品依赖已切换。
