# AZ-028 审批响应服务与IPC边界（实施中）

2026-10-05，Codex。延续单一AZ-028，先检查台账/最新证据/仓库状态，无在途构建或新外部修改。piscis-engine仅保留AZ-029原来的三个修改文件，没有扩展内核修改或发布。

新增ApprovalBroker：同一AppState boot与共享pending map，host按固定path/binding/参数签发意图，renderer仅能响应已有不可变请求。新请求限定approval:命名空间，不能误走旧bool路由。UI提供原预览digest，摘要不匹配拒绝且不消费正确卡片；批准仅进入approved，不替执行器claim。决策worker拥有receiver投递，IPC等待者取消不会让临界决策半途丢失；已关闭接收者撤销未消费批准并明确返回投递失败。取消attempt移除待响应通道并使未claim批准失效；已claim要求执行器取消/对账，不能宣称撤回副作用。

Tauri响应命令从注入的WebviewWindow获取真实label与URL，要求main及本地应用origin；release不接受开发origin，开发只接受配置的localhost:5273，extension label、外部/相似域名拒绝。自定义tauri scheme的URL origin可能为opaque，适配器仅对已核实scheme/localhost/无port或userinfo重建本地origin。没有接受renderer自报actor、window或origin。此边界不代表OS键鼠不能点击审批页，也未验收完整Webview隔离；后者仍属AZ-032及桌面能力任务。

前端PermissionCard支持原task/environment/expiry元数据与digest提交；没有摘要的新namespace不能Allow。CodeZ/WorkZ事件接收保留卡片原元数据。旧请求的异步响应不能关闭新task或新digest的卡片；投递失败不触发onResolved。旧kernel UUID bool请求仍仅作为兼容传输并受main来源检查，未将其升级为持久授权。

最终验证：审批/响应服务合计15项定向测试通过（0.29s），包含新namespace拒绝落库；严格领域clippy通过（4.84s），固定依赖宿主check通过（53.20s）。日志 `.cache/upgrade-approval-cycle-010-complete.log`、`.cache/upgrade-approval-clippy-cycle-010-complete.log`、`.cache/upgrade-approval-host-cycle-010-complete.log`。本次未重跑旧20项Git领域测试，上周期全30项回归证据仍单独保留，不能把它计成最新全35项执行。

前端最终62项及typecheck通过，5个card用例覆盖原scope/digest、缺预览、投递失败、换task及同request换digest。ESLint退出0，两个工作区有7条既有React依赖warning；无error，不宣称零warning。日志 `.cache/upgrade-approval-ui-cycle-010-final.log`。台账validate及diff检查通过，无本次在途构建。本次尚未重建生产包、运行原生点击/真实模型/跨平台/崩溃恢复。

尚未生效的执行路径：还没有工具hook签发新broker请求，旧主聊天仍走kernel bool map。下周期利用已找到的before_tool/after_tool组合hook，在fresh环境校验后issue、等待human决定、执行前claim、按真实tool-use结束；保留P0/P1委派复用。其后关闭被新hook覆盖的旧确认流程，避免重复询问或无map绕过。AZ-028保持doing，不记完成，不提升M3 Gate。

复盘：事件缺少执行关联不等于内核缺少生命周期hook；必须读到真正的执行点。可信IPC来源和摘要绑定分别约束决策者与授权对象，数据库approved又不同于claimed。前端卡片竞态虽不改变数据库参数，也会误导人，应一并消除。完整审批预览还需由可信issuer给出且不得受工具/扩展伪造事件改写；不能只相信卡片附带digest。
