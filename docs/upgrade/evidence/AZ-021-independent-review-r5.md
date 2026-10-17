# AZ-021 独立复核记录（r5：code_run 实测、负向夹具、宽限期下界断言）

复核者：independent-reviewer-135fa3b0（非 owner Codex (取消闭环)）。未修改文件，未提交。
范围：排队与执行中取消的宽限收尾；真实 shell 与真实 code_run 取消夹具；无协作工具在宽限期后中断的负向夹具。不含 GUI Stop 与非 Windows 验证。
清单：`docs/upgrade/evidence/AZ-021-source-manifest-r5.json`（13 个文件）。
修复记录：`docs/upgrade/evidence/AZ-021-cancel-grace-fix-r4.md`。补丁：`docs/upgrade/patches/AZ-021-cancel-grace-v0.8.68.patch`。

## 结论

PASS（第一轮复核发现 1 项非阻塞建议：负向测试缺下界断言，已采纳；第二轮复核发现修复记录中 loop_.rs 哈希过期，已修正后 PASS）。

## 复核发现（非阻塞，已记录）

1. 宽限期到期后，沙箱 worker 的清理可能仍在运行；外层返回 `已被用户取消`，receipt 记为失败（tool_completion.rs:136-150）。chat_turn.rs 是否据此处理未验证。
2. 宽限期对所有工具生效，非协作工具最多多等 8 秒（设计取舍，已在修复记录中说明）。

## 复核检查

- `loop_.rs` 的 `real_code_run_cancel_keeps_its_own_result_within_grace` 经 `CodeRunTool` 与 `execute_single_tool` 实际执行；断言 `Sandbox command was cancelled` 只能由 code_run 取消分支产生，`已被用户取消` 的排除断言能区分 watcher 误报；耗时上限排除 120 秒超时路径。
- `uncooperative_tool_is_interrupted_after_grace`：断言 `已被用户取消`，耗时 >= `CANCEL_GRACE_SECS`（确认等满宽限期）且 < `CANCEL_GRACE_SECS + 5`；不挂起。
- `cargo test -p piscis-kernel --locked real_code_run_cancel`：1 passed（约 1.1 秒）。
- `cargo test -p piscis-kernel --locked uncooperative_tool`：1 passed（约 8.4 秒）。
- `AZ-021-source-manifest-r5.json` 的 13 个 sha256 与当前文件逐一一致。
- `AZ-021-source-manifest-r3.json` 的 sha256 为 `7cb78000…8820ca`，与 ledger 中已记录的 r3 引用一致。
- `AZ-021-cancel-grace-fix-r4.md` 的 loop_.rs 哈希 `D0BDB53A…F171A` 与当前文件一致。

## 未覆盖（不冒称通过）

- 全量套件仅在下界断言加入前运行（393 passed）；下界断言加入后只重跑了 `uncooperative_tool`。
- 修改前基线快照位于临时目录，不在仓库内，本次复核未重新计算其哈希（作者在本轮已用其验证补丁字节一致）。
- 未验证 GUI Stop、Windows Job Object 与 ACL 清理的端到端时序、非 Windows 平台。
