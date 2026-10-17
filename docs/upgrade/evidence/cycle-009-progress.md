# 持久审批核心实施周期

2026-10-05，Asia/Shanghai，heartbeat agentz。仅Codex，单一主任务AZ-028。开始读取计划/进度/最新证据，核对无适用AGENTS和在途构建，保留AgentZ及piscis-engine已有修复。没有发布、部署或外部消息。

交付：独立SQLite审批核心，绑定任务/环境/run/attempt/boot/版本，规范化参数摘要、期限、决策、原子一次消费、执行结果与同事务审计；重启失效和claimed unknown。旧UI审批投递失败不再显示成功。

证据：AZ-028-durable-core.md。10项审批/传输定向测试、全30项领域回归、严格clippy及固定依赖宿主check通过；台账工具隔离回归和diff检查通过。所有本次构建已退出。未验证原生界面/真实模型/跨平台/断电或完整回滚。

复盘：只持久bool不足以授权；批准与消费、投递与执行必须分开。审计故障应阻止claim。并行同名工具的ToolEnd不能按名称猜测归属；当前kernel缺少审批tool-use关联与执行前fresh授权接口，不能靠旁路事件收集冒充实现。

状态：AZ-028保持doing；done仍14/78。AZ-029仍review且远端固定依赖未采用本地秘密修复；AZ-009/013原生证据与MB模型基线缺口保持，不提升Gate。

下一步仍AZ-028：落地可信issuer/受保护human决策/实际执行前claim及关联finish，检查无confirm map和关闭confirmation flag时的默认行为；保持P0/P1同范围委派复用。必要内核改动本地验证并保存补丁，不自行发布。
