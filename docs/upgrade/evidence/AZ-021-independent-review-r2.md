# AZ-021 独立实现审查记录（r2）

审查者：independent-reviewer-135fa3b0（未参与实现，独立于 owner Codex (取消闭环)）
范围：排队取消注册、执行取消等待工具与子进程终止确认、cancelling → cancelled 的 UI 与宿主状态。只读审查，未修改文件，未运行全量测试。
清单：`docs/upgrade/evidence/AZ-021-source-manifest.json`，12 个源文件哈希与审查时一致。

## 结论

BLOCK（窄）。没有发现会误报 `cancelled` 的路径，但 shell 与 code_run 的执行中取消可能以 `interrupted` 结束，不满足“cancelling → cancelled”的验收。

## 阻塞项

- `vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs:2383-2422`：200 ms 的 `cancel_watcher` 与工具 future 竞争。沙箱工具自己的取消路径（`proc_windows_sandbox.rs:326-380`，等待 job 清理与 ACL 回收）可能输掉这场竞争。watcher 先完成时，`call_with_completion` 的 receipt 被丢弃并记为失败（`tool_completion.rs`，`OwnedReceipt::drop`），宿主得到 `Failed`，`chat_turn.rs:2826-2835` 报告 `interrupted`。
- 安全性成立：`drain` 仍会等待 worker 活动计数归零或超时，不会提前释放。问题在于标签不确定：shell 与 code_run 取消的结果可能不稳定。该结论由代码推理得出，未运行复现。
- 修复方向：对 owned-completion 工具给取消后一段宽限期再丢弃 future，或对这些工具去掉 watcher；然后补一条真实 shell 取消夹具，断言结果为 `cancelled`。

## 已核对通过

- 排队取消：`chat.rs` 约 268-300 行、`chat_cancel.rs:97-108` 在获取许可后重新检查取消；`run.rs:1335-1350` 使用比较并交换转换，已取消的排队 run 不会启动。
- 取消后不再发起新工具：`loop_.rs:2362-2373`（启动前检查）、`tool_completion.rs:57`（已取消或已封口时拒绝 register）、`loop_.rs:3880-3910`（批次检查）、`loop_.rs:2226-2243`（权限拒绝）。
- 单次快照：`chat_turn.rs:2784-2796` 只读取一次 `cancellation_requested`，同时用于 `finish` 与 `lifecycle_state`；未确认的 drain 转为 `interrupted`。
- 宿主 drain：`cancel_completion.rs:51-70` 与 `tool_completion.rs` 仅在所有 receipt 完成时返回 `Complete`；`Pending` 与 `Failed` 不会产生 `cancelled`。

## 非阻塞项

- `chat_turn.rs:2789-2791`：10 秒上限会把 `Pending` 转为 `interrupted`，此时 worker 可能仍在运行，但门已释放。证据文档已说明该限制。
- `elevate.rs:629`、`user_tool.rs:179` 在 owned receipt 之外派生子进程，只有 invocation 级完成覆盖。
- `proc_windows_sandbox.rs` 依赖 `wait_for_job_empty` 与 `workspace.finish`，二者不在清单内，本次未读取。

## 证据限制

证据文档已如实说明：未运行 Cargo 或 GUI；真实 GUI Stop、Windows 后代/管道/ACL 回收、正式 done 状态均未验证；通过计数（4、3、2）未给出可复现命令。

## 对 AZ-044 的影响

AZ-044 依赖 AZ-021。在 AZ-021 通过独立审查之前，AZ-044 的实现审查无法写入台账（校验会检查上游的独立审查）。
