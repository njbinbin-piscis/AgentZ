# AZ-016 合并预检基础（实施中）

2026-10-05，Asia/Shanghai，Codex。当前仅领域基础，未接入旧 merge IPC 和评审界面，未宣称现有按钮已受新事务保护。

新增 merge::preview：加载稳定 taskId，验证真实 Git common-dir/worktree/branch，要求主目录已在明确本地目标分支，不执行 checkout；评审 task HEAD 不符时要求重新评审。拒绝源/目标的跟踪文件暂存与未暂存、非运行时未跟踪源码，以及既有 MERGE/REBASE/CHERRY_PICK/REVERT/sequencer 状态。用 merge-tree --write-tree 提前发现冲突；不支持该能力的 Git 明确失败。末尾重验身份、HEAD、目标分支和 dirty；返回冻结源/目标 SHA、结果 tree 与 canonical common-dir。

新增 RepositoryAccess：按 canonical common-dir 的队列互斥，同仓库请求串行，另一个仓库可独立获取。guard 可由 blocking worker 持有；只有宿主实际接入并在出队时重验后才构成产品互斥，不把 primitive 测试冒充并发合并通过。

验证：16 项领域测试通过（29.81s）、严格 clippy 通过（3.17s）。实际临时 Git 案例核对成功预检不改主 HEAD/index/文件；错任务、SHA 目标、过期 task HEAD、dirty/staged 主目录及 dirty 源目录拒绝；真实冲突在 merge 前发现，主 index 字节与源内容不变且没有 MERGE_HEAD。同仓库队列等待与跨仓库独立测试通过。

最后补充未跟踪主文件与既有 MERGE_HEAD 拒绝，定向回归正在运行；日志 `.cache/upgrade-merge-preflight-cycle-006.log`。先收集现有运行，不重复启动。

最终定向结果：会话 26335 退出 0，补充未跟踪主文件/既有事务后的真实 Git 预检案例通过（34.18s）。当前无在途构建。本项仍为 doing，宿主事务/实际合并与前端版本契约尚未接入。

下一步：接入仓库队列与运行独占租约；评审变更集保存 task HEAD；冻结目标版本；实现合并前恢复依据、成功结果核对、commit/abort 失败现场归属判定和真实错误，删除 legacy 隐式 checkout/错误吞噬路径。完成并发实际合并、HEAD 漂移、hook/abort 故障及 UI/API 回归后再验收。

不覆盖：其他进程/外部编辑器的 OS 级互斥与原生交互。原生旅程、真实模型与平台 Gate 状态不变。
