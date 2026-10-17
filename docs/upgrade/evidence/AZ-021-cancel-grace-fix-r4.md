# AZ-021 取消宽限修复记录（r4：补齐 code_run 实测与无协作工具负向用例）

对应 `AZ-021-independent-review-r3.md` 之后的补充要求：shell 之外还需 code_run 实测取消，以及不响应取消的工具必须在宽限期后被中断。

## 清单与补丁

- 源清单：`docs/upgrade/evidence/AZ-021-source-manifest-r5.json`（13 个文件，sha256 已按当前文件重算）。
- 补丁：`docs/upgrade/patches/AZ-021-cancel-grace-v0.8.68.patch`（已重新生成）。
  - 验证：对修改前基线快照执行 `git -c core.autocrlf=false apply`，结果与当前 `vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs` 的 SHA256 完全一致（`D0BDB53A…F171A`）。
  - 基线快照是临时副本（位于 %TEMP%，不在仓库内）；其 loop_.rs 哈希 `5b58ea8d…` 与 r3 清单一致。

## 本轮新增测试（均在 `vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs` 测试模块）

1. `real_code_run_cancel_keeps_its_own_result_within_grace`（仅 Windows）
   - 真实 `CodeRunTool`，经 `execute_single_tool` 执行 `for /L %i in (1,1,200000000) do @rem`，1 秒后取消。
   - 断言：结果含 `Sandbox command was cancelled`；不含 `已被用户取消`；耗时 < 20 秒。
   - 结果：`cargo test -p piscis-kernel --locked real_code_run_cancel` 通过，耗时约 1.1 秒。
2. `uncooperative_tool_is_interrupted_after_grace`
   - 不观察 `ctx.cancel` 的 `NonTerminatingTool`（睡眠 600 秒），300 毫秒后取消。
   - 断言：结果含 `已被用户取消`（通用中断路径）；耗时 >= `CANCEL_GRACE_SECS`（证明确实等满宽限期，非立即丢弃）；耗时 < `CANCEL_GRACE_SECS + 5` 秒。
   - 结果：`cargo test -p piscis-kernel --locked uncooperative_tool` 通过，耗时约 8.4 秒。

已有测试保持通过：`cancelled_tool_that_tears_down_keeps_its_own_result`、`real_shell_cancel_keeps_shell_cancel_result_within_grace`。

## 全量验证

- `cargo test -p piscis-kernel --locked`（r4 中间态，仅缺下界断言之前）：393 passed，0 failed，1 ignored；另 4、25 个用例通过。下界断言加入后仅重跑了 `uncooperative_tool`。

## 残余风险（复核非阻塞项，已记录）

- 宽限期到期后，沙箱 worker 的 spawn_blocking 清理可能仍在运行，外层立即返回 `已被用户取消`；receipt 被丢弃，记为失败（tool_completion.rs:136-150）。chat_turn.rs 是否据此处理未在本轮验证。
- 宽限期对所有工具生效（不只 owned-completion 工具），非协作工具最多多等 8 秒。

## 仍未覆盖（不冒称通过）

- 未在真实 GUI（Stop 按钮）中验证；GUI 验收由 ADR-008 批量豁免，不覆盖测试与审查。
- 未验证宽限期内 Windows 作业对象与 ACL 清理的端到端时序（仅有进程级测试）。
- 未验证非 Windows 平台的 code_run 取消（测试以 `cfg(windows)` 限定）。
- `chat_turn.rs` 的 10 秒 drain 上限与本次宽限期的交互，未单独实测。
