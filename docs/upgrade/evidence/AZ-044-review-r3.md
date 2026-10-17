# AZ-044 实现审查记录（r3）

审查者：independent-reviewer-135fa3b0（独立于 owner Codex）
范围：终端生命周期与进程树回收（preparing）。本记录不是正式 Gate 验收。

清单：`docs/upgrade/evidence/AZ-044-implementation-manifest-r3.json`（6 个文件，SHA256 与审查时一致）。其中 `src-tauri/Cargo.toml` 同时包含其他工作留下的未提交改动（版本号 0.9.7 与 `agentz-task-state` 路径依赖），那些改动不属于 AZ-044。

## 演进

- r1 基于 taskkill 与 pid：审查 BLOCK。根进程已退出时，失败的 taskkill 重试会被视为成功而丢弃会话，后代进程可能残留。
- r2 增加失败 pid 记录后重试：审查 BLOCK。根进程死亡后该 pid 的 taskkill 返回 128，会话永久卡住；pid 复用还可能误杀无关进程。
- r3 改用 Windows Job Object（kill-on-close）：teardown 不再依赖 pid。

## 结论

PASS（非阻塞）。

## 已核对（代码阅读）

- `TerminalJob::adopt`：CreateJobObjectW 之后立即包装，之后任一错误路径通过 Drop 关闭句柄；进程句柄在分配成功与失败两条路径上都会关闭。
- `terminate` 调用 TerminateJobObject 并检查返回值；`Drop` 仅关闭一次，类型无 `Clone`。
- 创建失败时 `contain_shell` 失败会杀掉 shell 并返回错误（fail-closed）。
- 会话持有 job 直到从注册表移除；移除时句柄关闭，并因 kill-on-close 回收残留进程。
- 测试：`terminate_reaches_descendants_after_shell_exits` 直接覆盖“根进程已杀、后代仍在”的重试场景；`cargo test -p agentz-desktop --locked terminal_tree` 3 项通过，`commands::ide` 11 项通过。

## 非阻塞项

- 竞态：shell 在 job 分配之前派生的进程不在 job 内。窗口为毫秒级，已写入 `contain_shell` 注释。
- 逃逸：`CREATE_BREAKAWAY_FROM_JOB` 的子进程会离开 job，尚未测试；与旧 `taskkill /T` 相比是回退项，需要后续评估。
- 嵌套限制：若 AgentZ 自身运行在禁止嵌套的 job 中，所有终端创建都会失败。该环境未测试。
- 非 Windows 编译未验证：本机 `cargo check --target x86_64-unknown-linux-gnu` 中止，只做了代码阅读。
- 锁定 registry 在 taskkill 期间持有的问题已不再适用（现在是 TerminateJobObject）；destroy_all 仍在锁内调用，耗时较短，记为观察项。

## 未验证

- 真实 Windows 进程树的桌面级验收（含提权子进程）。
- 非 Windows 平台编译与测试。
