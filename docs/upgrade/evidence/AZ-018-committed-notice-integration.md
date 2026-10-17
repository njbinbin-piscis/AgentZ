# AZ-018 持久通知高水位与被动回放集成

2026-10-08。两个 Codex 子代理独占纯通知 reducer 与面板消费；主线程独立核对源代码及回归。未重置用户工作，未发布。

agent_event_committed 通知只作为历史补齐提示，不直接确认游标，不追加 active 聊天文本，不执行权限或恢复模型。严格匹配 session 与完整 task/environment/Run/Attempt 身份、agent.event 命名空间和安全整数；重复或乱序通知合并最大高水位。旧流忽略，坏匹配流明确失败。

面板先 await listener 注册再 initial discovery，防止注册盲窗；异步迟到注册在卸载时 dispose。scope/session generation 隔离旧请求。200ms聚合、一次在途读取及pending补读；初始未知流通知缓存最多32条/累计8MiB，单条1MiB，溢出不缓存payload但保持补读请求。回放仍从0完整读取，连续sequence和精确事件dedup沿用collector；累计5000事件/8MiB预算，失败不显示部分内容。未达到已知高水位明确报错，避免无限自旋。

独立主线程复核限制：本包从0重读而非增量游标缓存，长活跃流的CPU/SQLite吞吐尚未测量；不能称完整消息历史恢复、掉电、GUI或真实模型断流验收。仅当前Attempt显示，旧Attempt仍留在库；旧流通知不改变当前显示。通知无客户端ack，生命周期终态仍依赖busy/manual/focus刷新。1MiB通知信封预算略严于payload预算，接近边界可能明确拒绝；后续预算一致性测试补齐。未知通知类型不会触发工具或审批。

实际主线程67836：31项前端目标测试通过（notice5/projection6/collector6/panel14）；最终typecheck/ESLint结果后续追加。代理最终14组件测试/typecheck/限定ESLint已通过，主线程不替代记录原生验收。无Rust修改，本包不重复Cargo；上一宿主writer4通过证据沿用。正式任务/Gate不提升。

主线程67836最终exit0：31目标测试、typecheck及限定ESLint全部通过。原生GUI/真实模型/吞吐仍未执行。
