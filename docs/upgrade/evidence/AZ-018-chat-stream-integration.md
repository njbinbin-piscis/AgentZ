# AZ-018 本地聊天事件先提交后发布候选

2026-10-08，主线程接线；原正式状态保持 preparing。RunStore append API 与宿主 writer 由两个 Codex 子代理独占实施，第三个代理只读审查 collector 接线。适用 AGENTS.md 搜索无结果，既有修改未重置或批量暂存。

append_agent_event 在 Immediate 事务内校验 captured binding 的 project/task/environment/Run/current Attempt，要求 Run 与 Attempt 均 Running、NotReady 交付及合法时间元数据；agent. 稳定 ID 命名空间、agent.event 类型，复用 1 MiB JSON 预算与精确幂等规则，commit 后返回真实 Event。不会改 Run 时间、交付或执行状态，不授予工具执行权。现有库 writer 使用 READ_WRITE 且无 CREATE、初始化或迁移，避免存在检查后删除文件时新建空库。

本地普通 interactive chat_turn 在实际 early_binding 所指执行上打开 DurableStreamWriter。collector 的 AgentEvent 序列化后先 record_before_publish，提交成功 callback 才按原 payload 发布 agent_event，再处理派生提示；Text/ToolStart/ToolEnd/Error/Done 等通过此通道的事件可由已有 scoped replay API 读取。远程/团队/池入口及绕过该通道的直接 EventSink 发布不在本包覆盖范围。

写入或序列化失败明确显示错误，设置 cooperative cancel，不发布该失败事件，也不自动重试工具副作用；Run/lifecycle/final/API 响应按失败处理。独立复核发现 collector JoinError 原默认值会吞失败、原 API ok 会保留 engine ok，主线程已修复为显式 JoinError 失败及 ok && !turn_failed；durability 错误优先返回真实存储原因，同时 timeout 不覆盖该原因。取消是停止内核后续协作的信号，不宣称可撤销已经执行的工具或强制杀死不合作的工具。

现有 API 严格超预算即拒绝，本包未添加附件 blob/chunk。大于 1 MiB 的单 AgentEvent 会导致本次记录失败并停止此轮，不静默截断。SQLite 单事件提交、最多 5 秒锁等待的实际流式吞吐尚未测量；replay page 4 MiB/前端 100 页上限仍生效，超预算历史显示错误。live payload 暂未附 cursor，前端当前执行历史只展示通用事件标签；完整聊天 UI 的断流重建与已显示消息去重另包实现。不能称所有事件旁路、旧历史、GUI 重启、真实模型断流或掉电验收完成。

首次主线程79404全量86测试：85通过、1个新 fixture 错误（fixture已注册环境，.agentz本来存在）。已改断言为 runs.db 未创建且已有环境 DB bytes 不变；未将失败 run 宣称通过。最新统一回归65529在途，最终实际结果后续追加。

最终实际主线程65529：全量 task-state86项全部通过、strict all-target Clippy exit0，writer4与event_replay6测试通过。源码后续 collector 错误分类改动触发同一链的宿主重编译，event_replay6在最终源码上通过；随后86931最新writer4专项再次全部通过（宿主命令exit0）。覆盖提交后独立连接可见、失败callback零次、missing不创建、queued/terminal/stale拒绝、精确dedup及预算回滚。JoinError/API返回/混合超时分类为独立代码复核修复并通过最终宿主编译；未假称已在真实模型或原生UI注入这些故障。
