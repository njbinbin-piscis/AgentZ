# BotZ Mode Design Draft

**English** | [简体中文](botz-design-2026-10-25.zh-CN.md)

> Status (2026-11-02): this is the original 2026-10-25 proposal; §2 describes the code as it was then. Since then `botz` has shipped as the third mode (`type Mode = "codez" | "workz" | "botz"` in `src/App.tsx`, shortcut Ctrl/⌘+Shift+Alt+N in `src/utils/modeShortcut.ts`), with the room registry, bot profiles (memory, routines, daily/monthly token budgets), per-room concurrency, parent/child todos, an @mention inbox and one-click PRs in `src-tauri/src/commands/botz.rs`, `src-tauri/src/runtime/parent_wake.rs` and `src/workspaces/botz/`. Rooms now own their member list instead of only pointing at a team template. Some mechanisms landed differently from the plan — see §8. Not implemented: a dedicated `pool_todo_claim` command (atomic claiming is the kernel's `pool_org(action="claim_todo")`), `to_agent`/`read_at` columns, the `mailbox_read`/`mailbox_send` tools and a `bot_threads` table.

2026-10-25. BotZ is a third mode alongside CodeZ and WorkZ: the UI is organized around agents rather than sessions. Under the hood it reuses WorkZ's teams, resource pools, patrol and heartbeat.

## 1. Research findings

| Project | Stars (approx.) | Mechanisms worth borrowing |
|---------|-----------------|----------------------------|
| Paperclip | 70k | Tree-shaped org; atomic task claiming (give up on conflict); heartbeat wake-ups (timer / assigned / @mentioned / approval result / manual); per-agent monthly budget, warn at 80%, pause at 100% |
| Claude Code Agent Teams | Official | Lead + teammates; shared task list with dependency blocking; one JSON mailbox per agent; protocol messages such as plan approval and shutdown requests; humans can talk directly to any member |
| Vibe Kanban | 28k | Kanban card → worktree → inline diff comments fed back to the agent → one-click PR |
| Claude Squad / Conductor / Crystal | 3k–9k | One worktree per agent (plus tmux or a container); instance list with status |
| Pixel Agents | 10k | Tool calls and permission events drive office character animations (typing, reading, speech bubble while waiting); office areas map to directories |
| ChatDev / AI Town | 35k / 9k | Pixel-art company or town; conversations by role and phase |
| Letta | 25k | Persistent identity; layered memory the agent can edit itself |
| Grok Bot | Closed source | The sidebar is a bot roster, not a chat history; each bot has an identity, memory, tools and presence; Routines run on a schedule or on events; it only interrupts the human when approval is needed |

Sources are in §7.

## 2. Current state of AgentZ (reusable)

- Mode switching: `type Mode = "codez" | "workz"` in `App.tsx`, title-bar buttons, `modeShortcut.ts`, persisted in `LayoutSnapshot.mode`.
- Teams: `teams.rs`; templates stored at `{config}/teams/<id>/team.json`; two modes, swarm and workflow.
- Resource pools: `teams_create_pool` creates a pool per team + project; `pool_org`, `pool_chat` already exist; so do `poolTodos`, `poolMessages` and `agentz:pool-event`.
- Member execution: `spawn_koi_turn` in `runtime/koi.rs`; the session id is the synthesized `koi_task_{owner}_{todo8}`.
- Patrol and heartbeat: `runtime/patrol.rs` (every 30 s), `runtime/heartbeat.rs`.
- Kanban: `CollabBoard.tsx`. Worktrees: `agentTask.ts`. Concurrency: the `agent_slots` semaphore, default 3.
- Agent definitions: `{config}/agents/<id>/agent.json`, with `icon`, `color`, `system_prompt`, `skills`, `tools`, `llm_provider_id`.

Gaps:

1. There is no cross-session "agent home page". An agent is just a persona; its identity is scattered across sessions.
2. Members' `koi_task_*` sessions have no persistent thread you can open, and there is no "agent → session" index.
3. There is no Room concept. `App.tsx` has a single `projectDir` and the pool is bound to the current project, so several mutually isolated teams cannot run at the same time.
4. There are no per-agent mailboxes, task claim locks, budgets or a Routines UI.

## 3. Concept model

- **Bot**: a persistent agent instance = agent definition (`agent.json`) + identity (name, avatar, color) + long-term memory + Routines + a main thread. Globally unique, across projects.
- **Room**: an isolated team workspace = name + working directory (local or `agentz-remote://`) + team template + one pool + desk layout. Each Room has its own pool database scope, kanban board and worktrees; Rooms cannot see each other.
- **Desk**: a member's seat in a Room, bound to a Bot and to that member's thread session in the Room (source `botz-member`).
- **Task**: reuses `pool_todos` with a unified state machine `backlog → todo → in_progress → in_review → done`, plus `blocked` (a reason is required) and `cancelled`; supports `parent_id` and `depends_on`.
- **Mailbox**: one inbox per Bot per Room, built on `pool_messages` plus recipient and read fields; supports @mentions.

## 4. Two views

### 4.1 Bot view (Grok Bot–style, one global agent)

- The left column is the Bot roster: avatar, presence (idle / working / awaiting approval / error / paused), current task, today's spend.
- The center is the Bot's main thread: one persistent conversation reusing the CodeZ/WorkZ message list, tool cards and file-change cards. When context grows too long the existing compaction handles it; no new session is created.
- The right column is the Bot home page:
  - Persona and memory blocks (editable; the agent can update them too).
  - Routines: cron- or event-triggered, run a prompt or skill, with a "dry run" button.
  - The Rooms it belongs to and its run history (from run events).
- When a Bot is @mentioned or assigned a task in a Room, its main thread shows a summary; clicking it jumps to the corresponding desk in that Room.

### 4.2 Room view (anthropomorphic team office)

- Room tabs on top: you can create several, each bound to a directory and a team template. Each shows the number of running agents and pending approvals.
- The main area is the office floor plan:
  - Each member has a desk with a cartoon avatar generated from the agent's icon and color — SVG and CSS animations first, no game engine.
  - Avatar state is event-driven: typing while writing files, flipping a book while reading or searching, watching the screen while running commands, a "!" bubble while awaiting approval, drinking coffee while idle, a red bubble on error.
  - Clicking a desk opens that member's session in this Room in a right-hand drawer, where you can talk to it directly (humans can talk to any member).
  - The coordinator sits at the "manager desk".
- The sidebar or bottom bar holds the team kanban: reuses CollabBoard filtered by the Room's pool; cards show the owner's avatar and can be dragged to change status or owner; `in_review` cards open a diff review, and comments are fed back to the owner.
- Activity feed: messages between members (`pool_messages`) briefly appear as bubbles between desks, with the full record in the activity feed panel.

## 5. Scheduling mechanisms (filling gaps on top of the existing pool)

1. **Atomic claiming**: `pool_todo_claim(todo_id, agent)` implemented with a conditional UPDATE; on failure it returns a conflict and the agent moves on to the next task without retrying.
2. **Dependency blocking**: when a prerequisite task completes, tasks depending on it are automatically unblocked from `blocked` to `todo` and their owners are woken.
3. **Heartbeat wake-ups**: extend `heartbeat.rs` with triggers for timer, assigned, @mentioned, approval result and manual. Each wake-up runs only one short turn with a fixed flow: read inbox, pick a task, claim it, execute, write progress, update status.
4. **Mailbox**: add `to_agent` and `read_at` to `pool_messages`. Add `mailbox_read` and `mailbox_send` tools, or extend `pool_chat`.
5. **Structured approval**: plan approval and dangerous operations reuse PermissionCard and InteractiveCard; the approval result is a wake-up event.
6. **Concurrency and budget**: each Room has a concurrency cap (allocated under the global `agent_slots`); each Bot has daily and monthly token or cost caps, warn at 80%, pause that Bot at 100%.
7. **Isolation**: a Room's pool is scoped to `room.dir`; each claimed task gets one worktree (reusing `agentTask`); `in_review` produces a diff and a human confirms the merge.
8. **Parent/child tasks**: the coordinator sets `parent_id` when splitting a task; once all subtasks are done, the parent task's owner is woken to summarize.

## 6. Implementation phases

| Phase | Content | Main changes |
|-------|---------|--------------|
| P1 Skeleton | Add the `botz` mode (title bar, shortcut, layout persistence); Room registry stored in `{config}/botz/rooms.json` (directory, team, pool id); static desks in the Room view, clicking opens the member session (build the member → session index); CollabBoard embedded in the Room | `App.tsx`, `src/workspaces/botz/*`, `src-tauri/src/commands/botz.rs` |
| P2 Bring it to life | Event-driven avatar states, message bubbles, awaiting-approval alerts; multiple Rooms running at once (pool, patrol and heartbeat started per Room directory) | Reuse `onChatEvent` and `onPoolEvent`, routed per Room |
| P3 Scheduling | Atomic claiming, dependency blocking, recipient mailboxes, extended heartbeat triggers, Room concurrency caps | kernel `pool`, `heartbeat.rs`, `patrol.rs` |
| P4 Bot view | Bot roster, main thread, memory blocks, Routines (reusing automationReminder scheduling), budgets | Extend `agents.rs` with bot state, add a `bot_threads` table |
| P5 Review loop | Open inline diff comments on `in_review` kanban cards and feed them back; one-click merge or PR | Reuse GitPanel and agentTask merge logic |

Each phase is usable and releasable on its own.

## 7. Sources

- Paperclip: https://github.com/paperclipai/paperclip
- Claude Code Agent Teams: https://code.claude.com/docs/en/agent-teams
- Vibe Kanban: https://github.com/BloopAI/vibe-kanban
- Claude Squad: https://github.com/smtg-ai/claude-squad
- Pixel Agents: https://github.com/pixel-agents-hq/pixel-agents
- Grok Bot: https://x.ai/news/designing-grok-bot , https://docs.x.ai/grok-bot/skills-routines-and-automations
- Letta: https://github.com/letta-ai/letta
- ChatDev: https://github.com/OpenBMB/ChatDev

## 8. Implementation status (2026-10-26, 1.0.25)

> The original Chinese text of this section was lost to an encoding error before this revision; the table below was reconstructed from the 1.0.25 release ("BotZ room concurrency, member budgets, parent tasks, mentions inbox, open PR") and the current code.

| Item | Status | Notes |
|------|--------|-------|
| P1 skeleton, P2 animation | Done | Room registry `{config}/botz/rooms.json` (`botz_list_rooms` / `botz_save_room` / `botz_open_room`); event-driven desks and bubbles in `src/workspaces/botz/RoomView.tsx` |
| Dependency blocking | Done | Via the kernel todo's `depends_on`; patrol activates pending todos once upstream is done (`pool/coordinator.rs`) |
| Room concurrency cap | Done | `BotzRoom.max_concurrency`; patrol uses `room_max_concurrency` for that pool, falling back to the global pool cap (`runtime/patrol.rs`) |
| Budgets | Done | `BotProfile.daily_token_budget` / `monthly_token_budget`; `budget_block` refuses new member turns once a cap is reached (`commands/botz.rs`) |
| Parent/child tasks | Done (by convention) | The kernel todo has no parent column; the coordinator writes a `Parent: <id>` line in each subtask description, and when all subtasks finish patrol wakes the parent's owner once with an `@!` mention (`runtime/parent_wake.rs`); no `parent_id` column |
| Mailbox | Done | A member's inbox is the pool messages that @mention it; read state is kept per room in the renderer's `localStorage` (`botz-mail-<room id>`), not in `to_agent`/`read_at` columns |
| Wake-ups | Partial | Assignment and @mentions wake members through the existing pool/patrol path; `@!` targets a member directly; no separate heartbeat trigger types were added |
| Open PR | Done | `botz_open_pr` runs `gh pr create` and returns the PR URL |
| Atomic claiming | Existing kernel feature | Provided by `pool_org(action="claim_todo")`; no new `pool_todo_claim` command |
| Bot view | Done | `BotHomePage.tsx`, `BotPicker.tsx`, `BotRoutineRunner.tsx`; profiles via `botz_get_bot` / `botz_save_bot` (no `bot_threads` table) |
