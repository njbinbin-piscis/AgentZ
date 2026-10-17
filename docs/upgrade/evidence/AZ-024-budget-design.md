# AZ-024 整任务资源预算：只读最小设计

2026-10-08。读取正式台账 AZ-024、状态库 Run/Attempt、approval Binding 与环境登记实现；本包仅设计文档，不修改产品、台账或运行测试。正式范围为全部 attempt/成员累计 token、费用、时长和并发；验收要求耗尽可解释停止、续跑不重置、未知价格明示。以下是候选契约，尚未实现或接线。

## 权威身份与存储边界

当前 `RunStore` 的 `runs` 保存 task/environment/current attempt/execution/delivery/updated_ms，`attempts` 保存所属 run 与状态及时间。`retry` 新建 attempt 而不更换 task；多次 chat turn 可以有新 run。`approval::Binding` 形状验证不是执行授权；预算预留必须另外读取权威 Run/Attempt，并由宿主保持 fresh environment lease、任务活动锁及 captured binding。不能从最后 Done、session累计数或本地UI推导预算余额。

**累计键采用 canonical project + stable root task ID**。不把 run、attempt、environment、模型、成员或会话轮次放进总预算主键，因此 retry、正常续跑、更换环境不会隐式重置。它们作为账目来源维度保留。显式新建真正任务可获得独立预算；同任务 Replace 目标不是自动重置预算。预算上调/禁用/显式重置若将来支持，应是可信宿主发起的有版本变更并保留旧账，不能由模型工具操作授权。

建议第一核心包在现有 `.agentz/runs.db` 内添加独立 `resource_budget_schema` namespace，版本从1开始，不改 Run `user_version=1`。这样 reservation/usage/counter与同事务中的 Run/Attempt 权威查询能够原子化，不另建独立预算DB后假称跨库原子。只有显式预算初始化命令可以在验证正常既有Run库后建namespace；普通query只读、不create/repair，未知namespace版本/部分表/坏记录failclosed。初始化须Immediate锁后重读version，避免双首次open竞态。

实现归属建议：新 `crates/agentz-task-state/src/budget.rs` 为主，主线程独占 `run.rs`/`lib.rs` 最小导出或受限连接接入。不能仅先打开RunStore验证后另开连接而省略同写事务的schema/权威校验。应将现有layout校验作为内部共享帮助函数，或由RunStore提供预算事务入口；不公开任意SQL连接给renderer。

## 最小持久契约

| 记录 | 必需内容和约束 |
| --- | --- |
| TaskBudget | root_task_id、policy_revision、mode（observe/enforce显式）、可选input/output/total token上限、可选金额上限/单一currency、累计active_ms上限、可选task deadline、max_in_flight；既有账不因policy更新消失 |
| Membership | root task、member stable ID、实际run task ID、可信宿主登记来源与版本；默认只有self，成员不可自报根task；须同项目且实际Run归属校验 |
| Reservation | stable operation_id、root/member、captured task/run/attempt/environment、kind(main/summary/vision/其他实际计量入口)、boot_id、policy_revision、不可变intent digest、input/output估算预留、可选price快照、状态及时间 |
| Usage | stable usage_id、operation_id、实际provider/model、reported input/output及计量来源、active_ms、可选金额快照、记录时间；重复同ID同内容幂等，内容冲突报错 |
| Summary | settled已知值、outstanding预留、unknown计量/价格项数、overshoot/blocked原因、policy_revision；unknown单独表示，不能把None转换成0 |

所有数值以非负整数及checked arithmetic处理；SQLite必须校验storage type，拒浮点token、负数、溢出、空/跨域身份、future version和有状态但缺counter/对应reservation的损坏。金额采用带currency与明确scale的定点整数（例如micro-units），不用浮点，混合currency不能相加。price快照包含provider/model/价格来源版本/生效范围与input/output费率及舍入规则，不查询不稳定在线价格或默认价格。

**token预留是估算，不是provider真实消费保证。** 需要上限策略时，宿主应传合理input估计、请求max_output_tokens以及额外已知内部wire风险；实际usage超预留仍必须入账，设置overshoot并阻止后续工作，不能丢usage或截成余额。核心只能保证预留及已知消费的原子账务，不能保证tokenizer估算、缓存/推理计费或provider内部重发精确。未返回usage的失败不记“0实际费用”。

未知price：observe模式显示未知项/已知小计；显式金额enforce不能在缺价格且没有可信保守金额预留的情况下放行。已知价token也不等于真实账单，必须说明estimate/reported/billed来源。默认不启用收费限制、不擅自设用户金额或模型预算；核心第一阶段不触发模型调用。

## 原子预留与结算

`reserve(binding, root/member, operation_id, intent, expected_policy_revision)`：Immediate事务内验证schema、membership、Run=current Attempt且允许发起请求的运行状态、captured task/env匹配；严格匹配已有同operation的全部不可变字段，重复仅返回原reservation，不重复扣减或开启第二次执行。新项检查 `settled + outstanding + requested <= limit` 和in-flight上限，checked arithmetic成功后写reservation、预留余额及槽位，并同一事务提交。返回的permit只是该具体逻辑操作预算许可，不授予工具/OS能力；caller在commit成功前不得调用client。

