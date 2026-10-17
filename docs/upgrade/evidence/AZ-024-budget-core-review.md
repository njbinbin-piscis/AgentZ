# AZ-024 第一核心包负面审查

2026-10-08。范围：只读审查 AZ-024-budget-design.md、现有 RunStore/Binding 契约及与实现代理的接口协商。不修改 budget.rs、宿主/内核、台账，不运行 Cargo/模型/GUI。本文先记录设计的必要拒绝条件；撰写时 budget.rs 尚未落盘，因此不代表源码或测试通过。

## 设计可消费边界

设计采用现有 runs.db 独立 resource_budget_schema v1、Immediate 事务、自任务默认 membership，累计键 canonicalProject + rootTask，Run/Attempt/environment 仅为来源。这个边界允许先做纯账本，不授予网络/工具权限。已与 parallel_plan 协商以下接口约束；主线程需在最终源码落盘后复核及实跑夹具。

## 必须失败关闭的负面场景

| 风险 | 必要行为 | 必须真实 SQLite fixture |
|---|---|---|
| unknown price/usage 被转零 | None 不转0；未知usage保留预留/并发槽；有金额 enforce 而无可信价/保守金额上界时不发 permit。observe 显示已知小计和未知数量 | 缺price的金额模式拒绝；Started取消/断流后reopen仍Unknown；明确0 usage与缺usage不同；新attempt不能绕过 |
| 重试/续聊重置预算 | policy/currentRun不构成总key；新run和attempt仍扣相同rootTask余额；不可通过换env重置 | 两run、多attempt、换env累计；相同task policy再次初始化不清账；不同task隔离 |
| fake Binding形状合法 | Binding.validate只验证字段形状，必须同写事务读取 Run、当前Attempt、task/env/project并检查运行态 | 跨task/run/env/project、缺Attempt、corrupt执行态、非Running reserve拒绝且无新记录 |
| 旧attempt费用丢失 | reserve只允许current Running，settle可接受原已捕获历史Attempt，但归属必须仍真实、operation来源完全一致 | retry后旧attempt结算计入总Task，旧attempt新reserve拒绝；错run历史Attempt拒绝 |
| 重复reserve触发第二网络请求 | 精确重放只能返回原Reservation/Replay，Started/Unknown不能获二次fresh执行许可；intent/policy来源不可变 | ID同内容幂等；不同origin/intent/version冲突；两连接startCAS只一个成功 |
| 重复settle重复扣费或悄悄改账 | usage ID、operation、measurement完整幂等比较，同ID不同内容拒绝；settled终态不能再记第二usage | exact重放一次累计；不同usageID对同operation拒绝；同usage不同tokens/price/duration拒绝；失败后余额不变 |
| 实际超预留被截断 | 如实记录真实usage，checked arithmetic，后续blocked；不能丢超额或截成剩余额度 | actual>reserved入账并blocked；sum溢出、负数、REAL/文字数字、坏currency/scale拒绝 |
| 预算配置版本改变绕过限制 | first policy不可变，重复完全一致可读回；支持更新之前不同revision/内容拒绝且不重置counter。现阶段不自造用户预算 | 同revision不同limit/mode/currency拒绝；新revision也不能隐式清账；persist/reopen结果同 |
| namespace自动repair | 仅显式init能创建全新namespace；已有部分表、空/重复/future metadata、缺约束拒绝。验证和初始化在同一Immediate锁内重读 | 空metadata、少表、缺unique/NOT NULL、未来version、unknown列类型/损坏record拒绝并保持bytes；双初始化竞态 |
| query建库/迁移 | 只读open flags不含CREATE；缺DB/缺namespace/旧Run库不自动升级；不能调用RunStore::open代替只读验证 | 缺runs.db query后仍缺；缺namespace query前后bytes/文件集相同；只读合法账可查询 |
| counters与账目不一致仍放行 | 派生/存储计数必须与全部reservation/usage对账；缺parent/orphan、未知status不能被filter忽略 | 删除usage/parent、篡改counter、unknownstate导致query/reserve拒绝；并发最后额度只一成功 |

## 现有上游接口限制

- RunStore::open_existing_readonly/readwrite（run.rs:1048起）不建库，验证Run user_version=1、事件namespace及查询shape；新budget入口不能降级为RunStore::open（1078起），后者会创建目录/库和事件namespace。
- 既有open检查不是新写事务中的权威校验。两个连接间Run可能retry或结束，reserve必须在本次Immediate事务重新读Run/Attempt，不得使用事务前load snapshot。
- RunStore::attempts（1522起）验证历史Attempt枚举和时间，但返回结构未包含task/env；预算仍需通过run_id的真实join绑定。历史负时间/未知状态必须拒绝，不能只查row存在。
- approval Binding含project/task/environment/run/attempt及版本；其形状validate不是许可。预算也不替代fresh环境lease、任务活动锁或OS审批，第一核心包不得宣称宿主模型请求已受控。
- 账本记录active_ms不等于实时硬停止；暂未支持成员enrollment、wall-clock deadline、wire计费真实性。公开API/证据应明确unsupported，不能用纯core测试代表这些验收。

