# AZ-017 Run / Attempt 持久状态候选

2026-10-06；preparing，M1 Gate仍未通过，候选不计已验收任务。

`crates/agentz-task-state/src/run.rs`提供独立`.agentz/runs.db`：Run绑定task/environment并指向当前Attempt，Attempt保存各次执行状态和时间。创建先验证已登记的任务环境，不改变旧环境数据库。新版本schema被拒绝；初始化在SQLite立即事务中重新检查版本，避免同时打开时重复建表。

执行状态为queued、running、cancelling、succeeded、failed、cancelled、interrupted；交付状态独立为not_ready、awaiting_review、accepted、rejected。成功只进入awaiting_review，此模块不发布产物。交付决定目前只是存储原语，还需宿主可信入口授权，不能把方法名称当作权限校验。

状态转移在事务内同时更新Run和Attempt，以run/当前attempt/预期状态/时间做条件更新；旧回调不能结束新Attempt。失败/取消/中断可建立新Attempt，历史失败保留；成功不能直接retry。新Attempt身份冲突导致整笔事务回滚；同一task不能同时存在两个非终态Run。

## 实际验证

- `cargo check -p agentz-task-state --locked`：通过。
- 先执行5项Run专项：5 passed；对应全领域测试：43 passed、0 failed，含既有审批、环境、归档和合并夹具。
- 补充并发初始化/未来schema拒绝后最终Run专项：6 passed、0 failed；全领域套件未在补测试后再重复运行，不宣称最终44项全套已执行。
- `cargo clippy -p agentz-task-state --all-targets --locked -- -D warnings`：通过，包含最后新增测试。
- 台账隔离verify通过，覆盖preparing合法性与不能绕过Gate进入review；validate通过。

## 宿主接入待完成

当前真实chat在排队后才建立内存cancel槽；审批hook自行生成run UUID并让run_id=attempt_id。run_agentz_turn只有结束日志及临时计划。新存储模块尚未接入这些入口，不能声称真实任务已有重启恢复或取消持久化。

接入需统一宿主签发的run/attempt身份并传给审批Binding；覆盖排队、实际开始、结束及异常。直接CodeZ会话允许没有TaskEnvironment，不能为接入新表而擅自注册一个冻结工作区记录、改变现有分支切换语义。需明确其宿主验证身份契约后再接入。恢复对账、事件游标、持久计划、工具恢复和预算仍属于AZ-018至AZ-025。

因此本记录只证明候选存储实现和数据库测试，M1 GUI、真实模型、平台运行及发行验收保持原状态。

## 宿主身份接入候选（2026-10-06 后续）

ApprovalHooks新增new_with_identity入口，由宿主传入独立Run/Attempt身份；旧new入口保持兼容。fresh环境重校验保留当前Attempt，防止重建Binding时退回run_id=attempt_id。共享Binding形状校验开放为公开方法，此方法不授予执行权限。

首次宿主check（46133）失败：E0624，Binding::validate为私有；已修正可见性。随后宿主approval_execution测试构建（63738）仍在编译，尚无测试结果。主测试夹具改为独立fixture-run/fixture-attempt，并在精确审批用例读取持久审批记录断言两者；旧构造入口的其他测试保留。变更后的测试需等待该构建并确认实际采纳最新测试源，必要时增量重跑。此阶段仍未接入chat生命周期，不计AZ-017完成。

后续结果：63738完成，18 passed、0 failed、1 ignored。确认最终测试源后增量重跑50546，同样18 passed、0 failed、1 ignored；忽略的Undo候选用例没有执行。精确审批持久身份断言与fresh后执行流程通过。

chat_turn实际入口现由宿主生成独立UUID并传入new_with_identity，结束日志携带两种身份以便关联审批。每个新turn目前建立新Run；这不是持久重试协议，尚未写入runs.db。该新增实际入口变更正在宿主check（49150）中验证，不宣称桌面或模型流程已执行。

49150宿主check最终通过。补充数据库不一致夹具：模拟Attempt已中断而Run仍排队，转移必须拒绝并整笔回滚，重开数据库后保留原Run及异常Attempt证据。最终Run专项76113：7 passed、0 failed；后续格式化及严格领域Clippy55456通过。不把本专项等同于最终全部领域套件执行。

接入边界核查：run_agentz_turn同时被交互chat_send与后台池heartbeat调用；后者没有confirmation_responses。当前独立身份入口覆盖两者，但持久运行不能只包装chat_send。chat_send等待并发许可时还可能没有session_id，实际Session在chat_turn内建立。下一阶段需在排队前签发稳定任务身份、保留跨来源续聊校验，并在共享入口统一终态及异常处理；不能仅在执行末尾写成功记录，也不能用固定TaskEnvironment替代直接CodeZ分支绑定。

## 已登记任务生命周期接入候选

共享chat_turn在审批hook建立后、Agent执行前创建ManagedRun，写queued并转running；结束按超时interrupted、取消cancelling→cancelled、失败failed、成功succeeded持久化。早退/未来被取消触发析构时尽力记录interrupted，写失败通过宿主error日志明确要求对账。成功仍awaiting_review。宿主check23476通过；专项宿主单元测试构建另行运行，尚未得到结果。

