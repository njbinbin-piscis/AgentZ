# Koi 多 Agent 状态机漏洞分析报告（1.0.64 复核版）

日期：2026-11-12 · 代码基线：**AgentZ 1.0.64**（origin/main `66d2b56`，分支 `sync/main-1.0.64`）
kernel：**piscis-engine v0.8.73**，源码已 vendor 进仓库（`vendor/piscis-engine/`），本报告全部引用可在仓库内直接验证。

> 上一版报告基于 0.9.6 + kernel v0.8.68。本次同步后复核：**5 项 Critical 中 2 项已修复、1 项部分修复、2 项仍在**；另发现上游新增的修复机制与残留缺口。

> **修复进展（2026-11-12 执行批次，计划 `.agentz/plans/koi-fix-and-tool-friction.md`）**：
> - ✅ C2 失败即 blocked → kernel 增加 transient 自动重试（retry_count/next_retry_at + 30s·2^n 退避，默认 2 次，CoordinatorConfig.max_auto_retries 可配）；顺带修复 classify_error 漏判 "timed out"。
> - ✅ C4 watchdog 竞态 → 进程级共享 SubagentRuntime + todo↔turn 内存登记；patrol 回收前先按相同阈值取消 stale turn；wait_koi_turn 可重读；丢弃结果前记录日志。
> - ✅ C5 残留 → 看板新增 needs_review 列 + 15s 轮询对账；pool_members 读取时自愈 stale busy（持久化回写 idle）。
> - ✅ W1 workflow 节点超时 → 默认 600s（原 None 永久阻塞）。
> - ✅ W2 wait 一次性消费 → outcome 缓存可重读。
> - ✅ W6 UA 泄漏 → 实测确认 HeadlessChrome 泄漏，bind() 统一应用桌面 UA（agent-only 会话也覆盖）。
> - ✅ C3 残留（第二批）→ patrol 不再仅限 swarm（所有团队模式）；pool resume 时 ensure+kick 重启 patrol；项目 restore 时按需启动（ensure_pool_patrol_if_pools）；启动轮改用真实阈值（kernel watchdog 拒绝 ≤0 无差别重置，防多实例误杀）；退休逻辑区分"无 pool"与"仅暂停"（暂停池保持 patrol 存活）。
> - 验证：kernel pool 19 passed / ide-tools lsp 7 passed / desktop approval 5 passed / vitest workz 51 passed / read_lints 0 告警。
> - 仍开放：主聊天无配置超时仍 1 年（产品语义，未改）。

## 0. 同步结果

- 远程领先 175 个提交（0.9.6 → 1.0.64）。旧合并（355 冲突）已中止，改为：新分支 `sync/main-1.0.64` 基于 origin/main + cherry-pick 本地两个提交（UA 修复 `f3135b6`、docs/i18n `ff58788`）。
- 原 main 与备份分支 `backup/pre-sync-20261112` 保留，可随时回退。
- 工作区干净，`git diff --check` 与全仓冲突标记扫描均为零；package.json / Cargo.toml / Cargo.lock 版本一致（1.0.64）。
- ⚠️ 本地 UA 补丁 cherry-pick 后落在 robotz_browser 新驱动上（上游已弃用 chromiumoxide），**需验证其是否仍生效**（见 W6）。

## 1. 上游已修复（对照上版报告）

| 上版编号 | 修复内容 | 证据 |
|---|---|---|
| C1 合并残留 | 本次同步彻底清理 | 全仓扫描零冲突标记 |
| C3 heartbeat 死代码 | **已接线**：swarm 团队同步时启动 patrol + heartbeat | `commands/teams.rs:387-390` |
| C5 忙碌残留（主聊天路径） | 新增 **SessionRunningGuard**（RAII：任何提前退出/panic 都回写 idle/error）+ **reconcile_listed_status**（列表读取时按 TaskActivity 自愈 running→idle，含测试） | `commands/chat_turn/mod.rs:57-101`、`commands/session.rs:482-512` |
| 衔接延迟 | **kick 机制**：todo Completed/Cancelled 事件立即唤醒 patrol（500ms 后），不必等 30s 轮询；botz 面板操作也 kick | `runtime/koi.rs:60-64`、`runtime/patrol.rs:37-46,97-103`、`commands/botz.rs:498,536,728` |
| 并发失控 | patrol 增加每 pool 并发上限（默认 3，`AGENTZ_POOL_MAX_CONCURRENCY` 可调） | `runtime/patrol.rs:48-56,194-206` |
| 父任务收尾 | 新增 **wake_finished_parents**：子任务全完成后自动 @! 父任务 owner 汇总并 complete_todo | `runtime/patrol.rs:229-285`、`runtime/parent_wake.rs` |
| 看板不可编译 | CollabBoard 重写：正常编译 + 每卡实时 CLI 输出窗 + 依赖等待提示 | `workspaces/workz/CollabBoard.tsx` |
| 成员工具缺失 | Koi registry 现在注册 pool_chat/pool_org（成员可交付、complete_todo、@! 交接） | `runtime/koi.rs:226-229` |

## 2. 仍然存在的漏洞

