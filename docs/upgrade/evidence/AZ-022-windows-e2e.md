# AZ-022 AgentZ 层 Windows 子进程树端到端证据

日期：2026-10-12
范围：补齐独立复核指出的缺口（`docs/upgrade/plan.json` AZ-022 reeval：AgentZ 侧无 Windows 子进程树终止测试；无产品代码消费 `recovery_v1`）。

## 1. 新增测试

文件：`crates/agentz-host/tests/tool_recovery_windows.rs`（仅 Windows，`#![cfg(windows)]`）

通过 kernel 的 `ShellTool`（AppContainer 沙箱，`interpreter: "cmd"`）启动真实两层进程树：外层 `cmd /c` 启动内层 `cmd /c for /L ... do @rem AZ022-<marker>`，`marker` 对每个测试唯一。

| 用例 | 触发 | 断言 |
|---|---|---|
| `timed_out_child_tree_is_terminated_and_reported` | `timeout: 8` | `is_error`；`[recovery_v1]` 存在；`outcome: timed_out`；`termination: tree_terminated`；耗时 < 60s；运行期间观察到两层进程树（至少 2 个带标记的 cmd.exe）；结束后 0 个带标记的 cmd.exe 存活 |
| `cancelled_child_tree_is_terminated_and_reported` | 观察到两层进程树之后设置 `cancel`（最长等待 30s） | `is_error`；`[recovery_v1]` 存在；`outcome: cancelled`；`termination: tree_terminated`；取消前已观察到至少 2 个带标记进程；结束后 0 个带标记的 cmd.exe 存活 |

存活判定：`Get-CimInstance Win32_Process` 按 `Name -eq 'cmd.exe'` 且 `CommandLine` 含 `AZ022-<marker>` 计数（不使用 `-Filter`，见第 2 节第 2 点）。查询失败返回 `usize::MAX`，不会被当作“0 存活”。运行期间的观察线程记录观察到的最大数量，要求至少为 2（外层 kernel cmd 与内层循环 cmd），以证明两层进程都确实存在，而非仅有外层。

## 2. 过程中修正的问题（记录以便复核）

1. 首版用 `ping -n N 127.0.0.1` 作为长进程。AppContainer 拒绝 ping（stderr“拒绝访问”，`outcome: tool_failed`），测试没有进入超时/取消路径。改为本地 cmd 循环，不依赖网络。
2. 首版存活查询使用 `-Filter "Name='...'"`。在 PowerShell 参数传递中引号被破坏，`Get-CimInstance` 报错，但 `@(...).Count` 仍输出 0，即假阳性。阳性对照（主动启动一个未终止的 cmd）确认了这一点。修正：去掉 `-Filter`，改为 `Where-Object`，并把失败视为 `usize::MAX`。
3. 修正后的阳性对照（`%TEMP%\az022_control.ps1`）：存活时计数为 1，杀掉后为 0。
4. 新增“运行期间观察到存活”的要求，防止子进程未启动时的空跑误通过。
5. 独立复核（第一轮 PASS）的加固建议已采纳：要求观察到两层进程（而非一个）；取消改为等待观察到进程树之后再发出（取消前有最长 30s 的等待上限），超时测试时长由 5s 提高到 8s，降低采样不足导致的误失败；删除未使用的辅助方法以通过 `-D warnings`。

## 3. 验证结果

- `cargo test -p agentz-host --test tool_recovery_windows --locked`：2 passed（连续两轮）。
- `cargo test -p agentz-host`：kernel_contract 3 passed；tool_recovery_windows 2 passed。
- `cargo clippy -p agentz-host --all-targets --locked`：通过，无告警。
- 依赖变更：`crates/agentz-host/Cargo.toml` 新增 dev-dependencies `serde_json`、`tokio`（均为 workspace 已有依赖）。`Cargo.lock` 在首次运行时已按新依赖更新（之后 `--locked` 可直接通过）。

## 4. 清单

- `docs/upgrade/evidence/AZ-022-source-manifest-r2.json`：9 个文件（内核 5 个 + 补丁 1 个 + 新测试、`kernel_contract.rs`、`agentz-host/Cargo.toml`）。内核文件指纹与 r1 一致。

## 5. 未覆盖范围（不声称已验证）

- UAC / 提权无响应（`ElevationNoResponse`）路径：未在本次测试中触发，属于 ADR-008 的 GUI/UAC 统一豁免范围。
- 其他故障（输出截断在真实子进程上的表现、后置清理失败）：由内核单元测试覆盖，未在本次端到端测试中重复。
- `recovery_v1` 的 JSON 契约文档化：未在本次修改，`AZ-022-contract.md` 为既有契约文件。
- 非 Windows 平台：测试仅在 Windows 编译运行。
