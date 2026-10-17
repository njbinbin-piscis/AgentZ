# AZ-023 复核修复记录 r3

日期：2026-10-12
状态：修复待独立复核（本文件不改变台账状态）

## 1. 针对 BLOCK 的修复

### 1.1 取消退避缺少耗时断言（已修复，双边界）

原测试只断言请求次数，无法证明取消打断了退避等待。现增加耗时断言：

- `vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs`：`retry_fixture_timed` 返回 `(请求数, 耗时)`；`cancellation_during_real_backoff_starts_no_second_request` 断言：
  - 下界 `elapsed >= 50ms`：取消在第一次请求之后至少 50ms 才发出，因此运行必然进入退避窗口；立即结束的运行会被拒绝；
  - 上界 `elapsed < 800ms`：首次真实退避为 1s，超过上界说明退避被睡完而非被取消打断。
- `vendor/piscis-engine/piscis-kernel/tests/az023_retry_http.rs`：`exercise` 返回耗时；`http_transient_backoff_cancellation_starts_no_second_request` 使用同样的双边界（服务端在 500 响应后 50ms 才设置取消）。

依据：`retry_backoff(0) = 1s`（`loop_.rs:61-63`）；退避等待每 200ms 轮询取消标志。

第一轮修复只有上界；独立复核指出禁用重试时测试仍会通过，因此补充了下界。

### 1.2 清单说明与哈希链（已修复）

r2 清单的 `note` 称 `loop_.rs` 为“pre-AZ-021 + cancel-grace”，不准确。当前工作区文件与 AZ-023 状态之间的真实链条（使用 `git apply --directory=piscis-kernel` 统一根路径）：

| 步骤 | 文件 | sha256 |
|---|---|---|
| 当前工作区 `loop_.rs`（修复后） | `loop_.rs` | `1CA6F09E620C5E1D15797E6A598A25C29A5F50004A62D8D6C527C0F53C5339C6` |
| 反向应用 AZ-021 cancel-grace | 中间状态 | `E17D4775636B0885FE03430D306AB3EC2D4CFFE7E760997E3D0473923A1A692F` |
| 再反向应用 AZ-021 invocation-completion | AZ-023 状态（含耗时断言） | `2F213963BF306E829DDDE87FCFE6E5020FCB163AD8B90971EC92F320EDE1E31E` |
| 反向应用 r3 AZ-023-loop 补丁到上述状态 | AZ-023 之前（pre） | `DAF4E104C71119A236D626139914B587A0D4CE81D4B07ADE5FCED0EB7A5B0F7E` |

- 修复前的 AZ-023 状态（无耗时断言）为 `846AE9D3198C2C6BAC80611A530998EE8872FABB80154E08A2921066EA795781`，与 `AZ-023-reconstruction.md` 一致。
- 之前报告的“其他 AZ-021 补丁反向失败”是路径前缀问题（cancel-grace 补丁使用 `src/` 前缀，需要 `--directory=piscis-kernel`；invocation-completion 补丁使用 `piscis-kernel/` 前缀），不是内容冲突。统一根路径后，两者均可按序反向与正向应用。
- owned-completion 与 vendor-sandbox-completion 补丁只修改 `agent/mod.rs`、`agent/tool.rs`、`agent/tool_completion.rs`、`tools/shell.rs`、`tools/code_run.rs`，不修改 `loop_.rs`。

### 1.3 补丁重新生成

AZ-023 交付的测试代码已变化，因此 `docs/upgrade/patches/` 中两个 AZ-023 补丁已按最终 AZ-023 状态重新生成并覆盖（均未提交到 git）：

| 补丁 | sha256 |
|---|---|
| `AZ-023-error-class-v0.8.68.patch`（未变） | `4d4ecc3ca917391901d357f7c77922f572fcb25272dd2b6dd1a064a2f554cf22` |
| `AZ-023-loop-v0.8.68.patch` | `b45aedebcb4a257612d3e33651580480a15079c2a2df5f13f5a69580d550d7b2` |
| `AZ-023-http-fixture-v0.8.68.patch` | `f009e1fb5f94bc94e7d1d7bc00ac79b26092d3ff03657380017ababbe13006b7` |

验证：
- 新 loop 补丁从 pre 状态应用后得到 `2F2139…`（true）。
- 新 http 补丁应用到空目录后得到当前测试文件（true）。
- AZ-021 两个补丁正向应用到 `2F2139…` 后得到当前 `loop_.rs`（true）。

## 2. 验证命令与结果

在仓库根目录运行（kernel 通过根 `Cargo.toml` 的 `[patch]` 接入）：

- `cargo test -p piscis-kernel --locked --test az023_retry_http`：4 passed（含两个取消用例，双边界断言通过）。
- `cargo test -p piscis-kernel --locked --lib cancellation_during_real_backoff`：1 passed。
- `cargo test -p piscis-kernel --locked --lib retry`：11 passed。
- `node scripts/upgrade-kernel-candidate.mjs`：退出 0。八个补丁在固定基线归档上通过 `apply --check`、`apply`、`apply --reverse --check`；候选归档 `candidate.tar` sha256 `1ab4e3b3dfaba2b64308bf17f1f60b1ab516a66df36d301b0e4cc855f1726a4e`。

## 3. 清单

- r3 清单：`docs/upgrade/evidence/AZ-023-implementation-manifest-r3.json`（7 个文件，指纹为当前工作区内容）。
- r2 清单保留为历史记录，不再作为复核依据。

## 4. 非阻塞残留（不影响本次复核结论）

- `same_model_transient_retries` 命名与文档：字段表示总尝试次数而非重试次数，限幅 1..5。仅文档/命名问题，本次不改代码，留待后续整理。
- `error_class.rs` 中 5xx 与 model-not-found 的判定顺序：本次未修改该顺序，也未新增针对该组合的测试；作为后续边界测试的备注保留。
- 包版本显示：cargo 编译日志显示 `piscis-kernel v0.8.63`，补丁名为 `v0.8.68`。仅为命名差异，未在本次验证。
