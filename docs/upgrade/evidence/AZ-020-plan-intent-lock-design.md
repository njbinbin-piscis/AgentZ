# AZ-020 目标变更与排队执行互斥设计（只读）

未产品/build/台账变更。结论：已有 environment_access.execution 是全应用共享读租约，不能证明同task无排队/执行；task_cancel也不是稳定session索引。typed intent callback仅传“当前没有run”不足以实现互斥，必须将所有排队和目标变更接到同一真实gate。

## 当前事实

AppState.chat_cancel是一个CodeZ全局flag slot；task_cancel按前端每轮taskKey存flag，WorkZ taskKey是每轮随机值，不是session/task ID。Registration只确保taskKey或single slot不重入，不提供task→所有queuedrun的索引。不能通过task_cancel.contains_key(task_id)判空；chat_cancel=None也不能证明WorkZ任务空闲。

chat_send现在在environment读lease后fresh binding，先ManagedRun::queue_bound写queued，再Registration::new，再await agent_slots permit，再复查workspace，最后start_bound。RunStore有 `active_task_run` unique partial index：每个task ID仅一个 queued/running/cancelling，但只有写queued才触发；它不与plans.db目标变更同事务。未见公开task active查询。heartbeat路径不经过Registration/queue_bound，在turn晚期才begin_bound，同样需纳入前置gate。

## 最小真实锁与文件包

新增 `runtime/task_activity.rs`：AppState持 `TaskActivity`，内部Map<(canonical_project,task_id),Arc<tokio::sync::Mutex<()>>>，返回OwnedMutexGuard；公开 `try_enter(project,task)->Result<OwnedMutexGuard<()>,String>`。try_lock失败立即“task queued/running or goal mutation active”，不隐式等待再替换目标。map短期不删除entry避免同key同时形成两个lock；后续可按强引用安全GC。键不包括environment_id，环境交接不能绕过同task互斥。canonical_project使用host解析，task ID必须host验证existing session/source，renderer字段不能任意另起一个key。

`state.rs`增加task_activity初始化；`commands/chat.rs`在host分配task ID后、freshresolve/queue_bound前取gate，跨queue wait、registry/prompt、模型执行直到Run终态落库与Registration释放结束。cancel flag注册仍单独保留。取得gate不要求持任何cancel-map锁，防止await跨互相等待。多task不同key仍并行，agent_slots保持资源限制。

`runtime/heartbeat.rs::dispatch_pool_attention`在environment读lease后、ensure session/turn前以attention.session_id取同gate；不再允许“heartbeat模型正在构造但尚未queued”被目标变更穿插。早期失败drop gate。其它未来运行入口必须同一规则。

`commands/chat_plan.rs`（拟新增）Replace/Cancel在环境读lease后，以canonical project+validated task取同gate，fresh resolve_binding，再读权威Plan和expectedID/revision，查询RunStore是否有活跃task Run，最后调用PlanStoreCAS。全程持gate直到commit/cache/queryDTO结果形成。callback可用作业务适配但不能替代guard。拒绝busy时不自动chatCancel，更不能认为Stop flag=true就已终态。

`crates/agentz-task-state/src/run.rs`新增 `has_active_task(task_id)->Result<bool,String>`，查询queued/running/cancelling，unknown/corrupt state failclosed；确保schema已验证。该只读检查防进程重启后数据库尚有active Run而in-memory gate为空。恢复流程必须显式将遗留Run标Interrupted后才能改目标，不能自己把任何active行当stale删除。可返回active Run摘要以向UI解释busy，但不暴露内部SQL。

## 次序与失败边界

统一顺序：environment readlease → task gate → freshbinding → RunStore/PlanStore操作；不要拿cache锁后再取gate。queuedguard与taskguard的释放顺序必须先Run终态/dropManagedRun，再release registration，再drop task gate，避免“gate已空，Run仍running”短窗；函数显式tail/drop或包装RAII含两者。Run异常drop记Interrupted失败时activequery仍拒绝目标变更，不伪称空闲。

Replace/Cancel不占agent_slots，因为不执行模型；仍需task gate。query只读无需独占taskgate，但要返回完整DTO身份/revision，UItoken只供CAS不授权限。cancelled计划≠cancelled Run。

此方案保障同一AppState/单桌面host内互斥，跨SQLite库不原子；**不能宣称支持两桌面进程共享同项目并发**。另一个进程不遵守内存gate，activequery→plancommit仍有跨进程queue竞态。若产品允许多进程同项目，必须增加跨进程task锁（OS锁或同runs.db reservation事务/统一库）与所有queue入口配合；本次候选先明确单host边界，不能隐式放行多进程验收。

## 必须实跑的夹具

TaskActivity：同keyexecution/intenttryenter互斥，不同project/task并行，drop/earlyerror释放；执行持gate在零agent_slots排队时Cancel/Replace拒绝；heartbeat即使尚未begin_bound仍busy；busyflag设true但未终态仍拒绝；RunStoreactive遗留记录即使gate空也拒绝；期望revision过期不写；mutation期间同task新queue拒绝而其它task正常。需真实Plan/Run库以及真实guard，不用“callback返回false”镜像夹具替代。
