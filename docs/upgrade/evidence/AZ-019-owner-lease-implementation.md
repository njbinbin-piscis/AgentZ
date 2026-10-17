# AZ-019 启动对账：宿主 owner 租约实现记录（r1）

任务：进程启动检查未终结 Run、环境和存活进程。目标：僵尸 running 标为 interrupted 或确认仍存活；不默默重执行工具。

## 设计

- 每个 Run attempt 在排队（`queue_bound`）或开始执行（`begin_bound`）时取得 owner 租约：`<规范化项目根>/.agentz/owners/<run>.<attempt>.lease`，持有独占 OS 锁（Windows：`LockFileEx` 锁定第 0 字节，`FAIL_IMMEDIATELY`；Unix：`flock(LOCK_EX|LOCK_NB)`），锁文件内写入完整 `Binding` 与 pid。
- 进程死亡时 OS 释放锁，因此"锁可取得"是宿主可信的 owner-gone 证据；"锁被占用"即 owner 存活；"无租约文件"无 owner 证据，判为 unverifiable 并保留。
- 对账（`reconcile_project`）：列出 `queued/running/cancelling` 的 Run（新增 `RunStore::active_runs`），逐个探测租约。锁可取得时持锁调用领域 `RecoveryStore::reconcile(ConfirmedGone)`，由领域层在同一 IMMEDIATE 事务内 CAS 校验 attempt、execution、updated_ms，并经 `validate_budget_run` 校验 task/environment 后写入 `interrupted` 与 `execution.interrupted` 事件，`automatic_resume=false`，不重放工具。
- 挂载点：`open_project_kernel_state`（项目内核打开时）。对账失败只记录日志，不阻断打开。
- 取得租约失败时 `ManagedRun` 被解除武装（`finished = true`）。该分支为防御性代码，目前仅由阅读验证，未单独测试；现有 insert 会拒绝重复 Run ID，因此正常路径下失败只会发生在刚插入的行上。
- 租约 `lease` 字段在终态写入之后释放（`Drop::drop` 先于字段析构）。
- 锁文件路径先经 `canonical_root` 规范化，取得与对账指向同一文件（审查发现的路径形式不一致问题已修复）。

## 测试（`cargo test -p agentz-desktop --locked --lib owner_lease`：5 passed）

1. `held_lease_is_alive_and_released_lease_reads_back_its_binding`：同进程二次取得被拒；持有时探测为 Held；释放后探测为 Gone 且 binding 完整回读。
2. `attempt_without_lease_file_is_unverifiable`：无租约文件 → Missing。
3. `unsafe_identities_never_reach_the_filesystem`：路径穿越与分隔符被拒。
4. `killed_owner_is_interrupted_while_live_owner_is_preserved`（故障注入）：父进程通过 `current_exe` 启动子进程，子进程经 `ManagedRun::begin_bound` 进入 Running 并持有租约；父进程对账得到 `Alive` 且 DB 仍为 running；杀掉子进程后再次对账得到 `Interrupted`，DB 为 `interrupted/not_ready`；再次对账为空（幂等）。
5. `lease_child_process`：子进程入口（仅在 `AGENTZ_OWNER_LEASE_CHILD` 存在时运行）。

回归：`cargo test -p agentz-task-state --locked`：147 + 2 passed。`cargo test -p agentz-desktop --lib`：265 passed，1 failed。失败项 `commands::agents::smoke_tests::smoke_sync_seeded_agents_to_kois` 在读取环境变量 `HOME` 时 panic（Windows 默认未设置），发生在本次代码执行之前，属于既有环境依赖，不是本次改动引入。

## 清单

`docs/upgrade/evidence/AZ-019-implementation-manifest-r1.json`（8 个文件：`crates/agentz-task-state/src/run.rs`、`recovery.rs`、`src-tauri/src/runtime/owner_lease.rs`、`durable_run.rs`、`runtime/mod.rs`、`commands/data_scope.rs`、`src-tauri/Cargo.toml`、`Cargo.lock`）。

## 独立审查结论

审查者结论 PASS（见 `AZ-019-independent-review-r1.md`）：未发现活动 owner 的 Run 会被误标为 interrupted 的路径；租约释放顺序、CAS 与 Missing 保留均已核对。

## 限制（未覆盖，不冒称通过）

- 创建 Run 行与取得租约之间若崩溃，会留下没有租约的 Queued 行：判为 unverifiable 并保留，且因 `active_task_run` 唯一索引可能阻塞同一任务的新活动 Run。该窗口很小，需要后续清理任务处理。
- 租约文件在创建与加锁之间崩溃，会留下空或残缺文件：探测返回错误并保留，不会自动修复。
- 非 Windows 平台的 `flock` 分支未编译验证（本机仅 Windows）。在 NFS 等网络文件系统上 `flock` 可能被模拟，存在误判 Gone 的可能；当前未拒绝网络文件系统。
- 只有通过 `ManagedRun::queue_bound`/`begin_bound` 创建的 Run 有租约（生产路径：`chat.rs` 的 `queue_bound` 与 `chat_turn.rs` 的 `begin_bound`）；其他创建路径（`queue`、`begin`、`task_details`、`durable_stream` 等）无租约，对账时判为 unverifiable 并保留。
- 租约生命周期与 `ManagedRun` 守卫绑定：若守卫在执行结束前被丢弃，Run 会被标为 interrupted（这是既有行为）。
- 租约文件不会在终态后删除，会随 attempt 数量增长（后续清理任务）。
- 只有进程级杀进程故障注入，没有真实系统重启的验证。
- 未验证 GUI 中的启动体验（ADR-008 批量豁免 GUI）。