### 🔴 C2. 失败即 blocked，无自动重试（核心根因，未变）
- kernel `coordinator.rs:739` 失败 → `block_koi_todo`；`:743` 成功但未 complete_todo → `needs_review`。
- `activate_pending_todos`（`coordinator.rs:1319`）**仍只捡 `status == "todo"`**。
- 桌面侧 grep `resume_blocked_todo|auto_retry|retry_count` **零命中**——blocked/needs_review 只能靠 Piscis 手动 resume。
- heartbeat 虽已接线，但它是"提醒 Piscis 来协调"，不是自动恢复；Piscis 不在线/不响应时任务仍停摆。
**结论**：一次瞬时 LLM 错误 = 任务永久 blocked + 下游 depends_on 链冻结。这是"卡住/未交付"的第一根因。

### 🔴 C4. watchdog 纯 DB 重置，不取消运行中的 turn（未变）
- `watchdog_recover`（`coordinator.rs:1390-1409`）仍是纯 DB pass-through，无 `cancel_koi_turn` 调用。
- 桌面 inflight map 按 turn_id 键控，watchdog 重置后旧 turn 继续跑 → patrol 再派发 → **同一 Koi 双 turn 并发**。
- `cleanup_unclaimed_outcome`（`runtime/koi.rs:198-213`）仍 60s 后静默丢弃未认领结果，无 DB 回写。
- 缓解：patrol 阈值改为 `max(600, task_timeout*1.2)`（patrol.rs:162-177），正常超时先于 watchdog 触发，误杀概率降低；但崩溃/挂死场景的竞态仍在。

### 🟡 C3 残留. patrol/heartbeat 覆盖面
- 仍仅 `team.mode == "swarm"` 启动（teams.rs:387）；非 swarm 团队无兜底。
- patrol 3 轮空转自杀（patrol.rs:105-110）；pool pause→resume 无重启路径（botz_assign_member 会重新 ensure，但单纯 resume 不会）。
- 启动轮仍用 0 阈值无差别回滚（patrol.rs:93）——多实例场景误杀风险仍在。

### 🟡 C5 残留. Koi 成员徽章与看板自愈
- 主聊天 session 已有 Guard+reconcile，但 **`kois.status`（成员徽章）没有等价机制**：`pool_members` 仍原样返回 DB 值，崩溃路径若跳过 idle 恢复（coordinator.rs:227-236 只在 inner 正常返回后执行），徽章永久 busy。
- CollabBoard 仍纯事件驱动、无轮询（CollabBoard.tsx:85-102 注释声称 polls 实际没有）；终态事件丢失 = 冻结。
- **needs_review 仍不在任何看板列**（CollabBoard.tsx:46-50：todo/in_progress+blocked/done）——"完成但未交付"的任务在看板上凭空消失。

### 🟡 W1. 超时语义（部分未变）
- 主聊天无配置超时仍是 **1 年**（`chat_turn/mod.rs:871-874`）。
- workflow 节点仍 `task_timeout_secs: None`（`runtime/workflow.rs:493,607`）→ 单节点挂死阻塞整条 workflow。
- kernel 默认 600s 生效于 Koi turn（coordinator.rs:50），此项正常。

### 🟡 W2. wait_koi_turn 一次性消费（未变）
`runtime/koi.rs:178-181`：二次 wait 直接 Err，重试路径结果丢失。

### 🟡 W6（新）. 本地 UA 补丁落点失效风险
上游浏览器驱动已从 chromiumoxide 换为 robotz_browser；cherry-pick 的 UA 补丁（+11 行）需确认在新驱动中仍被执行，否则"移动端布局/内容裁剪"问题会回归。

## 3. DeepSeek Harness 借鉴方案（不变，优先级按现状调整）

来源：github.com/deepseek-ai/deepseek-harness（文档存 `dsh-docs-tmp/`）。

1. **失败分类 + 自动重试**（对治 C2，最高优先）：借鉴其 `agent/request-error` waterfall——错误先经 hook 判定 retry/保留，而非一刀切 blocked。给 koi_todos 加 retry_count，transient 错误自动回 todo + 退避。
2. **interrupted 终态合成**（对治 C4/C5）：借鉴 persistence.zh.md 的 `interruptedTurnClosers`——启动/patrol 扫描"有 start 无 end"的 turn，合成确定终态并回写，替代纯时间阈值猜测；watchdog 回收前先取消 inflight turn（todo↔turn_id 持久化映射）。
3. **任务 DAG + revision CAS**（对治竞态）：借鉴 agent-team.zh.md 的 compare-and-set + blockedBy 无环校验 + ready 派生属性。
4. **运行态/任务态解耦投影**（对治 C5 徽章）：借鉴"当前轮次活动与任务完成相互独立"——成员徽章显示会话运行态（来自 turn 事件），看板显示任务态，不再共用 kois.status。
5. **看板 needs_review 列 + 低频轮询对账**：最小改动，直接消除"任务消失"和"事件丢失冻结"。

## 4. 建议修复顺序（1.0.64 基线）

1. **C5 残留**：CollabBoard 加 needs_review 列 + 15s 轮询；kois.status 增加类似 reconcile_listed_status 的读取时自愈。改动小、收益直接。
2. **C2**：kernel 侧失败分类重试（vendor 源码已在仓库内，可直接改 + 测试）。
3. **C4**：turn 登记表（todo↔turn_id）+ watchdog 先取消后回收 + epoch 拒绝旧结果。
4. **W6**：验证 UA 补丁在 robotz_browser 下生效。
5. **W1/W2**：workflow 节点超时兜底、wait 结果可重读。
6. 中期：引入 revision CAS 与 interrupted 终态合成。
