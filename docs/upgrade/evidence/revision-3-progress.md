# 升级进展对照

> **已过期（2026-10-15）**：本文件为 2026-10-05 快照，不再代表当前状态。当前以 `docs/upgrade/plan.json` 台账及 2026-10-13 全量 reassessment（`docs/upgrade/evidence/reassessment-2026-10-13-integrations.json`）为准；台账已记录后续修复与阻塞。

2026-10-05。计划版本 3。基线仍是 a787b6d / AgentZ 0.9.6；此后的实现都在工作区未发布的修改里。台账数字只统计任务验收，产品旅程以各 Gate 为准。

## 判断

工程契约已经冻结，单任务的环境、变更、归档和合并预检已经接到宿主。权限意图的持久层和主聊天执行钩子也已经接到宿主。桌面旅程、真实模型和耐久运行还没有验收。

M0 通过，只说明可以按契约改代码。M1 Gate 还不能签：AZ-009、AZ-013 缺原生核对，AZ-079 的同轮 Undo 尚未改内核。M2 还没有 Run 表，续聊仍会按默认值清掉计划。AZ-028 在多个周期里扩大到搜索、索引、LSP、符号和预览卡片；这些实现留在原证据里，未完成的表面改由后续任务单独验收。

## 台账与代码

| 范围 | 台账 | 代码里已经存在的部分 | 还差的验收 |
| --- | --- | --- | --- |
| M0 AZ-001–003、005、006 | done | 契约、身份 ADR、Git 夹具、支持矩阵、单人容量 | 桌面与用户验收不在本 Gate |
| MB AZ-004 | blocked | 没有固定 12 项任务的真实模型运行记录 | 模型配置与预算 |
| M1 AZ-007、008 | done | `task-environments.db`；`chat_send` 排队前后校验 worktree | 不代替 AZ-009 的重启续聊 |
| M1 AZ-009 | review | WorkZ `openTask` 按 session 读取登记，执行根不再信共享 ref | 原生 A/B 交叉续聊和重启后的文件证据 |
| M1 AZ-010–012、014–016 | done | 显式起点、四类 ChangeSet、`task_journal_root`、归档恢复、保留式清理、合并预检与事务 | 宿主和领域测试；原生旅程另计 |
| M1 AZ-013 | review | `AgentTaskReview` 按 task id 取 ChangeSet / FileView | 原生文件列表与目录核对 |
| M1 AZ-079 | pending | 固定内核 `undo_turn` 仍按路径保留最新 before 内容 | 同轮重复编辑恢复到轮前字节 |
| M2 AZ-017–026 | pending | 审批 binding 里有 attempt 字段；取消槽在 `AppState` 内存；`clear_plan` 默认 true | 持久 Run/Attempt、事件游标、启动对账 |
| M3 AZ-027 | done | 权限分级契约 | 不是执行器 |
| M3 AZ-028 | review | `agentz-task-state` 审批库、`ApprovalBroker`、`ApprovalHooks` 的 before/after | 原生点击；相邻表面已拆出 |
| M3 AZ-029 | review | 本地内核补丁：加密失败拒绝保存，并附可复核 patch | 固定远端依赖仍是 v0.8.68；跨版本回滚未复核 |
| M3 AZ-030 及以后 | pending | 技能元数据已有 source；后台工具表会裁掉写入工具 | 系统凭据、秘密引用、Webview、授权变更记录、OS 隔离 |
| M4–M9 | pending | 既有产品功能仍在，台账中的交接、IDE、Desktop、发行任务未按本计划开工 | 各自 Gate |

## 已经接上的宿主路径

- 环境：`crates/agentz-task-state` 登记 project、workspace、分支和 base SHA；`agent_task` 用 Git 事实校验，拒绝前端改绑。
- 变更与交付：同一任务记录驱动 ChangeSet、FileView、归档和合并预检。评审界面读取这份记录，不再用分支三点 diff 代替工作区。
- Journal：查询、diff 和 Undo 经 `task_journal_root` 进入任务根。AZ-079 说明路由正确并不能使同轮多次编辑的 Undo 恢复到最早快照。
- 审批：主聊天在工具调用前签发意图、人预览、claim，调用后结束。新 boot 使旧的 pending/approved 过期，已 claim 未结束的记为 unknown。Koi、IM 和没有受保护预览的后台注册表被收成只读，这是注册表裁剪，不是操作系统隔离。

## 执行偏差

单人执行，台账曾同时处于 1 项 doing 和 3 项 review，超过 review 上限 2。时间主要耗在 AZ-028 的相邻表面上，而 M1 Gate 所缺的是原生证据和一条内核 Undo 修复。已完成项记录的 actual_days 是会话内粗估，多在 0.02–0.15 天，不能换成日历工期或用户验收。

版本 3 把当前视野收成四件可审阅工作：AZ-009、AZ-013、AZ-028、AZ-029。下一项代码是 AZ-079。M2 从 `agentz-task-state` 的 Run/Attempt 记录和 `clear_plan` 默认值改起，等 M1 Gate 通过后再进入 doing。