**幂等reserve重放不能意味着允许重发网络请求。** 调用方需消费该permit的单次执行状态，真实请求开始前以CAS记录Started；重复Started/Unknown不可再次执行。客户端内部reasoning wire重发必须被预算adapter看见或按保守预留明确列为未覆盖，不能把一个logical operation称一条wire。

`settle(operation_id, usage_id, captured_origin, measurement)`：一个Immediate事务内检查reservation归属、同usage精确去重及全部counter；写usage、累计tokens/已知金额/active_ms、释放该项预留和并发槽、终结reservation。实际量大于预留仍如实结算并blocked后续预留。提交失败时caller显示计量未持久化且暂停后续费用操作，不能通知“预算结算成功”。费用账不改变Run delivery或授予权限。

reserve要求current running Attempt；**settle允许原已捕获Attempt现在已terminal或历史**，但必须仍属于原Run/task/env且operation完全匹配，防止迟到真实usage被retry抛弃。它不能给旧Attempt新permit或自动恢复执行。Run结束与budget结算若在独立宿主调用中发生，不宣称两者同时commit；生命周期终态也不自动释放未知reservation。

失败分类需区分：可信明确“请求未发送”可CAS释放Reserved并记录原因；Started后断连/取消/进程崩溃，没有可信usage时进入Unknown保留预留与槽位。崩溃恢复不依TTL自动释放，也不能靠新boot/newAttempt/reset清账。未结算项使启用预算的任务failclosed，显示root/task/run/attempt、unknown资源、原时间及恢复入口；人工/可信对账可显式将未执行释放或补已知usage，需审计版本和幂等事件，不自动重跑工具或模型。

## 时间与并发定义

active_ms为各操作实际活跃耗时的累计，跨attempt/成员求和；两个并发成员各运行1秒累计2秒。它不同于task墙钟时长。运行中使用单调时钟计算duration，持久保存非负累计；UTC创建/更新时间用于审计，不能由系统时钟回拨得到负耗时。task deadline如启用则锚定原任务预算起点，续跑不重置，回拨/未知anchor明确停下，不凭Run.updated_ms猜首次开始。

若要硬时长停止，预留本身不足：执行adapter必须在剩余deadline/active预算范围内竞争取消，结束后结算实际值；并发活跃耗时共享上限需要预留duration切片或更严格控制，不能仅事后累计后宣称实时上限有效。核心第一阶段保存duration来源和未知项，只对已知加预留数值做严格原子检查。

并发槽随reservation持久化，未知Started槽崩溃后保留。task_activity现保护同task chat排队/执行，不等同未来成员共享总预算锁；SQLite Immediate事务是并发总预留原子点。成员目录/映射未实现之前拒绝非self membership，不把当前team/pool入口自动纳入支持。

## 可分阶段实施与完成条件

1. **AZ024-A 纯存储核心**：namespace、TaskBudget/self membership、reserve/start/settle/unknown/query，Run权威事务校验及typed拒绝原因。明确observe/enforce无宿主默认启用。完成条件为真实SQLite重开、并发、边界及损坏夹具通过；不称模型/成员全覆盖。
2. **AZ024-B 宿主预算adapter**：可信配置与captured task身份，单逻辑操作permit生命周期、失败保留、可解释拒绝和只读汇总。客户端main/retry/auxiliary所有实际入口由入口审计列举；未接入口保持unsupported，不能先提供全覆盖UI。
3. **AZ024-C 内核与wire计量**：main/fallback/纠偏/overflow摘要/vision及可见内部wire的reserve/usage回调；未知usage/price显示及cancel竞态。先loopback HTTP矩阵，不花真实provider费用。实际provider账单一致性另验。
4. **AZ024-D 成员与恢复**：可信membership、共享总预算、unknown对账与policy变更审计、GUI重启/解释性停止/预算查询。只有成员实际接线后才验收“全部成员累计”。真实模型、掉电恢复、费用/时间真实性需资源，不能以core夹具替代。

## 第一核心包真实fixture清单

- 三个Attempt/两个Run累计到同task；retry、更换environment、正常续聊不reset；不同task不混账。
- reserve边界精确相等允许、+1拒绝；两个连接同时争最后token/并发槽只一个成功，不超额。
- reserve相同ID精确幂等且Started不授二次permit，intent/身份/policy版本冲突拒绝；settle同usageID同内容仅累计一次，不同内容拒绝。
- 旧Attempt结算合法且不能预留新请求；跨task/env/run/operation和未登记member拒绝。
- actual>reservation如实结算且blocked；未知usage/价格保持Unknown，不当0；混currency/坏scale拒绝。
- commit后drop/reopen账和槽仍在，started未结算崩溃failclosed，newAttempt不能绕过；显式可信未发送释放与usage对账幂等。
- 已知duration多成员加和、negative/overflow/clock回拨拒绝；时间硬停止待adapterfixture，core不冒称完成。
- missing只读不建库、v0/未来/部分namespace/损坏counter或record拒绝且bytes不变；双初始化重读version安全。

该方案优先完成可验证账务契约；AZ024继续preparing，未运行任何新测试、GUI、付费模型或默认费用限制。
