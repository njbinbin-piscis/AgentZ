# AZ-025 收口证据：外部动作幂等与恢复对账

日期：2026-10-12
范围：`crates/agentz-task-state/src/external_action.rs`（领域模块）及其在 `lib.rs`、`run.rs`、`events.rs`、`approval.rs` 中的依赖。

## 1. 验收标准对照

| 验收项 | 结果 | 依据 |
|---|---|---|
| 执行成功但结果丢失时不盲重放；先查询或要求人工处理 | 满足 | `controlled_service_loses_result_reopen_never_replays`：Started 之后丢失结果，重新打开数据库后不再发放 permit；`unknown_requires_evidence_and_failed_is_terminal`：Unknown 只能经受信对账进入终态。无自动重试接口。 |
| 验证：可控外部服务重复/丢结果故障案例 | 满足（范围限定） | 进程内合成服务计数器模拟“执行成功但丢失结果”；同一 action key 重复准备返回 Existing 且不可执行（`duplicate_prepare_is_existing_and_immutable_scope_conflicts`）；两个连接竞争 start 只产生一个 permit（`competing_connections_issue_exactly_one_permit`）。 |

## 2. 实际执行结果（本轮，2026-10-12）

- `cargo test -p agentz-task-state --locked external_action -- --test-threads=1`：12 passed，0 failed。
- `cargo clippy -p agentz-task-state --locked --all-targets -- -D warnings`：通过。

12 个测试覆盖：重复准备与不可变范围冲突、两连接单 permit 竞争、旧 attempt 拒绝、丢结果后重开不重放、Unknown 与 NoEffect 终态语义、completion SQL 失败保持 Started、事件 INSERT 失败回滚、revision 耗尽不改变状态、缺失/未来版本/部分/损坏/视图命名空间不修复、只读不改字节、审计载荷有界且不含秘密。

## 3. 清单

`docs/upgrade/evidence/AZ-025-closure-manifest.json`，5 个文件，指纹为当前字节。

- `external_action.rs`（`2f078b…`）、`lib.rs`（`ba73a7…`）、`events.rs`（`7fcfa2…`）、`approval.rs`（`61cc69…`）与 AZ-025 台账既有复核记录一致。
- `run.rs` 当前为 `b68c1c…`，与 AZ-025 台账记录的 `ea4b9f…` 不同：AZ-019 之后为 `run.rs` 增加了 `active_runs`，`ea4b…` 是此前的字节。当前 `run.rs` 字节**不**被既有 AZ-025 复核覆盖，只被本次收口复核覆盖（见第 5 节）。

## 5. 收口复核的来源说明

第一轮独立收口复核（`independent-reviewer-az025`）对代码的 8 项安全属性和 12 个测试全部给出 PASS，唯一阻塞是 `run.rs` 的来源记录错误（即第 3 节所述）。该问题已在清单和本证据中更正：当前 `run.rs` 字节由收口复核覆盖，不再声称由既有 AZ-025 复核覆盖。非阻塞观察（调用方可断言 NoEffect、`start` 覆盖 origin 未比较 run_id、`action_digest` 无盐）记入第 4 节限制。

## 4. 明确的限制（不声称已验证）

- 受控服务是进程内计数器，不是 HTTP 服务，也不是真实外部提供方；不声称真实发送、发布或 PR 已验证。
- 没有宿主 provider 适配器、IPC、启动 owner 证明、renderer 授权；能力与绑定版本的新鲜度、实际外发请求与 permit 的一致性，由未来的受信宿主适配器负责。
- 不声称分布式 exactly-once：模块只记录本地的一次性授权，无法保证提供方遵守幂等键。
- 不能防止调用方滥用已发放的 permit。
- GUI 与安装包：本任务不涉及界面，`gui_status` 为 `not_required`。
- `Evidence::NoEffect` 与 Unknown 对账由调用方提供，类型上不强制“受信”；这是记录的限制，不是已解决的保证。
- `start` 覆盖存储的 origin，未与准备时的 run_id 比较；`prepare` 接受历史 attempt 的 origin。二者都不发放执行授权，但需在宿主集成时约束。
- `action_digest` 是对 `(task_id, action_key)` 的无盐 SHA256；低熵 key 可被离线猜测，不输出明文。
- `prepare`、`start` 在命名空间缺失时的错误分支没有直接测试（由 fail-closed 路径间接覆盖）。
- 本证据不改变任何正式依赖或 Gate 的状态。
