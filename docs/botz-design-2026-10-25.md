# BotZ 模式设计草案

2026-10-25。BotZ 是 CodeZ、WorkZ 之外的第三种模式：以代理（agent）为中心组织界面，而不是以会话为中心。底层沿用 WorkZ 的团队、资源池（pool）、巡检和心跳。

## 1. 调研结论

| 项目 | 星数（约） | 可借鉴的机制 |
|------|-----------|--------------|
| Paperclip | 70k | 树形组织；任务原子领取（冲突即放弃）；心跳唤醒（定时 / 被指派 / 被 @ / 审批结果 / 手动）；每代理月度预算，80% 警告、100% 暂停 |
| Claude Code Agent Teams | 官方 | lead + teammates；共享任务列表，带依赖阻塞；每代理一个 JSON 邮箱；计划审批、关闭请求等协议消息；人可直接和任一成员对话 |
| Vibe Kanban | 28k | 看板卡片 → worktree → diff 行内评论回灌给代理 → 一键 PR |
| Claude Squad / Conductor / Crystal | 3k–9k | 每个代理一个 worktree（加 tmux 或容器）；实例列表加状态 |
| Pixel Agents | 10k | 用工具调用和权限事件驱动办公室角色动画（打字、阅读、等待时冒气泡）；办公室区域映射到目录 |
| ChatDev / AI Town | 35k / 9k | 像素风公司或小镇，按角色和阶段对话 |
| Letta | 25k | 持久身份；分层记忆，代理可自行编辑 |
| Grok Bot | 闭源 | 侧栏是 Bot 名单而非聊天记录；每个 Bot 有身份、记忆、工具、在线状态；Routines 定时或由事件触发；只在需要审批时打扰人 |

来源见第 7 节。

## 2. AgentZ 现状（可复用）

- 模式切换：`App.tsx` 中 `type Mode = "codez" | "workz"`，标题栏按钮、`modeShortcut.ts`、`LayoutSnapshot.mode` 持久化。
- 团队：`teams.rs`，模板存于 `{config}/teams/<id>/team.json`，模式分 swarm 和 workflow。
- 资源池：`teams_create_pool` 按团队加项目创建 pool；`pool_org`、`pool_chat` 已有；`poolTodos`、`poolMessages`、`agentz:pool-event` 已有。
- 成员执行：`runtime/koi.rs` 的 `spawn_koi_turn`，会话 id 为合成的 `koi_task_{owner}_{todo8}`。
- 巡检与心跳：`runtime/patrol.rs`（30 秒一次）、`runtime/heartbeat.rs`。
- 看板：`CollabBoard.tsx`。worktree：`agentTask.ts`。并发：`agent_slots` 信号量，默认 3。
- 代理定义：`{config}/agents/<id>/agent.json`，含 `icon`、`color`、`system_prompt`、`skills`、`tools`、`llm_provider_id`。

缺口：

1. 没有跨会话的“代理主页”。代理只是人设，身份散落在各会话里。
2. 成员的 `koi_task_*` 会话没有持久的、可打开的线程，也没有“代理 → 会话”的索引。
3. 没有 Room 概念。`App.tsx` 只有一个 `projectDir`，pool 绑定在当前项目上，无法同时运行多个相互隔离的团队。
4. 没有每代理邮箱、任务领取锁、预算、Routines 界面。

## 3. 概念模型

- **Bot**：持久代理实例 = 代理定义（`agent.json`）+ 身份（名字、头像、颜色）+ 长期记忆 + Routines + 主线程。全局唯一，跨项目。
- **Room**：隔离的团队工作空间 = 名称 + 工作目录（本地或 `agentz-remote://`）+ 团队模板 + 一个 pool + 桌位布局。每个 Room 有自己的 pool 数据库作用域、看板和 worktree，Room 之间互不可见。
- **Desk**：Room 里一个成员的座位，绑定 Bot 和成员在该 Room 的线程会话（`botz-member` 源）。
- **Task**：沿用 `pool_todos`，统一状态机为 `backlog → todo → in_progress → in_review → done`，另有 `blocked`（必须填原因）和 `cancelled`；支持 `parent_id` 和 `depends_on`。
- **Mailbox**：每个 Bot 在每个 Room 一个收件箱，基于 `pool_messages` 加收件人和已读字段；支持 @提及。

## 4. 两个视图

### 4.1 Bot 视图（类 Grok Bot，全局单代理）

- 左栏是 Bot 名单：头像、在线状态（空闲 / 工作中 / 等待审批 / 出错 / 暂停）、当前任务、今日花费。
- 中间是 Bot 主线程：一个持久对话，复用 CodeZ/WorkZ 的消息列表、工具卡片、文件修改卡片。上下文过长时按现有压缩机制处理，不再新建会话。
- 右栏是 Bot 主页：
  - 人设与记忆块（可编辑，代理也可自行更新）。
  - Routines：cron 或事件触发，执行一个提示词或技能，带“试运行”按钮。
  - 所在 Room 列表与运行历史（来自 run events）。
- Bot 在 Room 里收到 @ 或被指派任务时，主线程显示一条摘要，点击可跳到对应 Room 的桌位。

### 4.2 Room 视图（团队拟人办公室）

