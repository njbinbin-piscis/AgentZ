# AZ-020 关闭证据（r2）：普通续聊保留开放计划

范围：普通任务续聊保留开放计划；新目标显式结束或替换计划。
验收：重启与续聊不清除未完成项；明确取消可以结束计划。
验证：clearPlan 与重启恢复集成案例（实际对应的测试见下表；台账中的 "clearPlan" 指 `chat_plan_apply` 的显式取消/替换路径与 `clear_plan` 请求参数的拒绝语义）。
清单：`docs/upgrade/evidence/AZ-020-closure-manifest-r2.json`（8 个文件）。

## 本轮修复（来自独立复核 BLOCK）

独立复核发现：`plan_todo` 工具在省略 `merge` 时默认 `false`（替换），续聊轮次只列出部分步骤会静默丢弃未完成项。修复：

1. `src-tauri/src/tools/durable_plan_todo.rs`：省略 `merge` 时默认合并（`unwrap_or(true)`）。
2. `src-tauri/src/runtime/durable_plan.rs`：`DurablePlanSession::write` 的替换分支若会丢弃仍为 `Pending` 或 `InProgress` 的条目，直接返回错误，计划不变；需要先合并或把这些条目标为 `cancelled`。

## 代码现状

- `src-tauri/src/commands/chat.rs:95-99`：`clear_plan` 省略或 `false` 时保留计划；显式 `true` 被拒绝。
- `crates/agentz-task-state/src/plan.rs`：`continue_plan` 只读；显式清除/替换保留历史，续聊不会重开已取消的计划。
- `src-tauri/src/commands/chat_plan.rs`：`chat_plan_apply` 是用户显式结束或替换计划的路径（`PlanIntent::Cancel` / `Replace`），校验期望 id、修订号与活动 Run 门禁。
- 内核 `vendor/.../tools/plan_todo.rs` 的 schema 描述仍写着“变更较大时优先替换整个列表”，与宿主默认值不一致；宿主以守卫拒绝丢弃未完成项。内核描述的修正属于 AZ-026（内核契约版本），记为残余风险。

## 测试结果（本轮运行）

`cargo test -p agentz-desktop --locked --lib durable_plan`：14 passed，0 failed。其中新增：

- `runtime::durable_plan::tests::replacement_cannot_drop_open_items_across_restart`：两步计划写入后重开 SQLite，续聊只列一步时替换被拒且计划不变；合并后另一条未完成项保留；取消后才允许替换。
- `tools::durable_plan_todo::tests::omitted_merge_keeps_open_items_and_explicit_replacement_is_refused`：省略 `merge` 的续聊保留未完成项；显式 `merge=false` 丢弃未完成项被拒且无副作用。

`cargo test -p agentz-task-state --locked plan`：18 passed，0 failed。包括 `restart_and_continuation_preserve_identity_revision_and_items`、`explicit_clear_preserves_steps_and_never_reopens_on_continuation`、`completed_plan_can_be_explicitly_extended_cleared_and_replaced`。

`cargo test -p agentz-desktop --locked --lib continue`：`commands::chat::continuation_intent_tests::omitted_and_explicit_continue_preserve_plan_but_legacy_clear_is_rejected` passed。

全库 `cargo test -p agentz-desktop --lib`：267 passed，1 failed。失败项为既有 `commands::agents::smoke_tests::smoke_sync_seeded_agents_to_kois`（读取 `HOME` 环境变量，Windows 未设置），与本次无关。

## 验收对照

| 验收条件 | 证据 |
| --- | --- |
| 重启不清除未完成项 | `replacement_cannot_drop_open_items_across_restart`（重开 SQLite 后续聊）；`restart_and_continuation_preserve_identity_revision_and_items`；`sqlite_restart_stale_revision_and_trigger_failure_preserve_projection` |
| 续聊（省略 merge）不清除未完成项 | `omitted_merge_keeps_open_items_and_explicit_replacement_is_refused` |
| 明确取消可以结束计划 | `explicit_clear_preserves_steps_and_never_reopens_on_continuation`；`cancelled_plan_cannot_be_reopened_by_tool`；`mixed_done_cancelled_and_all_cancelled_items_are_completed_without_cancelling_plan` |

## 限制（不冒称通过）

- “重启”是同一进程内关闭并重新打开 SQLite，不是真实应用重启的端到端测试。
- 没有经过 `chat_send` 的完整续聊集成测试，续聊语义由工具层与会话层测试覆盖。
- 前端 `clearPlan` 的 UI 行为未重跑 vitest。
- 没有 GUI 验证；按 ADR-008 豁免 GUI。
- 内核 schema 描述与宿主默认值不一致（见上）。
