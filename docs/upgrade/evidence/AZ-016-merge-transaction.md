# AZ-016 带版本与恢复依据的合并事务

2026-10-05，Asia/Shanghai，负责人与审核 Codex 自检。按身份/版本预检、受控合并与失败恢复依据范围完成；不宣称原生桌面或发行验收。

宿主替换 legacy checkout+merge 入口：缺少稳定 taskId、评审 task HEAD 或目标 HEAD 明确拒绝。完整变更集新增 head_sha，在采集结束复核未漂移；评审界面以它请求预检，再用预检 target HEAD 提交事务。源、目标漂移返回真实错误，不能默默换新版本。

预检与提交都按 canonical Git common-dir 入同一队列。提交出队后取得执行/journal 独占租约；其他活动/排队回合阻止目录事务。出队重验所有身份、dirty 和版本条件，不为合并隐式 checkout。RepositoryAccess 的克隆共用队列，不能因 AppState 克隆而分裂锁。执行租约仍是进程级保守互斥，跨项目的独占事务不是完整并发调度优化。

事务先固定 `refs/agentz/merges/<id>` 到目标旧 HEAD，保存 AZ-014 task 源快照，再发布 schema1 planned receipt 到 `.agentz/merges/<id>.json`。捕获恢复数据后再次预检，才调用 Git merge --no-ff。合并使用用户 Git 身份与配置；恢复元数据身份不会成为用户合并提交作者。成功后核对 tree、目标分支、父对象和 clean 状态，返回 merged/unchanged 与实际 after_head；分支不删除。

失败：只有原 HEAD/目标分支、MERGE_HEAD、index 与预检结果一致且没有后续 tracked 编辑，才调用 abort。hook 修改了现场则保留；abort 再失败明确返回，不吞掉错误。Git mutation 有60秒超时，超时保留现场，不自动 abort。元数据发布失败保留实际 commit/ref/snapshot，返回实际 state 和恢复引用；磁盘旧 planned 不冒认已更新。恢复依据为用户检查与后续恢复操作保留，不自动 reset --hard。

独立 Tokio worker 持有仓库/执行 guard；IPC waiter 取消不会直接取消 worker 或提前解锁。前端展示真实错误与恢复引用，需恢复或已经合并但收据失败时阻止再次提交；过期/没有 HEAD 的评审不能合并。

实际证据：

- 真实 Git 事务定向4项通过（101.49s）：并发同仓库事务第一项合并、第二项因旧 target SHA 拒绝，刷新后合并两份实际文件；父对象与 Merge Fixture 用户作者核对；冲突/dirty/staged/已有Git操作/错任务/过期评审拒绝，HEAD/index/字节不被预检修改。
- 提交 hook exit1：可证明归属的现场 abort 成功，HEAD 与主文件恢复，任务文件保留；hook 编辑主文件：不自动 abort，该字节与 MERGE_HEAD 保留；hook 创建 index.lock：abort 实际失败，完整错误返回、锁保留，receipt needs_recovery。
- 最终 receipt 发布故障：实际 merge commit 与文件成功，旧 planned JSON 保留，错误 state=merged，恢复 ref 可解析到原 HEAD。
- 领域19项全回归通过（100.82s），严格 clippy通过（5.38s）；其后补齐错误状态与取消 waiter 案例，最新全回归正在运行。
- 取消 waiter 的实际任务案例通过（36.53s）：worker 继续、租约阻止第二个独占操作，完成后释放并持久 merged 收据；不把它冒充原生 IPC 点击测试。
- 前端9文件/57项测试通过（5.32s），含冻结身份/版本传递、preflight失败不调用 merge、恢复引用与阻止重复提交、缺少HEAD禁用，以及旧归档/评审回归。typecheck退出0、修改文件ESLint退出0。
- 宿主队列改动后 cargo check通过（1m36s），日志 `.cache/upgrade-check-cycle-007-queue.log`。首次宿主检查因 RepositoryAccess 缺 Clone 失败，已改共享 Arc 队列并补克隆互斥回归；失败未记作通过。

最终结果：20项领域全回归通过（98.38s），日志 `.cache/upgrade-task-state-cycle-007-complete.log`。严格检查首次因测试中的冗余 async 包装失败，改为直接 spawn JoinHandle 后，严格clippy通过（4.42s），取消 waiter 定向回归再次通过（40.16s）；日志 `.cache/upgrade-clippy-cycle-007-final.log`、`.cache/upgrade-merge-cancellation-cycle-007-final.log`。宿主最终check退出0（15.94s），日志 `.cache/upgrade-check-cycle-007-complete.log`；生产构建退出0（5m37s），日志 `.cache/upgrade-build-cycle-007.log`，仍有既有大chunk/混用导入提示。台账和升级工具隔离回归通过，无在途构建。

恢复操作依据：先保留失败现场，读取 transaction JSON、实际 HEAD/MERGE_HEAD 与错误 state；恢复ref指向合并前目标提交，可先比较而不是覆盖当前目录。task_archive_id 可通过已实现源码快照接口恢复到新目录。需要恢复的现场没有自动删除锁或 hard reset；完整原生恢复向导仍待后续旅程验收。

边界：这是应用进程内受控事务与标准 Git 配置案例，不是 OS 沙箱；外部编辑器、另一个进程及自定义 Git 程序不受该锁控制，未宣称覆盖所有副作用。未验收：原生点击/重启恢复、平台差异、真实模型、全应用崩溃恢复与永久清除。AZ-009/013 和 MB Gate 状态不变。

复盘：只给按钮加 dirty 禁用不足以授权合并；稳定身份、评审SHA、目标SHA、队列出队重验、结果证明与持久恢复依据需同时成立。abort必须是有证据的恢复动作，不能用best-effort吞错误或覆盖hook/用户的新改动。