- 顶部是 Room 标签：可建多个，每个绑定一个目录和团队模板。显示运行中的代理数和待审批数。
- 主区是办公室平面：
  - 每个成员一张办公桌，坐着卡通化身，用代理的 icon 和 color 生成，先用 SVG 和 CSS 动画，不引入游戏引擎。
  - 化身状态由事件驱动：写文件时打字，读文件或搜索时翻书，运行命令时看屏幕，等待审批时头顶冒“！”气泡，空闲时喝咖啡，出错时冒红色气泡。
  - 点桌位会从右侧抽屉打开该成员在本 Room 的会话，可直接对话（人可以和任一成员说话）。
  - 协调者坐在“主管位”。
- 侧栏或底栏是团队看板：复用 CollabBoard，按 Room 的 pool 过滤；卡片显示负责人头像，可拖拽改状态或负责人；`in_review` 卡片可打开 diff 评审，评论回灌给负责人。
- 动态流：成员之间的消息（`pool_messages`）以气泡形式短暂显示在桌位之间，完整记录在动态流面板。

## 5. 调度机制（在现有 pool 上补齐）

1. **原子领取**：`pool_todo_claim(todo_id, agent)` 用条件 UPDATE 实现；失败返回冲突，代理换下一个任务，不重试。
2. **依赖阻塞**：前置任务完成后，自动把依赖它的任务从 `blocked` 解锁为 `todo`，并唤醒负责人。
3. **心跳唤醒**：扩展 `heartbeat.rs`，触发源为定时、被指派、被 @、审批有结果、手动。每次唤醒只跑一个短回合，固定流程是读收件箱、选任务、领取、执行、写进度、更新状态。
4. **邮箱**：`pool_messages` 增加 `to_agent` 和 `read_at`。新增工具 `mailbox_read` 和 `mailbox_send`，或在 `pool_chat` 上扩展。
5. **结构化审批**：计划审批和危险操作复用 PermissionCard 与 InteractiveCard；审批结果作为唤醒事件。
6. **并发与预算**：每个 Room 有并发上限（在全局 `agent_slots` 之下再分配）；每个 Bot 有每日和每月 token 或费用上限，80% 警告，100% 暂停该 Bot。
7. **隔离**：Room 的 pool 作用域为 `room.dir`；每个领取的任务一个 worktree（沿用 `agentTask`）；`in_review` 时出 diff，合并由人确认。
8. **父子任务**：协调者拆分任务时带 `parent_id`；子任务全部完成后唤醒父任务负责人汇总。

## 6. 实施分期

| 阶段 | 内容 | 主要改动 |
|------|------|----------|
| P1 骨架 | 新增 `botz` 模式（标题栏、快捷键、布局持久化）；Room 注册表，存于 `{config}/botz/rooms.json`（目录、团队、pool id）；Room 视图静态桌位，点击打开成员会话（建立成员 → 会话索引）；Room 内嵌 CollabBoard | `App.tsx`、`src/workspaces/botz/*`、`src-tauri/src/commands/botz.rs` |
| P2 动起来 | 事件驱动的化身状态、消息气泡、等待审批提醒；多 Room 同时运行（pool、巡检、心跳按 Room 目录各自启动） | 复用 `onChatEvent` 和 `onPoolEvent`，按 Room 分流 |
| P3 调度 | 原子领取、依赖阻塞、收件人邮箱、扩展心跳触发源、Room 并发上限 | kernel `pool`、`heartbeat.rs`、`patrol.rs` |
| P4 Bot 视图 | Bot 名单、主线程、记忆块、Routines（复用 automationReminder 的调度）、预算 | `agents.rs` 扩展 bot 状态，新增 `bot_threads` 表 |
| P5 评审闭环 | 看板 `in_review` 时打开 diff 行内评论并回灌；一键合并或开 PR | 复用 GitPanel 和 agentTask 的合并逻辑 |

每个阶段单独可用、单独发布。

## 7. 来源

- Paperclip：https://github.com/paperclipai/paperclip
- Claude Code Agent Teams：https://code.claude.com/docs/en/agent-teams
- Vibe Kanban：https://github.com/BloopAI/vibe-kanban
- Claude Squad：https://github.com/smtg-ai/claude-squad
- Pixel Agents：https://github.com/pixel-agents-hq/pixel-agents
- Grok Bot：https://x.ai/news/designing-grok-bot 、https://docs.x.ai/grok-bot/skills-routines-and-automations
- Letta：https://github.com/letta-ai/letta
- ChatDev：https://github.com/OpenBMB/ChatDev

## 8. ????(2026-10-26,1.0.25)

| ? | ?? | ?? |
|----|------|------|
| P1 ???P2 ??? | ?? | Room???????????? |
| ???? | ?? | todo ? depends_on |
| Room ???? | ?? | Room ??? max_concurrency,patrol ? pool ?? |
| ???? | ?? | ????????/????,80% ??,100% ?? |
| ???? | ??(??) | ??? Parent: <id>,????????????????;? parent_id ? |
| ?? | ?? | ?? @ ??????,? to_agent/read_at ?? |
| ??? | ?? | ????????????????? patrol ????;@! ? coordinator ??;?? @ ??? |
| Open PR | ?? | ?????? gh |
| ?? pool ?? | ??? | 1.0.25 ????? pool_chat/pool_org |

