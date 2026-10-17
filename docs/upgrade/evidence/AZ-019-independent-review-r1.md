# AZ-019 独立复核记录（r1：宿主 owner 租约与启动对账）

复核者：independent-reviewer-az019（非 owner Codex (恢复实现)）。只读复核，未修改文件，未提交。
范围：`src-tauri/src/runtime/owner_lease.rs`、`durable_run.rs`（attach_lease、queue_bound、begin_bound、queue）、`crates/agentz-task-state/src/run.rs`（active_runs）、`recovery.rs`（reconcile 的使用方式）、`commands/data_scope.rs`（open_project_kernel_state 挂载）。
实现说明：`docs/upgrade/evidence/AZ-019-owner-lease-implementation.md`。
清单：`docs/upgrade/evidence/AZ-019-implementation-manifest-r1.json`（8 个文件）。

## 结论

PASS。未发现活动 owner 的 Run 会被误标为 interrupted 的路径。

## 安全性核对

- 同进程第二个句柄取得 Windows `LockFileEx` 独占锁时返回 `ERROR_LOCK_VIOLATION`（锁属于文件句柄而非进程），映射为 Held；Unix 每次 `open()` 生成新的 open file description，`flock` 同样冲突。测试 `held_lease_is_alive_and_released_lease_reads_back_its_binding` 已实证 Windows 情形。
- 取得与探测之间的竞争：探测若在创建与加锁之间执行，读到空/残缺记录会返回 Error 并保留，不会误判。写入只在持锁时进行，写入期间的探测得到 Held。
- `attach_lease` 失败时 `finished = true` 先于返回，`Drop` 不写入；该分支为防御性代码，未单独测试。
- `Drop::drop` 先于字段析构，因此 Interrupted 写入先于租约释放。
- 对账期间持锁，且领域层 CAS 校验 attempt、execution、updated_ms，过期快照返回 Err 且不写入。
- 只有持有 `Gone` 租约（锁可取得且对应同一 run/attempt）时才产生 `ConfirmedGone`；无租约的 Run 只会得到 Missing 并被保留。

## 复核发现（均已处理或记入限制）

1. 路径形式不一致（取得用调用方路径，对账用规范化根）可能导致活动 owner 被判为 Missing。已修复：`lease_path` 使用 `canonical_root`。
2. 创建 Run 行与取得租约之间崩溃，会留下无租约的 Queued 行，并可能因 `active_task_run` 唯一索引阻塞同一任务。记入限制。
3. 租约文件在创建与加锁之间崩溃，会留下残缺文件，探测返回错误并永久保留。记入限制。
4. 非 Windows `flock` 分支未编译验证；NFS 上 `flock` 可能被模拟。记入限制。
5. 守卫生命周期与租约绑定：确认 `chat.rs` 的 `queue_bound` 与 `chat_turn.rs` 的 `begin_bound` 调用方保持守卫存活至执行结束（既有行为）。

## 复核检查

- `cargo test -p agentz-desktop --locked --lib owner_lease`：5 passed（Windows）。
- 清单 8 个文件的 sha256 与当前文件一致。
- 实现说明与代码一致；已修正 task/environment 由 `validate_budget_run` 校验而非 CAS 的表述。

## 未覆盖

- 未编译 Unix 分支。
- 未在真实系统重启中验证。
- 未运行全量 `agentz-desktop` 测试（既有 `smoke_sync_seeded_agents_to_kois` 因 `HOME` 未设置失败，与本次无关）。
