# AZ-024 收口证据：任务整体资源与费用预算（宿主接入）

- 任务：AZ-024，实现任务整体资源与费用预算
- 源文件指纹：`docs/upgrade/evidence/AZ-024-closure-manifest.json`（revision `closure-r7`，10 个文件）
- 独立审查历史：r1 FAIL → r2 FAIL → r3 FAIL → r4 FAIL → r5 FAIL → r6 PASS（审查人 `independent-reviewer-az024-r6`，记录见 `AZ-024-independent-review-r6.md`）
- r7（覆盖辅助调用接入后的字节，10 个文件）：PASS（审查人 `independent-reviewer-az024-r7`，记录见 `AZ-024-independent-review-r7.md`）；R7-1 与 R7-2 的措辞已在本文件第 4 节修正

## 1. 验收条件对照

验收条件：「预算耗尽进入解释性停止；续跑不重置全任务预算；未知价格明示」。

| 条件 | 实现 | 证据 |
| --- | --- | --- |
| 预算耗尽进入解释性停止 | 宿主在模型调用前预留额度；超限时返回以 `budget denied:` 开头的错误，调用不会到达 provider，错误作为 agent 错误进入本轮。 | 宿主测试 `exhausted_limit_stops_next_call_and_raised_limit_resumes_without_reset`、`exhausted_active_time_refuses_before_the_provider`；核心 `budget` 专项 25 项 |
| 续跑不重置全任务预算 | 额度挂在任务（task）上，不挂在 attempt 上；调整限额走新 revision，已用量继续累计。 | 宿主测试断言 revision 2 后 `known_input_tokens` 仍为 2；核心 `revision_lifts_overshoot_block_and_refuses_stale_or_skipped_revisions` |
| 未知价格明示 | 宿主不提供价格来源，cost 为 `None`，记录计入快照的 `unknown_price_records`，不会被当作 0 费用。 | 核心测试断言 `unknown_price_records == 4` |

补充约束：
- 默认不限额（unlimited）：`unlimited_default_records_usage_and_never_denies`、`dropped_calls_are_settled_and_do_not_hold_slots_or_budget`、核心 `unlimited_policy_is_not_blocked_by_an_overshoot`。
- 存储错误按已确认的 fail-open 处理：`missing_store_fails_open_and_reaches_the_provider`。
- 限额修改于下一轮生效：`active_time_limit_admits_calls_with_a_bounded_reservation`。

## 2. 实现范围

核心（`crates/agentz-task-state`）
- `budget.rs`：`revise_in_tx` 原子推进 revision；快照含 `unknown_price_records`。
- `budget_reservation.rs`：`ReservationStore::revise_policy`；`reconcile_orphans`（只处理非 `running` run 的孤儿记录：未启动则释放，已启动则按预留估计值结算）；`check_budget` 对无限额策略直接放行，超额阻断仅在对应 revision 且限额生效时适用。

宿主（`src-tauri`）
- `runtime/task_budget.rs`：`TaskBudgetLimits`（全局文件 `task_budget.json`）；`ensure_policy`；`BudgetScope`（父 turn 与子代理共享）；`BudgetedClient`（reserve → start → settle；`Settlement` 守卫保证被中止的调用也会结算；reserve/start 失败后回读记录并撤销或结算）；`PENDING` 队列（结算/释放失败后在下次 `begin` 前重试）。
- `commands/chat_turn.rs`：主 turn 注册工具时传入 `BudgetScope`，`early_binding` 产生后 `bind`，再包装主 client；子代理（`delegate`、`call_fish`）共享同一作用域；自动标题（后台 spawn）与 post-turn hooks 也接收同一作用域。
- `commands/post_turn.rs`、`commands/skill_review.rs`：记忆抽取与后台技能复盘的 client 经 `budget.guard` 包装。
- `commands/gateway.rs`：IM 回合调用 post-turn hooks 时传入未绑定作用域（即不计入任何任务预算，见残留项）。
- `tools/delegate.rs`、`tools/call_fish.rs`：新增 `budget` 字段并透传。

