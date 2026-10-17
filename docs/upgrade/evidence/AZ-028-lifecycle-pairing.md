# AZ-028 执行生命周期配对（cycle 013）

日期：2026-10-05；执行者 Codex。主实现仍为 AZ-028。

已核对进度、plan 及 cycle 012 证据，未发现仓库或祖先 AGENTS.md，开始时无 cargo/rustc。未覆盖其他任务修改。

固定内核 v0.8.68 的 loop_.rs 在 before_tool 返回 Deny 后仍调用 after_tool。宿主原来无条件调用 journal.after_tool，且仅以 tool_use_id 移除 claimed 审批，不能区分同 ID 的不同工具调用。

现在以 session、真实 tool_use_id、工具名、输入和工作根的 SHA 身份记录已进入 delegate.before_tool 的调用。after_tool 只有匹配记录时才进入 delegate，记录一次消费；提前、未匹配、重复或授权阶段拒绝的回调不进入 delegate.after_tool。claimed 记录也独立保存完整调用身份，其他工具复用 ID 不会结束原审批。delegate 自身拒绝时记录 failed，实际执行错误保持 unknown。

## 验证与复盘

首轮宿主集成 5/6，通过计数用例但同 ID 不同工具仍误将 claimed 写为 executed。新增测试捕获该问题；本轮保留日志 `.cache/upgrade-approval-hook-cycle-013.log`。首轮构建启动后补上的完整 claimed 身份修复尚未进入该编译快照。

随后停止修改并重新验证最新源码：6/6 通过，编译 2m44s，测试 0.19s，日志 `.cache/upgrade-approval-hook-cycle-013-final.log`。

新增计数 delegate 用例验证拒绝/提前回调零次后处理、正常配对一次、重复回调不重复执行。扩展实际 SQLite 审批用例验证同 ID 不同工具结束、换任务、改参数均不消费原 claimed；真正匹配回调才 executed。沿用实际 journal、取消、后台拒绝及数据库失效用例。测试手工驱动真实 AgentHooks，没有模型、原生窗口或 OS sandbox 验收。

git diff --check 通过（已有 Windows 行尾提示）。仅 Rust hook 与集成夹具变更，未重跑前端；台账校验另行记录。AZ-028 保持 doing：direct 根/环境一致性、等待期间任务绑定变化及原生审批 surface 验收仍待完成。AZ-079 未修复，未更改固定内核依赖或发布产品。
