# AZ-021 取消宽限修复记录（含真实 shell 夹具）

对应独立审查 `AZ-021-independent-review-r2.md` 的阻塞项：内核 `loop_.rs` 的 200 ms 取消 watcher 与工具 future 竞争，shell/code_run 取消可能被记为 `interrupted`。

## 改动

- 文件：`vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs`（LF 换行，与 upstream 基线一致）。
- 新增常量 `CANCEL_GRACE_SECS = 8`。
- `execute_single_tool` 的工具执行：工具 future 固定为 `Pin<Box<timeout(...)>>`；取消 watcher 触发后，给工具最多 8 秒自行收尾（shell/code_run 通过共享的 `ctx.cancel` 终止各自的 Job Object），超时才丢弃 future 并返回“已被用户取消”。
- 补丁：`docs/upgrade/patches/AZ-021-cancel-grace-v0.8.68.patch`（3 个 hunk）。在修改前基线副本上 `git -c core.autocrlf=false apply` 通过，并逐字节复现当前 vendor 文件。
- 清单：`docs/upgrade/evidence/AZ-021-source-manifest-r3.json`（13 个文件，含补丁）。

## 测试

1. `agent::loop_::tests::cancelled_tool_that_tears_down_keeps_its_own_result`：协作式夹具工具在取消后延迟 600 ms 收尾并返回 `torn down`；取消在 300 ms 发出。断言结果为工具自身返回值。`cargo test -p piscis-kernel --locked cancelled_tool_that_tears_down`：1 passed。
2. `agent::loop_::tests::real_shell_cancel_keeps_shell_cancel_result_within_grace`（仅 Windows）：注册真实 `ShellTool`，运行 `for /L` 空循环（约 2 亿次，无法自行结束），1 秒后设置取消。断言结果包含 shell 自己的 `Command was cancelled`，不包含“已被用户取消”，耗时小于 20 秒。`cargo test -p piscis-kernel --locked real_shell_cancel`：1 passed。

夹具过程中的一次失误：最初用 `ping -n 60 127.0.0.1`，在 AppContainer 内约 0.3 秒即以 exit 1 失败（ICMP 不可用），未进入取消路径，因此换成了 CPU 循环命令。

## 未覆盖与限制

- 未在修改前的旧代码上运行真实 shell 夹具以证明它会失败；按代码推理，旧代码的 watcher 在约 1 秒后获胜，返回“已被用户取消”，断言会失败。这是推理，未实测。
- 未测量沙箱收尾耗时；宽限 8 秒与沙箱最坏收尾（根进程等待 5 秒 + job 清空等待 5 秒 + ACL 回收）之间没有余量。超过宽限的收尾仍记为 `interrupted`（保守方向）。
- 未覆盖负向用例（工具永不结束 → 宽限后 interrupted）。
- 宿主侧 `chat_turn.rs` 10 秒 drain 与宽限的端到端预算未验证。
- code_run 的真实取消夹具未加；真实 GUI Stop、Windows 后代/管道/ACL 回收、正式 done 状态均未验证。
- AZ-021 不因本记录标记为 done：台账的 done 还需要上述未验证项与 Gate 验收。
