# AZ-028 持久审批领域核心（实施中）

2026-10-05，Codex单独执行、自检。开始前读取台账、进度、AZ-027/029最新证据；无适用AGENTS.md，无在途cargo/rustc构建，保留已有修改。AZ-029仍review，没有绕过其依赖启动AZ-030。

新增 `agentz-task-state::approval`，SQLite schema1独立intent和追加audit；数据库路径、process boot、时钟、身份与实际资源由宿主提供，模块未开放renderer或模型签发接口。绑定project/task/environment、binding版本、run/attempt、capability版本，规范化动作、资源与参数后计算SHA256；object键排序不依赖serde_json feature，数组保留语义顺序。持久层仅保存身份/动作/摘要/期限，不存原参数或资源原文。

状态分离：pending→approved/denied，approved→claimed（IMMEDIATE事务一次消费），claimed→executed/failed/unknown；审计与状态同事务，审计故障使claim回滚且返回错误。scope/参数/账号/目标/动作变化撤销旧批准，期限到达或审批后时钟回退失效，消费后拒绝重复批准/重试。完成结果允许在批准到期后记录，避免将已经发生的副作用伪称未执行。

重开同boot保留实际记录；新boot把旧pending/approved置expired，旧claimed置unknown，不自动重发。重复恢复不追加虚假重复事件。重复request拒绝，未知schema拒绝降级，非法记录返回错误。撤销尚未消费的批准；已claimed明确返回需要取消，不能声称撤回副作用。

宿主小修复：respond_permission_request复用受测legacy投递helper，接收者关闭时返回“decision was not delivered”；不再吞send错误。此helper仍是旧bool传输，不能当作持久授权或执行完成证明。

验证：10项定向测试通过（运行0.16s），严格 `cargo clippy -p agentz-task-state --all-targets -- -D warnings` 通过（40.02s）；日志 `.cache/upgrade-approval-cycle-009-final.log`、`.cache/upgrade-approval-clippy-cycle-009.log`。真实SQLite重开/并发两消费者仅一claim、触发器注入审计写入失败、唯一虚构秘密扫描数据库文件、schema99保留、closed oneshot投递与重放均有实际测试。

最终全领域回归30项通过（运行92.83s），包含原任务环境/起点/变更/归档/清理/合并事务20项；日志 `.cache/upgrade-task-state-cycle-009-complete.log`。固定依赖宿主 `cargo check -p agentz-desktop` 通过（包含锁等待2m49s），日志 `.cache/upgrade-approval-host-cycle-009.log`。台账validate、隔离plan.verify和git diff --check通过；本次构建均已退出。没有前端改动或原生验收，本次未重跑生产打包。

未完成接线：固定kernel仍产生request_id+bool，没有fresh scope/claim/执行结果钩子。领域模块尚未成为现有工具的执行授权器；受保护human审批通道、真实任务/环境解析、账号/surface资源提取、重启UI恢复、撤销/取消传递和执行结果对账仍待完成。未验证原生点击、模型、跨平台或崩溃旅程；M3 Gate不变，AZ-028保持doing，不计done。

安全边界：不保存参数原文不等于已完成秘密引用/脱敏，生产issuer仍须在摘要前将秘密换为reference（AZ-031/077/078）；未将内存哈希称为秘密防泄漏方案。仅应用事务持久化，不是OS隔离；外部工具访问审批数据库的限制仍需文件/进程执行器落实。新schema回退不得恢复旧批准。

下一步：在工具执行前接入host issuer、protected human decision、fresh execution claim及finish；将卡片绑定原任务/环境而非当前视图，加入取消/过期/重启案例，任何接线失败拒绝操作。若固定内核需新增接口，先保留可复核本地补丁和候选宿主证据，不自行发布。

## 接线检查结果

`chat_send`已在排队前/后解析任务环境并持有执行租约，可以作为身份来源；`run_agentz_turn`内部才固定实际session，故不能将renderer的空session或active view当task ID。每个turn须签发独立run/attempt，boot由共享AppState生成，不能每次打开数据库生成。

固定kernel的PermissionRequest位于agent/messages.rs，包含request_id/tool_name/tool_input/description，缺少实际tool-use ID。loop_.rs的审批在PolicyDecision::Warn分支，approved bool后才发ToolStart；不能只在事件collector里记录claimed就宣称执行前重新验证完成，也不能按相同tool_name猜测并行调用的执行结果。需要在实际执行点增加可靠授权/结果关联；只读事件旁路不足以完成本项。

现有confirm map缺失或用户关闭confirm flag时的行为也须纳入接线检查，不把这些条件解释为持久P2/P3批准。P0/P1任务委派范围按AZ-027复用，避免为了接线把所有普通编辑都重复弹窗。执行器参数/资源必须由可信resolver提供，审批按钮仅表示human决策；claimed后取消/超时或进程崩溃的未知结果保留unknown并对账，不重发外部副作用。

2026-10-05补充更正：继续检查loop_.rs时发现现有AgentHooks::before_tool在实际tool.call之前执行、after_tool在结果发出之前执行，ToolHookEvent包含真实tool-use ID、参数、session和workspace。之前“缺少执行前fresh授权接口”的说法过宽：缺失的是主机接线，固定内核已有可用生命周期接口。PermissionRequest旧事件仍缺tool-use关联，但可由host hook直接签发新intent并在对应after hook结束，无需为此新增内核接口。下一步在JournalWithIdeNotify外组合授权hook，保留journal及上下文通知；不能只靠事件collector旁路。
