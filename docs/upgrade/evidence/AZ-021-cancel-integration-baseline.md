# AZ-021 取消闭环接线基线

2026-10-09，只读现chat_cancel/durable_run/chat与kernel工具执行，未修改源码/台账，未运行Cargo或进程。优先补实际取消正确性，不重做已实现队列取消。

## 已有能力

- runtime/chat_cancel Registration在排队前登记flag，重复key拒绝；release/Drop只删除同Arc，不误清替换注册。acquire在等待及permit到手再次核cancel，每25ms检查，不启动已取消队列任务。
- chat.rs TaskRegistration保task_activity gate直到异步cancelmap cleanup完成；Run queue登记在permit之前，队列cancel持久Cancelled；开始再次验证环境。
- ApprovalHooks在审批等待和工具执行前核cancel，捕获同Run/Attempt；kernel工具启动前与模型退避已有cancel检查。
- proc_windows_sandbox.rs实际runner cancel会TerminateJobObject、root Wait、Job ActiveProcesses归零并drain pipe/恢复ACL；已有真实Windows受控收割fixture（执行证据见AZ034/083），不应重新造一个只杀root的取消实现。

## 必须优先解决：Cancelled早于owned工具实际退出

kernel agent/loop_.rs:2377工具运行select中，cancel_watcher赢时直接Drop tool.call future并返回“已被用户取消”。Windows tools/shell.rs:219等用spawn_blocking等待run_appcontainer_command；drop其await future不会停止或join已启动blocking worker。worker收到flag后会清Job/输出/ACL，但caller已经失去完成确认。

host chat_turn.rs:2805按cancel flag立即ManagedRun.finish；durable_run finish Running→Cancelling→Cancelled连续写后释放task_activity/调度slot。这里未wait owned进程/worker清理确认，因此“Cancelled”只证明token信号被处理，不证明owned执行退出。下轮可在旧ACL/Job cleanup仍在途时启动，是用户可感知取消正确性缺口。未实跑时序复现；这是明确源码路径推导。

最小修复不是改用户文案为成功停止，也不是仅sleep固定200ms：增加attempt-scoped owned-operation completion barrier。spawn_blocking开始前登记operation，同capturedcancel/session身份，worker完成Job/pipe/ACL cleanup才完成receipt；loop允许Stop signal但host保留Cancelling/环境lease/task gate/slot，等待有界barrier全部确认。cleanup失败或超时记录Interrupted/明确unknown（不能Cancelled），不自动重跑。阻塞worker Drop receipt只有真实return才确认，不能futureDrop即成功。

建议文件归属：先独立kernel operation registry/ToolContext最小接口及shell/code_run登记worker，由单代理维护source+固定vendor版本patch；host runtime新cancel_completion服务由另一代理，chat_turn/durable_run共享接线主线程。所有callsite启用范围须核全，不把未来remote/browser/桌面异步操作自动纳入已确认。

## scope与返回语义

chat_send的task_key是renderer提供的取消索引，而稳定Task gate按canonicalproject/session。chat_cancel只有task_key或global single，无projectscope/Run/Attempt参数；chat_cancel_all等待最多5秒后无论仍活跃都返回Ok。当前key是选择器，不是授权证明，也不表示收到返回就已停止。

未证实不受信Webview可调用该command，故不宣称已复现权限漏洞；但新的取消receipt API应按host captured project/task/run/attempt注册，由main-window scope核对，不用随机rendererkey当稳定身份或允许旧key取消新attempt。可保留旧Stop信号接口兼容，另提供requestAccepted/pending/confirmed/failed状态；all超时不得声称全停止。

队列注册、active gate和Arc cleanup已有fixtures，不需推倒重做。业务Cancelled与执行工具已确证退出是两个事实：没有owned资源的模型等待可以及时确认；工具cancel必须有真实completion，外部副作用不能撤销也不能因Stop自动重试。

## 最小受控fixture

1. mock owned worker：Started握手后Stop；worker故意延迟cleanup，断言Run仍Cancelling且新sameTask gate/slot不可取得；释放cleanup才Cancelled。fixture不能只检查AtomicBool=true。
2. worker cleanup失败/有界等待超时：状态Interrupted/unknown、错误明确；不能Succeeded/Cancelled，无新工具调用或自动retry。
3. 真实Windows AppContainer parent+child握手：正向存活对照，cancel后确认root与Job后代均退出/pipe完成/ACL恢复，receipt之后才发final；保持已有runner测试使用，不重复重写底层。
4. queued Stop在permit竞争到手/环境重校时仍零工具/模型调用；旧Arc/旧Attempt receipt不完成新attempt，跨task不收割他人owned资源。
5. turn timeout/drop与userStop同时：持久终态按真实cleanup结果，监督registry继续拥有worker；进程崩溃尚无owner proof时保留Unverifiable，交AZ019，不猜已退出。
6. chat_cancel_all超过等待预算仍活跃返回pending/timeout，而非全停止完成；main-window/scope/oldAttempt边界另独立IPC夹具。

## 实施优先顺序

先冻结单local Windows shell/code_run的owned completion契约与mock时序fixture，主线程接最终化barrier，再跑既有Windows runner正向/取消确认。其他非合作工具、remote、UI/browser、团队/池应逐项列Unsupported/unknown，不能把本包变全部资源lease重构。用户允许后移GUI仍不允许用token flag假称进程退出；整AZ021验收保持未完成。
