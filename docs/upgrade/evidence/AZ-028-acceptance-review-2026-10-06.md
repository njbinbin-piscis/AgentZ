# AZ-028 验收复核（2026-10-06）

复核范围严格按台账原始验收：参数变化、过期或换任务后重新审批；重启后不得保留失效授权。GUI 点击验收不属于该条验收标准。本次未运行原生桌面 GUI，也不将其记为通过。

## 当前源码验证

- `cargo test --manifest-path src-tauri/Cargo.toml --test approval_execution --locked`：17 passed，0 failed，1 ignored。忽略项 `candidate_repeated_edits_undo_to_turn_start_through_host_hooks` 明确需要 AZ-079 candidate kernel；固定 v0.8.68 保留已知 Undo 缺陷。测试覆盖参数/文件范围、跨任务、任务与环境切换、等待期间 branch 变化、取消后不可重放、拒绝及后台调用在进入 delegate 前被拦截等宿主执行路径。
- `cargo test -p agentz-task-state approval --locked`：18 passed，0 failed。明确包含 `restart_expires_pending_and_approved_and_marks_claimed_unknown`、`changed_parameters_task_environment_account_or_target_revokes_old_approval`、`expiry_clock_regression_denial_and_revocation_never_allow_claim`、`expired_and_previous_boot_requests_cannot_be_delivered`，以及摘要规范化、原子 claim、审计写入失败关闭和取消撤权测试。
- 对应输出在本轮终端记录；执行时退出码均为 0。

历史实现及 SQLite 崩溃/审计/并发语义详见 `AZ-028-durable-core.md`；实际主执行器 hook、等待时任务/环境绑定和直接 CodeZ branch 变更另见本计划其他 `AZ-028-*` 证据。

## 结论与边界

台账列明的持久审批契约通过当前 host/store 自动化验收，足以关闭 AZ-028。真实桌面 native click、WebView 交互和生产打包未由本次测试覆盖；相关 GUI 验收仍未验证。AZ-034 的 OS 级目录/进程/网络约束仍是独立未完成任务，工具注册表裁剪与 worktree 均不构成沙箱声明。