此接入仅覆盖已登记TaskEnvironment。未登记直接CodeZ继续现有行为，暂不写Run；不擅自冻结其分支。queued目前在执行入口短暂记录，并未覆盖真实并发许可等待时间。进程崩溃不会执行析构，恢复对账仍未实现。Run/Attempt重试UI、排队取消及直接绑定尚未完成，故AZ-017继续preparing。

后续30102宿主生命周期专项：1 passed、0 failed，覆盖成功/取消/失败/超时/早退、成功待审、未登记不生成Run。

进一步拆分queue/start：chat_send对已有登记的session在等待agent_slots前写queued，guard传入共享chat_turn并复用同一Run/Attempt进入running；许可获取或环境复核失败时guard析构记interrupted。后台入口传None并保留共享入口登记。新增夹具检查排队状态、排队直接取消、重复start拒绝。此批40544宿主check正在运行，新增夹具尚未执行。Stop的内存cancel槽仍在许可后建立，因此真实排队Stop尚未接入；未登记/新会话排队也尚未持久化，不宣称全排队验收完成。

40544已通过；最终生命周期夹具48298：1 passed、0 failed，新增排队状态/取消/重复start断言实际通过。

当前批次把chat取消登记移到许可等待前，等待时每25ms检查既有AtomicBool；Stop可在无许可时结束等待，已登记Run写cancelled。取消槽拒绝重复占用；显式释放及早退析构均按Arc身份比较清理，旧清理任务不能删除新登记。环境复核及其他早退依赖异步析构清理；运行时关闭时不能保证该清理任务执行，不能代替崩溃恢复。宿主check90426与新增取消专项夹具结果待确认，未做桌面Stop验收。直接CodeZ仍未写Run。

取消专项33077最终1 passed、0 failed：无许可排队Stop、重复登记拒绝、旧析构不能删除替代槽、单聊早退清理通过。

直接CodeZ新增create_direct存储入口，仅接收可信宿主完成workspace/Git验证后的Binding；验证project、未登记任务、direct指纹与binding_version一致，并持久本次Run环境摘要，不创建TaskEnvironment。共享chat_turn取审批hook验证后的Binding调用begin_bound，因此直接会话和后台共享入口也记录真实执行终态；每次turn重新验证分支，既有审批fresh策略保持。Run专项56400最终8 passed、0 failed，新增未登记不冻结及拒绝managed冒用/指纹不一致夹具通过；宿主check67542通过。扩展宿主direct-bound生命周期夹具正在构建，尚未取得结果。

直接/新会话仍未在许可等待前写Run；Run重试入口与崩溃恢复没有接入，系统时钟回退可能使状态CAS拒绝而要求对账。上述存储接口不自行证明调用者可信，renderer/model没有新增发行接口。

56931扩展宿主生命周期夹具最终1 passed、0 failed；direct-bound执行成功及未创建TaskEnvironment断言通过。

当前批次chat_send为未提供/空Session身份签发UUID，在许可等待前通过既有resolve_binding验证工作区/Git，queue_bound记录managed或direct快照，Attempt独立签发；共享入口建立审批hook后start_bound逐字段对比排队快照，Git分支/任务环境变化即拒绝并中断该Run，不执行Agent。跨来源续聊校验仍在chat_turn内保留，拒绝时Run以中断证据结束。无需把直接CodeZ登记为固定TaskEnvironment。新增夹具模拟直接分支指纹排队期间变化并检查拒绝启动及持久中断。87206宿主check运行中，最新夹具待执行；仍无桌面/模型实测、重试UI或崩溃恢复验收。

87206已通过；32930最新宿主生命周期专项1 passed、0 failed，包括直接排队指纹变化拒绝及中断断言。审批回归99048正在构建，来源兼容专项70209等待同一构建目录锁；没有重复启动审批构建。核对内核新建Session与ensure_fixed_session：未提供title时前者为NULL、后者为空串，宿主标题逻辑对空串有显式处理；完整新会话UI仍未执行。进一步在queue_bound写入前检查canonical项目与managed环境身份，拒绝错误绑定，最新补充尚待编译/夹具确认。

## 当前候选交付结论

99048审批回归18 passed、0 failed、1 ignored（Undo未执行）；70209来源分类/标题兼容专项5 passed、0 failed。最终84808宿主runtime范围15 passed、0 failed，包括最新queue_bound、直接排队指纹变化、取消清理及既有审批/工作流回归。上述为无桌面/模型的Rust夹具，不证明完整新会话UI或真实LLM执行已验收。

本任务候选代码与自动化验证阶段已交付：独立Run/Attempt、持久执行/交付状态、managed及direct排队/结束、早退中断、审批身份统一、排队取消与绑定重校验。AZ-017保持preparing，等待M1 Gate和桌面/模型旅程补验；不计已验收，不开放其依赖任务。崩溃恢复/事件游标及预算属后续任务，运行中取消即时状态的对账和Run重试UI仍不能宣称实现。接下来切换到任务级依赖已满足的独立候选AZ-042语言服务恢复，继续保持单一主要实现任务。
