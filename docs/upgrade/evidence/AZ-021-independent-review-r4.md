# AZ-021 独立实现审查记录（r4，加入真实 shell 夹具后复核）

审查者：independent-reviewer-135fa3b0（未参与实现，独立于 owner Codex (取消闭环)）
范围：排队取消与执行中取消的宽限收尾，以及真实 shell 取消夹具。只读复核，未修改文件。
清单：`docs/upgrade/evidence/AZ-021-source-manifest-r3.json`（13 个文件，哈希全部一致）。
修复与夹具说明：`docs/upgrade/evidence/AZ-021-cancel-grace-fix.md`；补丁 `docs/upgrade/patches/AZ-021-cancel-grace-v0.8.68.patch`（在修改前基线上可应用，并复现 vendor 文件）。

## 结论

PASS（非阻塞）。r2 的阻塞（watcher 与工具 future 竞争）已修复；真实 shell 夹具经代码阅读确认走的是真实取消分支（`shell.rs:275` 仅在 `output.cancelled` 时产生 “Command was cancelled”，且先于 120 秒超时判断）。

本记录不等于 AZ-021 完成：台账的 done 仍需 code_run 取消夹具、负向用例、沙箱收尾耗时测量、真实 GUI Stop 与 Windows 后代/管道/ACL 回收。

## 测试

- `cargo test -p piscis-kernel --locked cancelled_tool_that_tears_down`：1 passed（作者本地运行）。
- `cargo test -p piscis-kernel --locked real_shell_cancel`：1 passed（作者本地运行）。

## 非阻塞项

- 真实 shell 夹具的断言依赖竞态：旧 watcher 路径理论上在约 1 秒获胜，但时序不完全确定，因此它不是确定性的回归证明。
- 夹具超时 120 秒（工具超时），取消失败时最坏约 120 秒才以超时结束，没有外层的显式守卫。
- 夹具依赖可用的 AppContainer，仅 Windows；沙箱建立失败时会以恢复错误失败。
- 宽限 8 秒与沙箱最坏收尾时间之间没有余量（见 r2 记录）。
