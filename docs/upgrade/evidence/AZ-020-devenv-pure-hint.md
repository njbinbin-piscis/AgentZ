# AZ-020 Devenv prompt-only 入口

独占 `commands/devenv.rs` 新增 `pub async fn context_hint_for_turn(project: &Path) -> Option<String>`，仅执行 scan(project,false)、load_dismissed、hint_section，不读写 injected_sessions，不接受计划cache或sink，不添加todo、不emit PlanUpdate。scan的探测缓存仍按既有TTL使用；它不是会话计划缓存。原 context_for_turn 等其他调用端兼容保留，由主线程另包把主turn改用本API。

新增一个行为夹具，使用唯一临时项目与注入探测报告避免依赖本机安装工具：pure hint返回缺失工具提示；真实PlanStore持久计划ID/revision/条目不变；既有内存投影不变；dismiss持久化后提示消失且计划仍不变。临时目录删除前确认位于temp，关闭SQLite。夹具未执行，不能声称GUI/宿主验收通过。

实际执行 rustfmt与限定diff检查exit0，未Cargo/模型/build。本包不将提示当权限或自动计划授权。

文案复核修正：公共hint_section维持legacy文案，内部结构化生成器通过mode为pure context生成准确提示，不做字符串替换。pure文案说明没有自动计划、resolve仅复查环境、dismiss仅忽略线索，计划需显式plan_todo；工具执行仍遵循宿主权限规则，不保证每条命令都会用户确认。pure fixture增加提示内容断言，同时继续检查真实持久计划不变。主线程在途72998使用旧源码的结果不能覆盖这次修改，需统一增量重跑。

主线程建议限定运行 `commands::devenv::tests::pure_hint_respects_dismissal_without_changing_durable_plan_or_projection` 并与其它devenv测试统一复核。接线后主turn上下文须只传workspace/project，不传session/cache/sink，以消除SQLite持久化前的内存todo旁路。
