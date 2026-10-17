# AZ-020 单宿主 task activity gate 前置包

新增runtime/task_activity.rs，AppState持TaskActivity；键为host canonical_root(project)+稳定taskID，不包含env或每轮随机cancelkey。Map存Weak<Mutex>，每次进入清理无强引用entry；OwnedMutexGuard保持锁实例生命期，避免同key产生两个有效锁。try_enter拒busy，不排队等待后偷偷变更目标；不同project/task继续并行。

chat_send持environment读lease后、workspace/kernel/queue_bound之前取taskguard。heartbeat在environment读lease之后、ensure session/turn之前取同guard。两端均不在turn嵌套租约。guard只是同task执行/变更互斥能力，不授予工具权限；后续typed Replace/Cancel命令必须调用同gate并fresh binding，当前未接目标命令。

chat内新增TaskRegistration持guard，normal release先await cancellation Registration.release再dropguard；early return/abort Drop将registration+guard交给清理任务，先release后dropguard。release使用borrow across await，防中途取消丢失registration。queue_bound由原Registration之前移至TaskRegistration构造之后，局部逆序drop确保ManagedRun同步终结再启动registration/taskguard释放。queue注册和agent slot资源限制行为保留，不执行模型或扩大并行度。

如runtime已关闭无法spawn，局部guard仍按drop释放；durable Run终态写失败不能靠本gate证明task空闲。后续变更命令仍必须查真实RunStoreactive记录，遗留active拒绝。跨进程同项目未由内存gate解决，不宣称跨库原子。

主线程复核修正：agent slot acquire失败且非用户cancel时，原错误分支queued_run仍alive便先release登记/gate；现显式drop queued_run先同步记Interrupted，再await release。用户cancel分支finish Cancelled本已同步consume；finish失败时Run guard Drop先于外层TaskRegistration清理。workspace/journal其它早错依赖queued_run后声明的逆序drop；turn返回后其Run已消费/终结，再release。70584在途编译结果不能替代这次修改，需统一增量重跑。

runtime关闭导致Handle不可得时，Registration本身Drop也无法spawn；in-memorycancel slot可能残留至AppState销毁，不能声称异步登记一定完成。正常运行期TaskRegistration清理持gate直至release完成；关闭失败边界须依靠Run持久终态/下次恢复active检查，而非内存gate作已完成证据。

新增tokio fixture验证同规范project/task互斥（含路径`.`别名）、不同task/project并行、零slot排队时同task仍busy、drop释放、50临时key弱map回收、空task拒绝；未运行Cargo。主线程负责runtime mod导出并实跑。保留各文件此前dirty，未改Cargo/lib/台账/目标命令。

实际运行限定rustfmt与diff-check exit0。建议统一执行runtime::task_activity::tests并检查host编译；后续添真实typedintent+queued run集成夹具，不以callback“idle=true”替代gate验证。主线程应刷新chat/state/heartbeat受影响复核指纹。

新增commands::chat::continuation_intent_tests两条真实清理fixture（未在本代理运行）：持tasks map mutex阻塞Registration真实cleanup，分别触发TaskRegistration Drop和normal release；在cleanup受阻时验证cancel slot仍存在且task gate busy；解除阻塞后bounded await，验证slot移除后gate才可进入。使用真实Registration/TaskActivity与唯一temp项目，不复制cleanup实现。主线程统一增量实跑；真实Run先terminal的错误分支仍通过代码与Run guard测试组合验证，未以这两条清理测试替代跨库验收。

Primary70584 run::tests17 passed/0failed, host task_activity fixture1 passed/0failed exit0. The queue-failure cleanup edit and additional actual TaskRegistration cleanup fixtures require an incremental host rerun; do not use this earlier result to attest later source. Scope registry and actual gate/Run/plan service adapter are isolated follow-ups, no IPC registration or cross-process guarantee.

