# AZ-020 宿主最小接线设计（只读定位）

本记录不改变产品、Cargo 或台账。主线程已报告计划 10 测试、事件 6 测试及 strict Clippy 通过；本包未独立运行 build，也不据此声明桌面或模型验收通过。

## 当前真实数据流与缺口

1. `src/services/tauri/chat.ts::chatSend` 省略 clearPlan 时传 true；`src-tauri/src/commands/chat.rs::chat_send` 也以 `clear_plan.unwrap_or(true)` 调用 turn。
2. `src-tauri/src/commands/chat_turn.rs::run_agentz_turn` 确定 session 后直接 `plan_store.remove(session_id)`。普通输入因此可能删除未完成条目。后续 prompt 从该内存表提取 pending/in_progress；turn 结束仅计算开放数，不能在进程重启后恢复。
3. 内核 `piscis-kernel/src/agent/plan.rs` 定义 `Arc<Mutex<HashMap<session,Vec<PlanTodoItem>>>>`；`tools/plan_todo.rs` 验证输入、merge 或整体替换内存、再发 PlanUpdate。EventSink 的 emit 无 Result，因此在事件桥中被动保存无法把数据库失败反馈给工具，不能作为权威写入路径。
4. CodeZ AssistantPanel 队列保存 clearPlan，并在运行前清空 UI；WorkZ index 调 chatSend 时未显式设置该字段。两个视图只消费 plan_update，不具备 task/env/plan revision 的持久查询恢复协议。
5. `active_plan_excerpt` 从 workspace 的 session plan 文件取截断文本；它是 plan 模式文本产物，不能替代结构化 todo 库，应继续独立呈现。

## 可实施最小文件归属

第一包 Rust，避免改外部内核与其 Cargo pin：新增 `src-tauri/src/runtime/durable_plan.rs`（宿主服务、映射及纯夹具）；新增 `src-tauri/src/tools/durable_plan_todo.rs`（实现相同 Tool schema，注册时覆盖 plan_todo）；修改 `src-tauri/src/runtime/mod.rs` 导出；修改 `src-tauri/src/commands/chat_turn.rs` 注册覆盖及 turn 开始恢复。由主线程在原 host 复核结束后取得归属，不能与其它代理同时改 chat_turn。

第二包命令：`src-tauri/src/commands/chat.rs` 增加 typed intent 和查询/取消命令；`src-tauri/src/lib.rs` 仅注册新命令。先纯 service 测试，再接 Tauri。不要重写 approval.rs：直接调用现有 `runtime::approval::resolve_binding`。

第三包前端：`src/services/tauri/chat.ts`（类型、默认 continuation、读取/取消）；`src/workspaces/codez/AssistantPanel.tsx` 与 `src/workspaces/workz/index.tsx`（切 task 后查询、迟到响应隔离、显式新目标动作）；各自现有 test 文件补夹具。必要的小型新 goal UI 由当前产品组件承载，普通 Send 无需额外弹窗。

另一个必要小包先改 `crates/agentz-task-state/src/plan.rs`：增加 **ItemState::Cancelled** 与有效的 terminal 判定测试。现有内核合法 item.status=cancelled，而候选只有 pending/in_progress/done，不得静默变 pending 或 done。可定义无开放条目且非空为 Completed（含明确取消条目），保留 item 级 cancelled；整个计划 cancelled 仍由显式计划取消产生。这个设计变更须单独审阅并实跑后再接线。

## 映射与接口

内核 id → PlanItem.id；content → description；pending → Pending；in_progress → InProgress；completed → Done；cancelled → 新增 Cancelled。未知值拒绝，先 normalize 再执行内核 validate_todos，以保留最多一个 in_progress 的契约。merge 使用已持久化当前项，不使用渲染器或旧 cache 作为基线；全部列表替换仍属于相同目标的 update，不能自动生成新 plan identity。

宿主生成稳定 plan UUID，objective 来自首次用户目标/display prompt（非每轮新 Run）。运行时 service 保存当前计划 ID/revision；每次工具调用先 fresh resolve_binding(project,workspace,session,run,require_binding)，与 turn 捕获的稳定 PlanBinding 对比，再 load+CAS。迟到旧 Run 工具回调不得自动采用刚读取的新 revision 覆盖新目标：它使用 turn 捕获/已成功更新的 revision；成功后仅更新自己的 token。纯计划操作不声称获批 shell/fs 能力。

建议 typed PlanIntent：Continue（缺省），Replace { expectedPlanId, expectedRevision }（用户显式新目标；目标取新 prompt），Cancel { expectedPlanId, expectedRevision }（独立命令）。plan_todo 无权将 cancelled 计划自动重开。无当前计划时首次 plan_todo 才 create；Continue 不产生空新计划。兼容旧 clearPlan=true 不可继续隐式删除：先停止两个已知调用端传默认 true；旧字段可明确返回已废弃错误，不能把 true 猜作用户新目标授权。

查询 `chat_plan_get(projectDir,sessionId,workspaceDir,requireTaskEnvironment)` 返回绑定、计划 ID/revision/status/items 或 null。查询没有执行工具授权；执行/取消入口使用已有 environment_access lease，解析登记 workspace，fresh resolve_binding，核对 host-assigned session/run identity。前端过滤 task/env +请求世代，显示 cancelled 的历史状态但不将其注入开放计划 prompt。

## 跨库失败与恢复策略

plans.db 是唯一计划权威；内核内存为可丢弃投影。写库成功后更新内存并 emit；失败返回 ToolResult::err，不 emit 成功、不变 cache。禁止先调用原工具再保存，因为原工具已经修改内存和发成功事件。

turn 开始先 fresh binding/read，成功后投影当前计划，再组 prompt；读取损坏/新 schema/绑定变化失败则明确拒绝该 turn，不用空计划继续覆盖。计划 replace 在队列许可取得且最终 binding 复查后、模型启动之前执行，用户消息记录与 Run 状态失败不反向伪称替换未发生：返回具体错误和已提交 plan ID/revision，重试必须读取再显式确认新的 token。不能自动重放 replace。

Run cancelled 不自动清计划；用户停模型与取消目标分开。Plan cancelled 不自动撤销活跃 Run：取消目标命令先要求 Run 已停止或拒绝并返回“需先停止运行”，防止旧 tool 在取消后改计划。未来若串行执行“停 Run 后 cancel plan”，任何一步失败都明确反馈，不声称跨库原子。

服务恢复不读取旧内存去回写。首次加载无计划可保留 null；旧 session 临时表/markdown 文件不自动迁移为已授权新目标。通过明确导入可另列后续任务。

## 可运行验证建议

先运行 `cargo test -p agentz-task-state plan::tests --locked` 补取消 item 状态夹具；再在 src-tauri manifest 下只运行 `runtime::durable_plan::tests` 与工具纯 service tests：重启投影恢复、普通 turn 连续两 run 保持 ID/revision、fresh binding 变化拒绝、旧 run stale revision 拒绝、工具 merge 保留稳定 ID、库失败不改 cache/不 emit、cancelled item 无损映射、取消计划不可工具重开、失败 replace 不自动重放。命令层注入测试应避免真实模型/SSH。

前端 Vitest：默认发送 continuation；切 task 的迟到 plan_get 不覆盖新 task；replace/cancel 使用当前 ID/revision，失败刷新并保留错误；CodeZ/WorkZ 读取同一权威 DTO。实际桌面重启后继续未完成目标与取消验收仍需独立运行，不能由这些纯夹具替代。
