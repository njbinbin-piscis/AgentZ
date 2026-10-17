# AZ-018 独立实现审查记录（r2）

审查者：independent-reviewer-135fa3b0（未参与实现，独立于 owner Codex (事件实现)）
范围：本地单 Agent 的耐久事件（task/run/attempt/environment 标识、递增序号、幂等、断流补齐、不跨任务泄漏）。只读审查，未修改文件，未运行测试。
清单：`docs/upgrade/evidence/AZ-018-implementation-manifest-r2.json`，10 个文件，哈希全部一致。

## 结论

PASS（非阻塞）。范围限定为本地单 Agent 路径；远程、团队、池化与委派运行不在耐久历史内（设计如此）。GUI 与进程重启行为未验证。

## 已核对

- 序号分配：`events.rs:138-140` 与主键 `events.rs:93` 保证每个流严格递增；`run.rs:1455` 使用 IMMEDIATE 事务，写入串行化；`run.rs:1492` 在提交之后才返回 `Event`，游标只在提交后报告。
- 回放：`event_replay.rs:88-101` 与 `events.rs:161-206` 拒绝外来游标并要求序号连续；SQL 按 task、run、attempt、environment 过滤；`event_replay.rs:76` 校验 `run.task_id`。
- 发布顺序：`chat_turn.rs:2582-2615` 的 `record_before_publish` 只在提交之后执行；出错则不发布（2590-2602）。标识来自数据库行，而非 payload。
- 元数据与项目范围：`session_metadata.rs:21-44` 只读且精确匹配会话，失败即关闭；`project_scope.rs:103-170` 按窗口与代际限定句柄。二者不在事件路径上，无跨任务泄漏。

## 非阻塞项

- `chat_turn.rs:2526-2538`：耐久写入器只覆盖非远程、非团队、非池化运行，其他情形发布的事件无游标与标识。冻结证据已记录该范围。
- `chat_turn.rs:2573-2578`、`2597-2602`：`agent_lifecycle: failed` 先于耐久的 Run 终态写入（`run.finish` 约 2829 行）发出。`Done` 与最终生命周期的顺序是正确的。
- `events.rs:101-146`：去重同时比较 `created_ms`。生产者 `durable_stream.rs:50-58` 每次生成新 UUID，幂等只在测试中触发（`run.rs:46-68`），生产路径未覆盖“提交成功但返回错误”的情形。
- `events.rs:138`：序号按 (task, run, attempt, environment) 分配，而非按 run。验收文字写的是“每个 run”，台账应记录这是有意的设计。
- `events.rs:168-171`：每页都扫描整个 attempt，单个缺口会使整个 attempt 不可读。这是失败即关闭，但有性能与可用性代价。
- `external_action.rs:591`：未确认该调用方的事务模式（`recovery.rs:86` 为 IMMEDIATE）。

## 证据限制

候选证据已说明：未做 GUI、真实模型、跨平台与重启验证；无 SLA；远程、团队、池化与直接 sink 路径未覆盖。测试计数由作者给出，未附可复现命令日志。
