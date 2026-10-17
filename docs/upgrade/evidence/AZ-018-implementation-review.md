# AZ-018 独立实现契约复核

2026-10-09。Reviewer：Codex (remote_tests independent review)，与宿主实现/台账owner分离。只读复核当前源码及既有实际测试证据，未修改源码、台账，未运行Cargo/GUI/模型。当前source指纹见AZ-018-implementation-manifest.json，任何文件变化使本限定复核失效，须重新核对。

## 可供AZ019候选消费的限定契约

允许仅消费本地普通单Agent的Run/Event持久契约，不表示AZ018原任务done或任何里程碑Gate通过：

- `events::Stream`固定task/run/attempt/environment四维身份；Cursor包含完整stream+非负sequence，跨stream/future cursor拒绝。Event含scoped cursor、稳定event_id、kind、JSON payload和created_ms。
- events::append接受调用者Transaction，不自行commit；scoped event_id精确内容/时间幂等，冲突失败。RunStore create/transition/retry/delivery在同一Immediate事务内状态+生命周期事件commit；event失败使状态rollback。AZ019必须调用权威RunStore transition，不能独立写Run后另外append并声称原子。
- replay在同事务快照确认整stream整数正序连续（min=1/count=max），先整页metadata/type/identity/time及1MiB单payload/4MiB页预算校验，再有界JSON读取。缺口/坏类型/超预算明确失败，不返回看似完整部分成功。domain页limit1..1000，宿主固定1..200且limit+1判断has_more。
- RunStore已有库只读入口不create/repair/migrate；旧v1无events仅允许空历史与cursor0，不合成已运行历史。空历史不意味着任务从未执行。partial/future/空既有metadata/缺scoped event ID unique拒绝。
- Run/currentAttempt/historicalAttempt权威来源保持校验；历史来源可读不重新执行。AZ019对活跃Run的对账不能据空cursor自动resume，也不能凭UI最后Done决定终态。需要先证明本地执行owner失活，再用当前状态CAS到Interrupted；该恢复策略是AZ019新包，不是本上游已具备。
- host event_replay在canonicalproject/fresh PlanBinding对应task/env下读取Run及真实Attempt，并核same-task current元数据；只读PageDto返回stream/events/next_cursor/has_more，不执行工具或模型。
- IPC chat_current_run/chat_run_events由main window project scope授权，保持environment lease，existing_binding核已有session/task，异步后再with_verified_scope复查scope。renderer不能指定任意路径读取主目录。AZ019启动内部路径也须由可信host确定，不能复用renderer任意project参数。
- 本地普通chat DurableStreamWriter捕获Binding/currentRunning，AgentEvent提交成功才发布；存储/序列化/collector Join失败导致cancel及明确Failed，timeout不覆盖durability原因。不是所有event sink/模型/成员入口覆盖。

## 本次独立源码核对

实际读取events.rs append/replay两阶段和schema、run.rs事件布局/生命周期事务与只读读历史入口、runtime event_replay/durable_stream、commands chat_events及其scope/binding接线。与此前独立只读审查、最新AZ024修改后的Run事务helpers相互核对：新增预算helper没有改变Run/Event commit/replay契约。限定消费范围未见必须修复的P1/P2。

不消费全聊天GUI重建/所有任务/remote/team/pool/delegated、任意长历史、完整费用、进程执行恢复或OS工具再次执行语义。前端RunHistoryPanel的当前Attempt被动预览只是上层consumer，不作为AZ019核心开工前置。

## 已有实际验证（引用，不冒称本人运行）

- AZ-018-run-events-integration.md：主线程54983 latest Run14通过及strict Clippy，覆盖同事务事件失败rollback、重开、retry旧Attempt历史及旧v1/partial/unique布局。
- AZ-018-chat-stream-integration.md：65529 full task-state86全部通过/strict Clippy；最终writer4、event_replay6通过，覆盖先commit后callback、失败callback零、existing不建库、queued/terminal/stale拒绝、精确dedup及预算rollback。collector异常/timeout错误分类是独立源复核+宿主编译，不冒称真实模型故障注入。
- AZ-018-two-phase-replay-integration.md：73190 full task-state88通过/strict Clippy；56385受控1040独立commit、1042总events六页比对通过。单机性能样本非SLA。
- AZ-024-accounting-core-integration.md：52758当前Run预算helper修改后full task-state97通过、strict all-target Clippy通过，新增损坏Run/缺PK失败关闭，不改变事件原子契约。后续reservation包测试由主线程另记录，非AZ018新事件实现。
- AZ-018-local-candidate-freeze.md：两代理独立只读收尾未发现限定当前候选新增P1，保留39前端/88Rust证据及明确scope限制；实际新NSIS见AZ064构建证据。安装器和最新源码指纹不能互换。

原生GUI断流/重启、真实模型磁盘失败停止、掉电、跨平台、安装/回滚及备份恢复仍未执行。正文测试数字是各快照事实，不相加作更多测试，也不证明所有当前源码都在旧安装器中。

## 复核结果

**AZ019可在preparing下消费上述当前Run/Event持久与只读历史契约；AZ035可消费同一限定DTO/身份读取契约。** 只满足代码实施排序，不提升AZ018正式状态/任务完成数/Gate。主线程仍须通过upgrade-plan.mjs登记独立implementation_review及最新manifest，然后开启下游code_dependencies；不能绕过脚本或把未跑验收写成done。

2026-10-09新增chat_task_details沿用main window、opaque scope、existing Binding与environment lease，并在await后再次验证scope；既有Run/Event读接口未改。新宿主详情测试27341正在编译，不宣称实跑完成。