## 3. 验证命令与结果

| 命令 | 结果 |
| --- | --- |
| `cargo test -p agentz-task-state --lib` | 150 passed, 0 failed（r4 之后核心无改动，结果仍有效） |
| `cargo test -p agentz-task-state --lib budget` | 25 passed, 0 failed |
| `cargo clippy -p agentz-task-state --all-targets -- -D warnings` | exit 0 |
| `cargo test -p agentz-desktop --lib task_budget` | 10 passed, 0 failed |
| `cargo clippy -p agentz-desktop --lib --tests` | 本次改动的宿主文件（`task_budget.rs`、`chat_turn.rs`、`delegate.rs`、`call_fish.rs`、`post_turn.rs`、`skill_review.rs`、`gateway.rs`）无报警；其余报错为未改动文件的既有问题：`commands/symbols.rs`、`runtime/approval.rs`、`skills/service.rs`、`tools/durable_plan_todo.rs`、`tools/remote_fs.rs` |

## 4. 残留项（已披露，需 owner 确认接受）

按已确认的归属规则（“仅改造任务绑定类调用，全局维护类调用作为明确残留”）：

1. **交付后的 post-turn 调用当前不计入任务用量预算**：自动标题、记忆抽取、技能复盘在 `run.finish` 之后才 spawn，而 `reserve` 要求 attempt 处于 `running` 且 `delivery == not_ready`，因此这些调用的预留会被核心拒绝，调用按 fail-open 放行，不计入任务预算（仅记录告警）。例外：若任务的 active 时长额度已耗尽，这些调用会以 `budget denied:` 被拒绝，而不是放行。也就是说，辅助调用在当前时序下**没有被计入任务用量预算**，接入只是为它们预留了接入点。若要真正覆盖这些调用，需要单独设计（例如在交付前完成辅助调用，或允许交付后的结算账本）。
2. **全局维护调用未纳入任务预算**：curator、memory consolidate、inline edit。它们不属于任何任务，按规则保持在任务预算之外。
3. **IM/Gateway 回合未纳入任务预算**：IM 回合走内核 `run_piscis_turn`，只绑定 IM 会话，没有任务 binding，主回合与其 post-turn 调用都不计入任何任务。Koi 相关调用未在 src-tauri 中找到对应的 client 构造点，未单独核实。
4. 限额配置仅通过全局文件 `task_budget.json` 生效（下一轮起按 revision 应用）；尚无设置界面，也没有 Tauri 配置命令。
5. 费用限额未启用：没有价格来源（`max_cost: None`），未知价格记录在快照中，尚未在 UI 中展示。
6. 单次调用的 active 时长预留为 `min(剩余额度, 600000 ms)`。并发调用的预留之和超过剩余额度时会被拒绝（N2）。单次超过 10 分钟的调用结算后会超出预留，在该 revision 上触发超额阻断，直到提高限额（该阻断设计已由用户确认）。
7. 结算或释放因存储错误失败时进入进程内队列，下次 `begin` 前重试；队列无上限，永久失败的条目会每次告警（R4-2、R4-4、R6-1）。进程退出后未落地的记录，在下次启动且 run 非 `running` 时按保守规则恢复（依赖 AZ-019 的启动对账）。
8. 崩溃恢复的调用按估计值或 10 分钟上限计入（R3-2，仅影响有限额任务）。
9. 存储连续两次故障、且提交后又无法读回记录时，记录可能悬挂到重启（R6 案例 e，P3）。
10. 拒绝文案为英文，未本地化；`task_budget.json` 损坏时回退为不限额并记录告警。
11. 并发恢复路径与 `start` Replay 分支没有可执行的故障注入测试（R5-1 修复经代码阅读与审查确认）。

## 5. 结论建议

- 三项验收条件在代码与测试层面满足（前提是调用发生在运行中的 attempt 内）。
- 独立审查 r6 为 PASS；r7 复审针对辅助调用接入。
- 残留项 1、2、3 决定了“任务预算”的真实覆盖范围，应由 owner 显式接受后再 `done`。
