# AZ-040 实施记录

主实现已从冻结的 AZ-038 限定候选转入首用输入与运行状态。正式验收仍为25/83，未提升Gate。

已实现共享输入摘要并接入 CodeZ/WorkZ：项目与配置工作区、所选/默认/未知模型来源、配置策略和运行状态；配置不表示执行授权。WorkZ Agent/Team隐藏模型选择器时显示未知，工作区只消费匹配当前session的worktree。CodeZ排队明确为本地待发，不冒称宿主RunQueued。

WorkZ使用原生details折叠高级起点、Agent/Team选项，保留既有字段、状态、事件处理及busy/task-bound禁用条件；输入和发送/停止入口保持常显。可选上下文项数量常显，不因折叠隐藏已启用选项。

独立来源审查发现既有permissionRequest缺少必需的project/session归属，不能直接用旧卡推断当前审批阻塞。本轮摘要对busy只显示执行中；既有审批卡保持原交互。完整审批归属、最近动作与5次用户走查仍待实际实现/验证，不能宣称已完成。

共享摘要最终5项组件测试通过（作者目标验证及root session93162），root scoped ESLint exit0、7条已有hooks警告；初次完整TypeScript session53141 exit0。最后显示文案/计数位置调整后的完整TypeScript99732仍待结果。独立实现审查已冻结，共享源码台账刷新53513仍在执行；勿重复台账写入。

同一AZ040下一切片为真实观察预算：仅展示available的已知用量、未知记录数、配置上限与观察超限；非available来源不补0，无法推断剩余额度或执行控制。预算展示组件正在并行实现，尚未接入或验收。

未运行原生GUI、代表用户走查、全量任务验收或新安装器构建。AZ038/AZ040后续源码均未包含于现存0.9.7合同候选安装器。

2026-10-14 当前工作树复核：重新运行 `InputContextSummary.test.tsx`、`BudgetObservation.test.tsx`、`TaskDetailsPanel.test.tsx`，17/17 通过；同一工作树前端类型检查及生产构建也通过。该检查只确认组件候选在当前源码中未回归，仍缺当前审批归属、最近动作、5次代表用户走查与原生GUI验收。

后续限定修复：CodeZ 的输入状态现在仅在当前会话收到 `permission_request` 且轮次仍 busy 时显示“等待审批”；项目或会话切换、轮次结束或取消时清除旧审批卡，避免跨任务误报。事件入口本来已按当前 `sessionRef` 过滤；本次沿用该归属约束。`npm run typecheck`、InputContextSummary 5/5 与目标 ESLint（0 error，3条既有hooks warning）通过。尚未运行原生审批流程或5次走查，不能据此验收 AZ-040。

当前会话工具事件现在提供只含工具名与 running/done/error 的最近动作摘要，不展示工具输入或输出。`toolSteps` 在项目/会话切换和新轮次时清空；未收到工具事件时不编造动作。当前组件/预算/详情测试 18/18 与 TypeScript 检查通过。仍需原生旅程和代表用户走查确认可读性及状态交错。

上述最新源码的压缩生产构建也通过（`npm run build`，3m55s；构建日志保存在本机临时目录）。构建成功只说明前端产物可生成，不替代原生界面验收。

## 最终限定候选冻结

已完成BudgetObservation最小接线，原TaskDetails任务/范围隔离保持。Root最终summary5/budget7/details5共17项通过(session27489)，最终完整TypeScript6800 exit0；预算/详情作用域ESLint15374 exit0。此前99732也已确认exit0，53513共享指纹刷新成功。预算来源无法确认时不补0，已知/未知记录和币种成本分列，未推算剩余额度。

七份源码与独立复核已保存为AZ-040-implementation-manifest.json及AZ-040-implementation-review.md，台账record成功，正式状态仍preparing。剩余审批归属、最近动作和5次真实用户走查保留后续；主实现转入AZ039交付评审首屏。