## 收尾要求

源码落盘后独立核对原子reserve/start/settle/unknown路径、错误rollback、layout/schema检查及真实fixture；主线程统一Cargo。未知/超额/损坏必须产生可解释typed结果而非成功余额0。第一核心包通过也只代表候选账务能力，AZ-024整体仍未完成。

## 实际源码审查（第一观察式账本候选）

本轮读取 budget.rs SHA256 `a92af6d3d83507827c65428f38cab35d54c2b98262cbf00576fd6f818dffc3e0`；run.rs `74ebee829e5fc91589ae73b3889f890942ec429873ffbb33d2856ef6377ba5b7`。这些指纹仅描述本次审阅输入，修复后需重新复核。主线程报告其 7 budget SQLite fixtures 已通过，full core/Clippy在途；本审查代理未运行这些测试。

实际API是 `open_existing(project,readonly)`、`initialize`、`configure(task,immutable Policy)`、`record_usage(Usage)->Snapshot` 和 `query(task)`。它是**观察式不可变最终测量**，尚无 reserve/start/settle permit、并发槽、成员enrollment或执行预算拦截。因此上方设计中的reservation未知保留/硬限额是后续要求，不是当前已实现能力。

### 必修 P2：无 usage 时接受损坏 Run

`budget.rs:396 configure` 只用 `COUNT(*) FROM runs WHERE task_id=?1` 证明任务存在；`snapshot:256` 仅在遍历已有usage时调用 `validate_budget_origin`。`run.rs:1548 validate_budget_store` 验证版本/事件layout/Run查询shape，没有验证对应Run内容。

可由源码明确推出的复现：合法Run库初始化预算namespace，在尚无usage时 `UPDATE runs SET execution='corrupt' WHERE run_id='run'`；随后 configure(task,policy) 仍可提交，query(task)返回正常零余额。删除当前Attempt、把Run时间设负数同样不被该路径验证。已有政策但usage为空时 query亦有相同缺口。未实际执行该复现，需新增SQLitefixture并由主线程运行。

建议：同一个事务中增加权威 `validate_budget_task(tx,task)`，逐个校验task的Run及完整历史Attempt；configure和已配置snapshot都调用。不要以伪造dummy Binding替代Run来源，也不要求历史Attempt仍current/Running。失败不得写policy。

### 必修 P2：既有 Run namespace 只查shape，缺约束可产生歧义

`validate_budget_store` 允许重建成同列但缺 Run/Attempt PRIMARY KEY/NOT NULL 的表；`validate_budget_origin:1569` 用query_row按run_id取一行，不检查重复Run。当该损坏表存在两条相同run_id不同task/env的Run时，归属校验可能任取第一行并认可。历史重复非currentAttempt也未被显式去重（当前重复由found_current拒绝）。预算namespace自身有table_info PK/NOT NULL和operation unique检查，但不会补足Run namespace的保证。

建议受限Run layout守卫验证runs/attempts必要PK/NOT NULL与主键唯一索引；或明确拒绝任何重复Run/Attempt并完整校验引用/类型。新增重建同shape缺PK及重复历史Attempt fixture，query/init/configure/record_usage拒绝且不repair。不要改Run user_version或自动升级。

### 有限成立的路径

- namespace检查全部三表、恰一INTEGER version1、精确columns/NOT NULL/PK和非partial `(task_id,operation_id)` unique；空metadata、futureversion、partial namespace明确拒绝。
- JSON数值使用i64 Option和nonnegative/checked add；usage列storage typeof必须text；unknown input/output/duration/cost保留独立计数，已知费用按currency分桶。观察式limits不等于费用授权，USD已知小计不含EUR或未知费用是明确范围。
- Policy初始后完全不可变；同revision改limit、新revision均拒绝。Task key不含Run/Attempt/env，测试源覆盖跨retry/newRun/env与迟到原Attempt。
- record_usage Immediate事务内validate origin；同usage/operation重复内容一致才幂等，冲突拒绝，汇总溢出导致新记录rollback。不存在可被释放的reservation，不会声称未知usage已结算免费。
- origin核task/env、Run状态+delivery、currentAttempt状态一致、历史Attempt必须terminal且时间合法、origin真实属于Run。对有usage的合法历史来源保留归账，不要求current Running，符合最终测量限定。
- open flags无CREATE，query无schema写；initialize是显式namespace创建，需既有Run库。尚未接任何LLM/费用限制、成员或GUI。

