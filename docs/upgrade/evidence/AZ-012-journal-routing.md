# AZ-012 Journal环境路由

查询变更、读取diff和Undo三个命令统一通过session任务登记解析环境根，而不是直接使用主项目目录。chat_send写入journal与评审/Undo读取相同登记，已登记环境失效时返回错误；未登记的普通主项目CodeZ路径保持兼容。

编译：`cargo check -p agentz-desktop --locked`通过（43.26s）；共享环境规则7项测试及clippy -D warnings通过。新增任务键/JSON身份一致性检查，损坏记录不能把查询映射到另一任务。

未验证：真实FileJournal在隔离任务的编辑→diff→Undo完整回滚，普通与隔离两个环境的内容核对。尚未建立正式local/free环境的登记与历史关联（M6范围）。本项保持review，不将编译和环境规则测试算作journal回滚验收。

下一步：在私有测试仓库运行实际编辑回合，检查主目录与worktree journal，Undo只还原目标环境，并保留恢复前后文件证据。涉及checkpoint的恢复路径仍须后续专项核对。

2026-10-05补充验收：`cargo test -p agentz-desktop --test task_journal_routing --locked -- --nocapture`最终通过（1项集成测试，9.01s；本次增量宿主构建5m26s）。测试直接调用IPC共用open_task_journal及登记解析，不使用替代journal实现。实际固定内核FileJournal hooks捕获主目录/A/B编辑与新建文件；重新打开SQLite读到before/after；Undo还原A并删除A新文件，主目录和B不变；local回滚不碰B；重复Undo无操作；分支失效返回环境错误，无主目录回退。源测试保留断言与复现入口，临时目录在测试结束清理。

本项可按约定集成验收关闭（Codex自检，不是独立QA）；上文未验证的实际FileJournal项已补齐。原生UI、真实模型和checkpoint专项仍未验收，未扩展本项完成声明。第一次宿主测试使用构建期间修正前的ChangeSet领域库，因运行时数据库被列举失败；重跑最新版本通过，详见AZ-011证据。
