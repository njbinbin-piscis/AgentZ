# AZ-079 产品依赖接入结果

2026-10-06，Codex。将固定提交527e55496caf4437e4265e02869f0a8c764549f6及AZ-029/AZ-079审核补丁生成的内核源码纳入vendor/piscis-engine（约2.2 MB、124文件，不含.git元数据），记录来源于vendor/piscis-engine/AGENTZ-BASELINE.md。Cargo.toml的三个路径patch让产品实际解析到这些内核包，Cargo.lock同步记录解析图。此前未提交的Cargo.lock任务状态crate新增条目继续保留。

产品配置实际验证：cycle040 `cargo test -p agentz-desktop --test approval_execution --test task_journal_routing -- --include-ignored`退出0，approval_execution 18/18，task_journal_routing 2/2；cycle041 `cargo test -p piscis-kernel --lib undo_tests`退出0，4/4。cycle042同宿主命令增加`--locked`退出0，再次18/18及2/2，证明Cargo.lock无需改写即可复跑。日志分别为.cache/upgrade-undo-product-integration-040.log、.cache/upgrade-undo-product-core-041.log、.cache/upgrade-undo-locked-042.log。

根Cargo.lock仍保留AgentZ既有13行agentz-task-state新增记录；本次新增本机源码patch解析所需条目与tempfile测试依赖并移除三个引擎包的git source标记。未改写该既有条目。台账validate与git diff --check通过。

同轮重复编辑撤销到轮前字节、新文件恢复为不存在、失败或已撤销快照排除、重复撤销不重放、任务/环境路由均由实际产品依赖验证。依赖固定在上述源码提交及审核补丁。AZ-079条件已满足，转done；M1里程碑尚未通过，GUI验收按用户指示跳过且不计通过。真实模型、跨平台、发布/回滚验收未在本任务覆盖。
