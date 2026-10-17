# AZ-020 前端续聊只读复核

本包只写本文件，无产品/台账/构建变更。通过 rg 与目录检查未发现 C:/AGENTS.md、C:/Projects/AGENTS.md 或 AgentZ/src/docs 下适用 AGENTS.md；本轮沿用主线程文件归属及冻结规则。

## 最新代码事实

- `src/services/tauri/chat.ts:134` 为 clearPlan 缺省 true；WorkZ 的 chatSend 不传此字段，实际每次续聊都请求 true。
- CodeZ `AssistantPanel::submit` 已有“开放计划保留”的局部修复：存在 pending/in_progress 时 `doSend(...,!preservePlan)` 传 false。因此不能笼统说 CodeZ 所有普通 Send 都 true。真正缺口是判断依据仅本地 planItems：切 session/重启为空、Completed/all cancelled、未收到 PlanUpdate 时仍传 true，不能作为新目标授权。
- CodeZ QueuedTurn 保存 clearPlan；busy 时保留提交当刻的布尔值，出队再执行。提交时尚未收到计划的 queued follow-up 可将 true 延迟到已有开放计划之后执行，清除彼时有效计划。自动续跑与某些交互回复显式 false，仍无法补普通队列入口的误判。
- CodeZ runTurn 在调用 chatSend 前 `if(turn.clearPlan) setPlanItems([])`；后台失败也不会自动恢复原 UI。switchSession 先异步加载消息再 setPlanItems([])，没有权威计划查询；新 session 清空投影合理，不能与同任务清库混为一谈。
- WorkZ runRef 每次都 setPlanItems([])，然后沿用 startSession 再 chatSend；因此同任务普通续聊 UI 与宿主都丢开放计划。newTask 则创建新 session/解除旧 foreground，让旧运行留在后台，是另一任务，不是取消旧目标。停止按钮调用 chatCancel(taskKey)，只停执行，不是计划取消。
- 现有用户动作支持“新会话/新任务”而无“同任务显式替换目标+expected plan ID/revision”及“取消目标”命令。不能依据新的输入文字、planItems 空、全部完成或按 Send 猜作授权替换。

## 最小安全补丁，必须与宿主接线同批放行

1. 宿主先提供权威计划 DTO、查询和 typed Continue/Replace/Cancel，并保证数据库成功后才更新 cache/emit。没有这些之前不只修改前端默认 false 来制造持久化已完成的表象。
2. chatSend 默认 Continue；CodeZ普通 submit 与自动续聊全为 Continue，移除基于开放项的 `!preservePlan` 目标判断。QueuedTurn 存 typed intent，不持久化 destructive clear boolean。WorkZ沿用 startSession 时不清 planItems，首次新 task 清空投影；两端逐步移除 clearPlan 参数。
3. 同任务可添加独立“新目标”动作，使用当前权威 plan ID/revision + 新 prompt。queue中 Replace 在执行时检查 captured token；过期必须提示并刷新，绝不自动改用最新 token。新会话无需 Replace，因为 task/session 身份不同。
4. “取消目标”独立于 Stop，调用 typed Cancel；活跃 Run 先停止或由宿主明确拒绝。取消成功后显示 cancelled 历史，不伪称停止所有 Run。普通输入不能重开 cancelled。
5. 切 project/session/task 后查询计划，采用请求递增世代+project/session/env身份守卫；A 查询晚于 B 不覆盖 B。查询失败显示错误与未知状态，不回写空列表。成功 Replace/Cancel 后刷新 DTO；失败不先清空 UI。stream PlanUpdate 后刷新或携带稳定 plan ID/revision 以拒绝迟到旧 run 更新。

## 精确 Vitest 包

建议新增 `src/services/tauri/chat.test.ts` mock invoke，验证默认 Continue、显式 Replace/Cancel含当前token、无旧 clearPlan=true。

新增/使用 `src/workspaces/codez/AssistantPanel.test.tsx`：开放/已完成/未知计划的普通提交均 Continue；busy 队列在后来收到 plan 后出队不清除；自动续跑仍 Continue；切 session A→B 后 A 的迟到计划查询不覆盖 B；Replace 过期提示且不回写新token；失败调用保留计划。通过 RTL mock 服务与最小 composer props，不依赖真实Tauri/model。

WorkZ 若 index 组件依赖过重，可先抽出纯 typed intent/请求世代辅助函数测试并加一个渲染集成 smoke，但不可只测 helper 便声称 UI通过。验证同任务 runRef续聊保持计划、新Task投影清空但不cancel旧计划、Stop仅chatCancel、取消目标独立且失败保留DTO、task切换迟到响应隔离。主线程按归属统一 Vitest；本包未运行测试，未声称GUI验收。
