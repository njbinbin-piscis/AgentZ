# AZ-042 收口证据：语言服务崩溃恢复

日期：2026-10-12
范围：`src/services/tauri/lspSession.ts`（恢复预算与恢复循环）、`src/services/tauri/lspSession.test.ts`（回归用例）。

## 1. 重评估缺口与当前代码

2026-10-11 重评估指出：成功重连后 `recoveryAttempts` 从不重置，会话内第四次崩溃不会恢复。当前代码已经修复：

- `lspSession.ts`：`recover()` 中，`reconnect` 成功后执行 `this.recoveryAttempts = 0`，注释写明“健康握手后，下一次崩溃获得新的重启预算”。
- 每小时窗口仍有 `MAX_RECOVERIES_PER_WINDOW = 10` 的硬上限，防止“启动后立即崩溃”的循环。

## 2. 测试覆盖

`lspSession.test.ts` 的 10 个用例全部通过，与恢复相关的包括：

- 重启预算耗尽后停止（`bounds failed automatic restarts instead of looping indefinitely`）。
- 在新网桥上恢复，共享客户端不替换（`recovers on a fresh bridge without replacing the shared client...`）。
- 每次健康重启后重置预算，后续崩溃仍能恢复（`resets the restart budget after each healthy restart so later crashes recover`）：多轮崩溃与恢复。
- 真正的崩溃循环在重启上限处停止（`stops a true crash loop at the restart cap after healthy restarts`）。
- 启动后崩溃的循环在窗口上限处停止（`stops a start-then-crash loop at the per-window recovery cap`）。

执行结果（本轮）：`npx vitest run src/services/tauri/lspSession.test.ts`：10 passed，0 failed。

## 3. 清单

`docs/upgrade/evidence/AZ-042-closure-manifest.json`：`lspSession.ts`、`lspSession.test.ts` 当前字节指纹。

## 4. 未验证范围（不声称已验证）

- 桌面端 TypeScript 与 Rust 语言服务进程被杀的实机演练未执行。按 ADR-008 统一 GUI 豁免记录为 `waived:ADR-008`，复核记录 GUI 未验证。
- Monaco 悬停与补全在恢复后的表现未在真实界面中验证。
- 健康状态 `unavailable` 与 `recovering` 的切换是视觉层面的细节，本次不作为验收项。
