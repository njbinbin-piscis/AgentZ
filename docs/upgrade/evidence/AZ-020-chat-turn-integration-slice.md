# AZ-020 chat turn 最小接线切片（只读）

只读设计，未产品/build/台账变更。正式修改由主线程取得 chat_turn 独占归属，并刷新受影响 ADR006 review 文件指纹，不能沿用旧 host 复核结果。

## 当前精确顺序

`run_agentz_turn`：约1907 build mutable registry（内核 plan_todo 先注册），随后 MCP/connector 注册；约2000 canonical workspace；约2030确认/创建 session；2087 clear_plan 为真删除共享 cache；随后写 user message，约2303将 cache 的 pending/in_progress加入 prompt；约2356把 registry 包成 Arc；2365才选 queued Run或新 run/独立attempt；2378创建 ApprovalHooks（内部 fresh resolve_binding）；2395 start_bound queued Run/新 begin_bound；最终 harness使用同一 cache。

`chat_send` 在 queue 前持有 environment_access.execution lease，host分配session/run/独立attempt并queue_bound；slot实际取得后再次 resolve workspace，随后调用 turn。`runtime/heartbeat.rs` 也直接调用 turn，当前传 clear_plan=true、queued_run=None，未见 environment_access lease/agent slot。不能只接 chat_send而遗漏这个调用端。

## 可直接落地顺序与API微调

1. turn入口尽早拒绝 deprecated clear_plan=true，返回“clearPlan=true 已废弃；普通续聊不清计划，新目标须显式接口”。必须在 registry/MCP、ensure session、append user message等副作用前拒绝。绝不把 true 当 Replace。false=Continue。
2. workspace规范化与session确定后，将现有2365的 `(run_id,attempt_id)` 分配原样前移；queued identity保持不变，None分别生成UUID。规范化 project；`require_binding = workspace!=project || task-state::load(project,session).is_some()`，与 ApprovalHooks当前策略一致。
3. fresh `resolve_binding(project,workspace,session,&run_id,require_binding).await`，补入独立 attempt，validate。打开 PlanStore，recover PlanBinding；任何损坏/绑定变化明确失败，不清空库、不写 user message。
4. 工具构造与 projection恢复须来自 **同一次 recover**，避免“读A恢复prompt、读B构造tool”并发取到不同 revision。建议 DurablePlanTodoTool新增只读异步 `projection()` 或构造返回 projection/token；new已recover并捕获CAS，从其状态得到投影，锁共享kernel cache插入session键。closed为运行上下文空投影，库历史保留；无当前计划为空，不create。
5. 现在才 append user message和构建开放项prompt；关闭/Completed不注入开放todo。需要UI恢复可在成功恢复后emit PlanUpdate投影，但它只代表运行上下文，不能替代包含closed status的权威query DTO。
6. 在最后一次mutable registry覆盖点、Arc::new之前注册 DurablePlanTodoTool；若slash/skill限制原registry不含plan_todo，不无条件扩权重加工具。先检查registry是否有同名现有工具，才替换；确保connector不能后来覆盖回旧plan_todo。以当前 Tool schema维持模型接口。
7. 创建ApprovalHooks的身份仍用相同run/attempt，保存 its binding克隆并与步骤3绑定逐字段比较，变化即拒绝模型启动；queued `start_bound`仍保留，None `begin_bound`仍保留。不能以“计划库恢复成功”跳过Run Guard/ApprovalHooks。

共享 lease：chat_send已持有，不能在turn嵌套再次读锁（待writer时读锁公平队列可造成自己持读锁又等读锁）。最小做法让heartbeat调用端也持有一次execution lease并取得slot贯穿turn；或统一把lease ownership重构到turn并移除caller原lease，但这扩大范围。本切片优先caller显式保证，继续由guard在early error/drop中结束queued生命周期。

工具每次写入fresh resolve_binding，保持state→cache锁顺序，commit后cache/event没有await。宿主lease不能由tool自行假定：两个turn调用端须都覆盖全程；旧Run的cancel Arc +captured token保持原样，不能为复用工具换成任意ctx.cancel。

## 必须同批改动的默认与调用端

- `src/services/tauri/chat.ts` clearPlan缺省改false（或typed Continue，暂时不扩大UI）。
- CodeZ `AssistantPanel.tsx` 普通submit总是false，取消基于本地preservePlan的破坏性判断；已有queued true必须清理/禁入；删除runTurn清UI分支，保留新session的投影清空。
- WorkZ index.tsx chatSend明确false，删除普通runRef开始无条件setPlanItems([])，保留newTask/openTask身份切换的投影清空。
- `commands/chat.rs` unwrap_or(false)，最好在queue前拒绝Some(true)，避免无意义的queued Run/用户消息；turn保留第二层reject以保护其它调用端。
- heartbeat.rs把true改false并补caller lease/slot，或明确暂时不接其durableplan并记录禁止清除。不能让统一turn reject后整个heartbeat长期失败。

本切片不新增Replace/Cancel UI；deprecatedtrue明确拒绝，用户同任务新目标仍缺独立接口，记录为剩余验收项。新session/task是新的任务身份，正常首次plan_todo创建计划。停止Run不取消计划。

## 原子验证与恢复边界

测试最小组合：服务/tool已有夹具；接线helper夹具验证“同一recover投影+captured revision”、旧registry限制不扩权、approval二次binding变化拒绝、deprecatedtrue在副作用前失败；TS invoke默认false、CodeZ忙队列false、WorkZ续聊不清投影。命令错误/库失败不得以空投影继续模型；cache可丢弃，commit后重启从plans恢复。

Run库与Plan库仍非原子。读取恢复失败发生在queued Run start前，guard负责中断记录；不要伪称 Run已成功取消或计划回滚。模型执行后故障只更新Run，不clear计划。跨平台/GUI/model重启验收仍未运行。
