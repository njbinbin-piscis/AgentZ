# AZ-021 独立实现审查记录（r3，修复后复核）

审查者：independent-reviewer-135fa3b0（未参与实现，独立于 owner Codex (取消闭环)）
范围：排队取消、执行中取消的宽限收尾、cancelling → cancelled 路径。只读复核，未修改文件。
清单：`docs/upgrade/evidence/AZ-021-source-manifest-r2.json`（13 个文件，哈希全部一致）。
修复说明：`docs/upgrade/evidence/AZ-021-cancel-grace-fix.md`；补丁 `docs/upgrade/patches/AZ-021-cancel-grace-v0.8.68.patch`。

## 结论

PASS（非阻塞）。前次 r2 的阻塞（watcher 与工具 future 竞争，shell/code_run 可能记为 interrupted）已解决。

注意：本审查不等于 AZ-021 完成。任务必须在真实 shell 取消夹具断言 `cancelled` 通过之后，才能标记 done。

## 已核对

- 宽限逻辑正确：工具在宽限内自行收尾则返回其自身结果；否则丢弃 future，结果为“已被用户取消”（receipt 记为失败，宿主报告 interrupted）。外层 120 秒 TOOL_TIMEOUT 仍映射为超时提示。
- 类型与生命周期：select 两个分支都产出 `Option<Result<Result<ToolResult>, Elapsed>>`；`&mut Pin<Box<Timeout>>` 是合法 future；Ready 之后不再轮询，无 future 泄漏。
- 取消后不再发起新工具调用：启动前检查、批次检查与 `register` 拒绝已取消上下文仍在该代码之前生效。
- 测试：`cargo test -p piscis-kernel --locked cancelled_tool_that_tears_down` → 1 passed。补丁在修改前基线上可应用，并逐字节复现 vendor 文件。

## 非阻塞项

- 非协作工具需要等满 8 秒才报告取消（原先约 200 毫秒）。可考虑对非 owned 工具缩短宽限，或在文档中说明该延迟。
- 宽限 8 秒短于沙箱最坏收尾时间（根进程等待 5 秒 + job 清空等待 5 秒，外加 ACL 回收）。慢收尾会记为 interrupted，方向是安全的。宿主 drain 上限为 10 秒，宽限与 drain 之间的余量未文档化，端到端总预算约为 8.2 + 10 秒。
- 测试只有正向用例。缺少负向用例（工具永不结束 → 宽限后 interrupted），也缺少真实 shell 取消夹具断言 `cancelled`。
- “旧代码在约 400 毫秒时 watcher 获胜、断言失败”是推理，未在基线副本上实测。

## 对 AZ-044 的影响

AZ-044 依赖 AZ-021。本记录提供的独立实现审查满足台账对上游独立审查的要求，AZ-044 的审查可以写入。
