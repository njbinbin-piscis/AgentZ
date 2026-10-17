# AZ-079 Undo All 修复实施

2026-10-05，Codex。用户明确要求跳过GUI继续下一步；原生验收保留review，不计通过。按依赖选择M1的AZ-079为唯一主实现，用户指示覆盖ADR-003等待review数量下降的排期限制。台账82任务、14验收、68未完成，无适用AGENTS.md。

保留piscis-engine原AZ-029三文件修改，另修改agent/file_journal.rs：undo_turn查询按id升序，再逐路径去重，取最早applied且未undone快照；单步Undo入口未改。新增真实临时文件/SQLite回归：重复编辑恢复二进制原字节且不重放、新建后编辑恢复不存在、失败快照与其他session隔离。

cargo test -p piscis-kernel undo_tests已启动，exec session39822，日志.cache/upgrade-undo-cycle-032.log。本记录时未取得完成结果，未记通过。后续先检查该进程，修复失败或补单步/已撤销回归，然后生成固定基线补丁并验证候选。AgentZ仍使用原固定远端依赖，不能宣称已接入修复；未发布或部署。
