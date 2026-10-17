# AZ-018 完成证据

任务：耐久事件携带 task/run/attempt/environment 标识及递增序号；重复事件幂等；断流可补齐；事件不泄漏到其他任务。

## 验证命令与结果（2026-10-11，作者本地运行）

- `cargo test -p agentz-task-state --locked`：149 项通过（库 147 + 集成 2），0 失败。覆盖：
  - `events.rs:567` `dedup_paging_and_cross_attempt_cursor_rejection`：去重、分页、跨 attempt 游标拒绝；
  - `run.rs:36` `agent_events_commit_reopen_deduplicate_exactly_and_preserve_run_timestamp`：提交、重开、精确去重；
  - `events.rs:527` `rollback_preserves_state_event_atomicity_and_committed_replay`：回滚原子性；
  - `lib.rs:387` `rejects_cross_project_records_and_future_schema`：跨项目拒绝。
- `cargo test -p agentz-desktop --locked durable_stream`：4 通过，1 忽略（大规模计时测试，按设计需 `--ignored` 单独运行）。覆盖提交后回调、失败追加不发布、重开后身份与游标一致。
- `cargo test -p agentz-desktop --locked event_replay`：6 通过（回放分页、外来游标拒绝、断流补齐路径）。

## 独立实现审查

`docs/upgrade/evidence/AZ-018-independent-review-r2.md`：PASS，清单 `AZ-018-implementation-manifest-r2.json` 10/10 哈希一致。

## 验收条件逐项

1. 重复事件幂等：`events.rs` 去重与 `run.rs:36` 精确去重测试通过。
2. 断流可补齐：`event_replay` 与分页重放测试通过。
3. 事件不泄漏到其他任务：`events.rs:567`、`lib.rs:387`、`event_replay.rs` 外来游标拒绝测试通过。

## 限制（未验证，不计入本任务）

- 远程、团队、池化与委派运行不在耐久历史内（设计如此，`chat_turn.rs:2526-2538`）。
- GUI 重连与进程重启的端到端行为未验证（按 ADR-008 豁免 GUI 验收）。
- 性能（千级增量计时测试）未在本次运行中执行。
- 生产幂等未覆盖“提交成功后返回错误”的情形（复核非阻塞项）。
