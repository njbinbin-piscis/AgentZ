# AZ-020 独立复核记录（r2：续聊保留开放计划修复后复核）

复核者：independent-reviewer-az020（非 owner Codex (计划实现)）。只读复核，未修改文件，未提交。
范围：`src-tauri/src/tools/durable_plan_todo.rs`（省略 merge 默认合并）、`src-tauri/src/runtime/durable_plan.rs`（替换分支拒绝丢弃未完成条目）及两条新增测试。
清单：`docs/upgrade/evidence/AZ-020-closure-manifest-r2.json`（8 个文件）。
关闭证据：`docs/upgrade/evidence/AZ-020-closure-evidence.md`。

## 结论

PASS。

第一轮复核（BLOCK）指出：`plan_todo` 省略 `merge` 时默认替换，续聊只列部分步骤会静默丢弃未完成项。第二轮复核确认该问题已修复。

## 复核检查

- `durable_plan_todo.rs:452`：`merge` 省略时为 `true`。
- `durable_plan.rs:140-170`：非合并写入若会丢弃 `Pending` 或 `InProgress` 条目，在提交前返回错误。
- `replacement_cannot_drop_open_items_across_restart`：SQLite 重开后，被拒绝的替换返回错误，计划仍为 2 条；合并后两条都保留；取消未完成项后替换被允许。
- `omitted_merge_keeps_open_items_and_explicit_replacement_is_refused`：省略 `merge` 保留未完成项；显式 `merge:false` 丢弃被拒绝，存储计划不变。
- `cargo test -p agentz-desktop --locked --lib durable_plan`：14 passed，0 failed。
- 清单 8 个文件的 sha256 与当前文件一致。

## 残余风险

- 内核 `vendor/.../tools/plan_todo.rs` 的 schema 描述仍写着“false 或省略时替换”，与宿主默认值不一致。宿主以守卫保证不丢数据，但模型可能被误导。内核描述的修正属于 AZ-026（内核契约版本），记入该任务。
- 证据文档中的全库结果（267 passed，1 个既有 `HOME` 环境失败）未由复核者重跑。
- “重启”仅指同一进程内重开 SQLite，不是真实应用重启。
