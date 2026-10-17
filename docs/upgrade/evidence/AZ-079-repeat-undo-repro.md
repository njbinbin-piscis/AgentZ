# F13 / AZ-079 同轮重复编辑Undo缺陷

2026-10-05，Codex。AZ-028固定内核hook集成验证中发现；不是新授权hook改变了journal算法。固定内核v0.8.68（527e55496caf4437e4265e02869f0a8c764549f6）file_journal.rs的undo_turn按id DESC查询，但以restored_paths去重，只恢复每条路径的最新before_content，跳过更早快照。注释声称最终保留最老前态，实际行为相反。

真实复现：临时source.txt内容original；同一journal turn中edit-1、edit-2，两次都经固定kernel journal before/after hook；undo_turn后实际为edit-1，预期original。首轮host集成5项中此项失败，日志 `.cache/upgrade-approval-hook-integration-cycle-011.log`，编译7m01s，失败断言保留。

当前状态：未修复，AZ-079 pending，独立于已完成AZ-012环境路由。不得将AZ-028的单次编辑/每次Undo验收当作此问题已经通过。M1的原生旅程和此跟进尚未完成，不提升Gate。

计划：按每路径最早有效快照恢复整个turn，验证重复编辑、先新建后编辑、失败/undone快照、真实Task A/B隔离与单步Undo；以临时数据验证，保留journal与原文件。固定依赖接入另需证据，禁止自行发布或宣称数据回滚通过。