结论：两项损坏Run/无usage漏洞修复并实跑新增夹具前，不能给核心候选无保留通过；其他路径只读审阅未见新的blocking问题。正式AZ024全任务资源执行预算仍未完成。

## P2 修复后限定复核

最终只读输入：budget.rs `097bed23752ceb1f0978bae03891a1613ad85e8128617b41226ff723d9c242c2`；run.rs `ea4b9f4e701e2d623c690d6356a43cb9d0c9b20d6a0daa9c9b91a3ae936c01d6`。

两项P2源码已覆盖：configure在写policy前同事务validate_budget_task；已配置snapshot在读usage前验证该task全部Run/history，即使0usage也不跳过。validate_budget_store验证Run/Attempt全部列类型、NOT NULL及PK位置，新open也调用。validate_budget_run核合法枚举、execution/delivery、currentAttempt一致、历史terminal、时间非负及origin存在。缺PK/重复历史Attempt不能成为预算权威；晚到旧Attempt仍可记录合法最终测量，未获得执行permit。

新增fixture源码覆盖零usage时非法execution/缺currentAttempt/负历史时间（已配置及未配置），policy不插入、bytes不变；同列缺AttemptPK（含重复旧Attempt）时open/init/configure/record/query拒绝。原跨retry/newRun/env及晚到原Attemptfixture保留。主线程统一运行52758 fullcore/Clippy，本审查没有运行测试；不能将源码复核写成实跑通过。

**限定结论：上述两项P2已在源码层解决，本观察式候选未见额外blocking缺陷。** table_info的TEXT PK已保证正常SQLite主键唯一；本包不扩大到恶意writable_schema离线绕约束、全部宿主输入或模型计费精度验收。第一核心包仍无请求预留/执行预算，正式Gate保持。

## Phase2 原子 reservation / Started / Unknown 审查约束

建议以独立可版本校验的reservation namespace增量实现，明确兼容现观察式measurement不可变账，不能靠CREATE IF NOT EXISTS静默修复旧库。最低状态机：Reserved → Started → Settled；Reserved → Released仅可信“未发送”；Started → Unknown（取消/断流/崩溃无usage）；Unknown → Settled/可信对账Released需显式审计。每步Immediate事务+expected状态CAS；同Task总key不变。

- reserve：同事务policy revision精确匹配、fresh权威current Running Attempt、origin合法；聚合已知usage+所有Reserved/Started/Unknown预留及slots。unknown已有项必须明确阻止新执行，不能从余额滤掉。未知价的金额限制拒绝，金额与currency快照immutable，checked integer。
- 单次Started：reserve返回new/replay分离，不把幂等重放当新执行。start CAS需要reservation完整origin/boot/intent，两个连接只一获得新Started结果；启动前提交失败不得发网络。预算permit不授OS/tool权。
- 崩溃：进程boot变化、retry或TTL不自动释放Reserved/Started。Reserved也存在“commit Started前没发送”与执行协议旁路风险，未经可信start状态及对账不能自动假定免费。query保留原来源、额度、slot及明确Unknown原因。
- settle：同事务写唯一usage和reservation终态，原usage累计与预留释放恰一次。原历史Attempt可结算，不能重新执行。实际超过预留如实入账并block，不truncate；缺usage不把known0代替。Phase1 record_usage不得成为绕开reservation后假称受控的宿主入口；区分observational来源与permit-linked来源。
- 并发：SQLite Immediate是跨连接总额度/slot原子点；任务活动锁不是成员总预算锁。Unknown继续占slot，两个连接争最后额度/slot只一个通过；CAS异常rollback不得留下usage或释放余额。
- 版本：不默认用户费用限制。配置immutable仍维持，未来更新单独CAS/审计；旧政策/价格snapshot和原账户总量不可reset。Task时间硬停止、成员、provider内部wire仍显式不支持直至adapter接线。

必要Phase2 fixture：边界相等/+1，两连接最后额度与同operation start竞争；Started后drop/reopen及retry/newboot不能释放；Unknown缺usage保持而真实0可settle；late旧Attempt结算；同ID冲突/重复settle不加账；超预留记录+blocked；schema空/缺unique/future/部分namespace拒绝bytes不变；中途SQL故障证明usage与release同rollback。corefixture仅证明原子账务，不代表真实费用硬上限或网络请求恰一次。
