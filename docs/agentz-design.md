# AgentZ — Design of a Cursor-like AI IDE

**English** | [简体中文](agentz-design.zh-CN.md)

> Status (2026-11-02, AgentZ 1.0.40, kernel piscis-engine v0.8.71): this document was written around the M0–M8 milestones and keeps that structure. Since then: the product shell has three modes — **CodeZ** (editor-centric, the "IDE mode" below), **WorkZ** (task/team-centric, including the worktree tasks of §6) and **BotZ** (agent-centric rooms, see [botz-design-2026-10-25.md](botz-design-2026-10-25.md)); the kernel's Pool/Koi orchestration *is* now wired into agent-mode turns (§1.1 has been updated); writable sub-agents exist through the `task` tool (§5); and a Node-based VS Code extension host now runs real `.vsix` extension JavaScript, with DAP debugging (§10). Still open: vector embeddings for the code index and TextMate grammars.

> AgentZ is a Cursor-like **AI IDE**. It was designed around two first-class modes:
> **IDE mode** (editor-centric, ≈ Cursor) and **Agent mode** (task-centric, ≈ Codex).
> Both share the agent kernel from [`piscis-engine`](https://github.com/njbinbin-piscis/piscis-engine)
> (`piscis-core` + `piscis-kernel` + `piscis-ide-tools`), pulled in as a git dependency pinned to a tag and
> patched to the audited copy in `vendor/piscis-engine`.
>
> **This document describes the actual implementation in the AgentZ repository**, not a vision. The
> milestone table in §9 is authoritative for what is "done / planned".

---

## 1. Positioning and key decisions

AgentZ is a **standalone repository** that consumes `piscis-engine` only as a **shared agent kernel**:

- Reused from the kernel: the agent loop (`piscis_kernel::agent`), the multi-model LLM client,
  the neutral tool set (file/shell/code_run/search/plan… in `piscis_kernel::tools`),
  the policy gate (`PolicyGate`), compaction v2 (rolling summary + state frame),
  and the headless turn runner (`piscis_kernel::headless`).
- Implemented by AgentZ itself (host layer): the multi-mode frontend, the IDE workspace (Monaco/LSP/terminal/Git/watcher),
  inline AI (Cmd-K / Tab), the codebase index, `.vsix` contribution ingestion and the extension host, worktree task isolation,
  sub-agent delegation, skills/rules/hooks, and Repo Wiki.

### 1.1 Pool/Koi multi-agent orchestration

The kernel contains a complete **Pool/Koi multi-agent orchestration** (coordinator, named persistent agents, shared task
board, `@mention` dispatch, sub-agent runtime). The original design deliberately left it off in the editor-type chat.
**Current state:** the host wires it into every agent-mode turn — `build_tool_registry` receives
`enable_pool = chat_mode != "plan"` and, when enabled, passes the Koi runtime and pool event sink from
`runtime::koi::pool_wiring`; Plan mode still runs without it:

```498:515:src-tauri/src/commands/chat_turn/tool_approval.rs
    let (subagent_runtime, pool_event_sink) = if enable_pool {
        let (rt, sink) = crate::runtime::koi::pool_wiring(&app);
        (Some(rt), Some(sink))
    } else {
        (None, None)
    };

    let mut handle = new_tool_registry_handle();
    let cfg = NeutralToolsConfig {
        db: Some(db),
        settings: Some(settings),
        builtin_tool_enabled,
        user_tools_dir,
        event_sink: Some(event_sink),
        plan_store: Some(plan_store),
        pool_event_sink,
        subagent_runtime,
        coordinator_config: Default::default(),
```

The original rationale still explains the CodeZ defaults: Pool/Koi is designed for "several long-lived agents collaborating on a
board", which is **heavy for an editor-type AI IDE** (it needs a board UI, named-agent lifecycles, heartbeat/watchdog/reconciliation).
The team/board experience therefore lives in WorkZ and BotZ (§12), while CodeZ leans on the lighter pair:

1. **Sub-agent delegation** (`delegate` / `call_fish` / `task`, §5) — for "parallel research, isolated context".
2. **`agent_task` git worktree isolation** (§6) — for "autonomous tasks that don't interfere and can be reviewed".

---

## 2. Product shape: one kernel, two modes

```
┌──────────────────────────────────────────────────────────┐
│  AgentZ                 [ IDE mode ]      [ Agent mode ]    │
├──────────────────────────────────────────────────────────┤
│  IDE mode (≈ Cursor)            Agent mode (≈ Codex)        │
│  Editor-centric:                Task-centric:               │
│  · Monaco + LSP                 · Submit goal → autonomous  │
│  · Tab completion / Cmd-K       · Isolated worktree/branch  │
│  · Right-hand AI chat + @refs   · Plan→edit→run tools→iterate│
│  · Inline diff / Review·Undo    · Task list + diff review   │
└──────────────────────────────────────────────────────────┘
   Shared: piscis-engine kernel (agent loop / tools / LLM / compaction / policy)
```

- **IDE mode**: human in the loop, low latency, incremental edits inside the editor; the agent is a "co-pilot".
- **Agent mode**: autonomous and long-running once given a goal; produces a reviewable diff in an isolated worktree
  that you then merge / open a PR for / discard.

Both reuse the same agent loop; only the **tool surface + context + output form** differ. Within a chat, AgentZ switches the
tool surface with `chat_mode` (`agent` / `plan`; any other value is treated as `agent`) (see §4.3).

> Today the title bar switches between CodeZ (IDE mode), WorkZ and BotZ (`type Mode = "codez" | "workz" | "botz"` in `src/App.tsx`).

---

## 3. Repository layout and kernel reuse

```
AgentZ/
├── Cargo.toml                  # Rust workspace; piscis-core / piscis-kernel / piscis-ide-tools via git deps (rev = v0.8.71), patched to vendor/
├── vendor/piscis-engine/       # audited source copy of the kernel crates
├── crates/agentz-host/         # kernel-link smoke binary
├── crates/agentz-task-state/   # task state crate
├── extension-host/             # Node VS Code extension host sidecar
├── src-tauri/                  # Tauri desktop host
│   └── src/
│       ├── lib.rs              # builder + command registration (capability index)
│       ├── state.rs            # AppState (terminals / watcher / LSP / browser / agent_slots …)
│       ├── context_assembly.rs # history rebuild + rolling summary + state frame + budget trimming
│       ├── journal.rs          # file journal (per-turn Review / Undo)
│       ├── lsp/                # LSP ↔ WebSocket bridge
│       ├── browser/ runtime/ remote/ gateway/ skills/
│       ├── tools/              # delegate / task / call_fish / codebase_search / graph_search / plan_mode_ui / chat_ui* …
│       └── commands/           # ide / chat / chat_turn/ / edit / codebase / agent_task / botz / teams / workflow /
│                               #   vsix / ext_host / dap / session / settings / clawhub / workbench /
│                               #   repo_wiki / graph* / interactive / journal / platform …
└── src/
    ├── App.tsx                 # multi-mode top-level shell (CodeZ / WorkZ / BotZ)
    ├── services/tauri/         # ide / lsp / chat / agentTask / botz … IPC
    ├── extensions/             # renderer side of the extension host (RPC, debug, webviews)
    └── workspaces/
        ├── codez/              # IDE workspace + AssistantPanel + ExtensionsPanel
        ├── workz/              # teams, CollabBoard, Agent tasks (goal → autonomous run + review)
        └── botz/               # rooms and bots
```

Key kernel-reuse path: `run_agentz_turn` in `commands/chat_turn/mod.rs` assembles the tool registry
(`build_tool_registry` in `chat_turn/tool_approval.rs`), system prompt, policy gate and harness
(`HarnessConfig::for_scheduler`) on the AgentZ side, then runs the kernel agent loop and streams
`AgentEvent`s to the frontend over Tauri event channels.

---

## 4. IDE mode (≈ Cursor)

### 4.1 Editor foundation (M0, done)
Monaco editor + full IDE commands: file tree/read/write, Git (status/diff/branches/add/reset/
discard/commit/checkout/create-branch), PTY terminal (xterm), file watcher,
LSP ↔ WebSocket bridge (`ide_lsp_*`). Opening any local folder enters the IDE (its own `projectDir`,
independent of any collaboration pool).

### 4.2 Three inline AI capabilities
- **Cmd-K inline edit (M2)**: selection → type an instruction → `inline_edit` (single-shot LLM transform, no agent
  loop) → a real inline diff in the editor (new lines green, original lines red) → Enter to accept / Esc to reject (undo).
- **Tab completion / ghost text (M5)**: `ai_inline_completion(file, prefix, suffix, cursor)`
  → lightweight completion rendered as ghost text by a Monaco inline-completions provider.
- **Chat sidebar + @references + Apply**: `AssistantPanel` drives `chat_send` → `run_agentz_turn`,
  a single agent turn streaming `AgentEvent`s (text + tool calls); the agent edits files directly and the IDE watcher
  picks them up live. `@references` are expanded host-side by `expand_file_refs` (`chat_turn/prompt_assembly.rs`):
  - `@path/to/file` → file content injected inline (12K-character cap per file, 48K total);
  - `@codebase` → runs the codebase index and injects the top hits inline (see §7).

### 4.3 Plan mode (two-level)

The CodeZ plan flow has two levels; plan files are always stored in `{project}/.agentz/plans/*.md`.

**Level 1 — Plan mode (`chat_mode = "plan"`)**

- Write/execute tools and `plan_todo` are disabled; the **only writable** path is `plan_write` → `.agentz/plans/*.md`.
- **Brainstorm first**: `plan_mode_ui` action=`brainstorm` clarifies requirements over several rounds as a questionnaire (multiple choice + free-form input) before calling `plan_write`.
- When the plan is complete, call `plan_mode_ui` action=`plan_ready`: shows a **Build** button and **pauses the agent loop**; when the user clicks Build, the chat switches to Agent mode and starts execution automatically.

**Level 2 — Agent mode (`chat_mode = "agent"`)**

- At the start of a complex task the agent may call `plan_mode_ui` action=`suggest_enter` to suggest entering Plan (if there is no answer within **30 s**, Agent mode continues).
- If a plan file exists, a plan summary is injected; during execution `plan_todo` is kept in sync and the plan and acceptance evidence are written back.

**Plan mode UI (`plan_mode_ui` / `chat_ui`)**

| action | Mode | Behavior |
|--------|------|----------|
| `suggest_enter` | Agent | 30 s prompt asking whether to enter Plan; timeout = continue in Agent |
| `brainstorm` | Plan | Multi-question form; blocks until the user submits |
| `plan_ready` | Plan | Build button + stop iterating |

### 4.4 Review / Undo
Each turn groups that round's file snapshots via `FileJournal::begin_turn` plus before/after-tool hooks;
`journal_list_changes` / `journal_undo_turn` support reviewing changes and undoing a whole turn.

### 4.5 Embedded browser and E2E automation

The CodeZ editor area provides a **Browser panel** (screenshot stream + element picking) that shares one CDP Chromium session
with the agent's `browser` tool (`robotz-browser` + `BrowserManager`).

**Tool layers**

| Layer | Tool | Purpose |
|-------|------|---------|
| Kernel | `web_fetch` | Lightweight read of static URLs |
| Host | `browser` | JS/SPA, clicks, snapshot/ref, assert, screenshots |

**Recommended E2E flow**: `navigate` → `lock` → `snapshot` → interact via `ref` → `assert_*` →
`screenshot(save_path=…)` → `unlock`. Plan mode disables `browser`; use `web_fetch` for static research.

**Panel sync**: after the agent calls `browser`, the host emits `browser-changed`; the frontend immediately refreshes the screenshot and address bar;
the first `tool_start(browser)` opens the Browser tab automatically. Closing the tab while the agent is active asks for confirmation.

**Regression scenarios**: `debug_scenarios_list` contains 4 browser scenarios. (The `codez-e2e-testing` skill mentioned in older releases is no longer shipped; the bundled preinstall pack has a generic `e2e-testing-patterns` skill.)

---

## 5. Multi-agent / sub-agents: `delegate` (done, M7) and successors

AgentZ's original multi-agent capability was the **`delegate` read-only research sub-agent**, not Pool/Koi.

- The main agent hands a well-scoped **read-only investigation** ("find every call site of X / explain how Y works /
  locate the config that controls Z") to a sub-agent. The sub-agent runs the kernel agent loop but with a
  **read-only tool surface** (explore + `codebase_search` + LSP, no write/shell/code_run), and it
  **cannot delegate further** (no recursion; depth stays at 1).
- Budget: iterations default to the `subagent_max_iterations` setting, or half of the main agent's `max_iterations` (at least 10) when unset;
  the timeout defaults to `subagent_timeout_secs`, or 900 s when unset (`run_subagent_with_options` in `chat_turn/tool_approval.rs`).
  At most 4 sub-agents run at once by default (`delegation::DEFAULT_MAX_CONCURRENT`, configurable).
- The sub-agent's process never enters the main agent's context; only its **final findings** come back as the tool result — keeping the main
  context clean.
- **Concurrency**: `delegate` is marked `is_read_only() = true`, so the kernel agent loop runs multiple `delegate` calls in the same
  round **in parallel** (read-only tools go into a concurrent batch; only write tools are serialized). This is
  AgentZ's version of "parallel sub-agent research".

```66:70:src-tauri/src/tools/delegate.rs
    fn is_read_only(&self) -> bool {
        // The delegated sub-agent is itself read-only, so delegating is safe to
        // run concurrently with other read-only tools.
        true
    }
```

- **Delegation modes**: in Plan mode only `delegate` is registered; in Agent mode `delegate`, `call_fish` (builtin "Fish" helpers) and
  `task` are registered according to the delegation mode setting (`commands/delegation.rs`).
- **Writable sub-agents (`task`)**: `task` delegates to a registered agent or builtin Fish using its manifest. A writable agent gets
  the agent-mode tools; in a git repo each writer works in its own worktree and is three-way merged back (conflicts are reported); outside git, writers are
  serialized (`tools/task.rs`, `runtime/subagent_worktree`).

**Boundary (original, vs. Cursor)**: when this section was written AgentZ had **no** concurrent "writing" sub-agents and no persistent named
agent pool / board / `@mention` orchestration. Both now exist (`task` above; WorkZ/BotZ in §12). For long-running parallel code changes, the
Agent-mode multi-worktree tasks (§6) remain the main path: one isolated worktree per task.

---

## 6. Agent mode (≈ Codex): worktree task isolation (done, M3 + M4)

The biggest difference from IDE mode: **task-based, autonomous, isolated, reviewable**. **Not a board/Pool**, but
"a task list + one isolated git worktree per task". (In the current UI these tasks live in WorkZ.)

### 6.1 Task lifecycle
```
Submit goal (prompt + project + optional base branch)
   │
   ├─ 1. Isolate: git worktree on a new workz/task-<id> branch
   │        (worktree at <project>/../.agentz-worktrees/task-<id>)
   ├─ 2. Autonomy: run run_agentz_turn (full agent-mode tools) — plan→edit→run tools→iterate
   ├─ 3. Streaming: steps (text + tool calls) stream to the frontend; Stop cancels the in-flight turn
   ├─ 4. Review: the Changes panel shows files changed in base...branch + per-file side-by-side diff
   └─ 5. Finish: merge (no-ff) / one-click PR (gh) / discard (remove worktree + branch)
```

Related commands (`commands/agent_task.rs`): `agent_task_create` / `_list` / `_get` /
`_changed_files` / `_file_diff` / `_change_set` / `_file_view` / `_merge_preview` / `_merge` / `_open_pr` / `_discard`,
plus snapshot commands (`_archive_snapshot` / `_restore_snapshot` / `_rollback_snapshot`). The main worktree is never
written directly, so a misbehaving agent cannot pollute the user's checkout.

### 6.2 Parallel tasks
Multiple Agent tasks each get their own worktree + branch + kernel session and can be committed and reviewed independently — naturally
isolated in parallel. **This is where AgentZ's "multi-agent parallelism" lands** (task-level parallelism, not multiple agents within one task).

---

## 7. Codebase index (done, M5)

- Implementation: `commands/codebase.rs` splits source files into **50-line windows** and stores them in
  `{project}/.agentz/index.db` (SQLite).
- Retrieval: **keyword / term-frequency ranking + path-name boost** (not vector embeddings); no embedding API key needed.
  The schema is deliberately simple so an embedding column can be added later, reusing `piscis_kernel::memory::vector`.
- Exposure: the `codebase_search` tool (for the agent) + `@codebase` inline recall (for IDE chat).
- Building: `codebase_index_build` does a full rebuild; a query against an empty index builds it lazily; the watcher can
  call `index_file` for single-file incremental updates.

---

## 8. Skills / rules / hooks / MCP (done, M6)

- **ClawHub skill marketplace**: `clawhub_search` / `clawhub_install`; installs into `{config}/skills/installed/`.
  At turn start, skills are injected with "progressive disclosure" as `## Available skills` (name + summary only; when a skill matches the task, the
  agent first `file_read`s the full `SKILL.md`). Executable skills live in `{config}/user-tools/`.
- **Skill evolution (v0.5.0)**: four-quadrant storage (`installed/`, `.draft/`, `learned/`, `.archive/`);
  controlled writes through `skill_manage`; post-turn background review + memory extraction; Curator maintenance while idle;
  the global `{config}/piscis.db` stores skill metadata/revisions (separate from the session DB).
- **Project rules**: `*.md` / `*.mdc` under `{workspace}/.agentz/rules/` (preferred) or `.cursor/rules/` (compatibility)
  are concatenated into `## Project rules` and injected as system constraints.
- **Hooks**: shell-command hooks in `{project}/.agentz/hooks.json` (10 s timeout, project root, `AGENTZ_*` env). `beforeAgentTurn` output is injected as additional system context; a `beforeFileEdit` hook exiting non-zero blocks the edit (wired in `JournalWithIdeNotify::before_tool`); `afterFileEdit` (same hook, after the tool) and `afterAgentTurn` (spawned after a successful turn) are side-effect only.
- **MCP**: during turn assembly, `settings.mcp_servers` are connected (stdio/SSE) through `register_mcp_tools`
  and their tools registered.
- **Management surface**: `commands/workbench.rs` (list/uninstall skills, rules CRUD/enable/disable, read/write/run hooks).

---

## 9. Milestones and progress (actual status)

| Milestone | Content | Status |
|---|---|---|
| **M0** | Dual-mode shell + IDE workspace (files/Git/terminal/watcher/LSP) + Tauri host | ✅ Done |
| **M1** | IDE AI chat sidebar (single agent turn, streaming events, `@file`) | ✅ Done |
| **M2** | Cursor-like chat & editing: markdown rendering, session sidebar (list/new/delete/fork), message queue, Stop, Cmd-K inline edit | ✅ Done |
| **M2.5** | `.vsix` contributions: themes ✅ / snippets ✅ / LSP ✅; TextMate grammars ⏳ | 🟡 Partial |
| **M3** | Agent-mode task list: goal → autonomous run, streaming steps, task history sidebar, Changes review panel | ✅ Done |
| **M4** | Agent task isolation: git worktree + branch + diff review + merge/PR/discard | ✅ Done |
| **M5** | Codebase index (`.agentz/index.db`, keyword ranking) + `codebase_search` + `@codebase` + Tab completion | ✅ Done |
| **M6** | MCP servers + ClawHub skills + project rules + hooks + user-tools | ✅ Done |
| **M7** | SubAgent delegation: `delegate` read-only research sub-agent (concurrent, non-recursive) | ✅ Done |
| **M8** | Repo Wiki (`repo_wiki_generate`) + automatic model routing (`CODEZ_AUTO_MODEL_ROUTING`, plan→fast / agent→strong) | ✅ Done |

**Planned / not done**:
- Upgrade the index to real vector embeddings (reusing the cosine + hybrid search in `memory::vector`).
- TextMate grammars for `.vsix` (`vscode-textmate` + `vscode-oniguruma` WASM). (DAP debugging has since shipped: `commands/dap.rs`, `src/extensions/debug/`.)
- A headless / cloud long-running task runner and sandboxing (the kernel's `piscis-cli`/NDJSON infrastructure can pave the way).

---

## 10. VS Code `.vsix` compatibility (M2.5)

> Core fact: AgentZ uses **Monaco** (the editor core), not the full VS Code workbench.
> Original strategy: consume a `.vsix` as a "declarative contribution data package" and **not execute arbitrary extension JS**.
> Update: in addition, a clean-room Node extension host (`extension-host/`, brokered by `commands/ext_host.rs`) now runs real extension
> JavaScript against a `vscode` API implementation over an NDJSON RPC protocol; see [extension-compatibility-1.0.md](extension-compatibility-1.0.md).

`import_vsix` in `commands/vsix.rs` unzips the `.vsix`, reads `contributes.*` from `extension/package.json`
and dispatches it through an allowlist:

| Contribution | Status | How |
|---|---|---|
| Color themes | ✅ | `tokenColors` + workbench `colors` → `monaco.editor.defineTheme`, applied globally and persisted |
| Snippets | ✅ | `contributes.snippets` → registered as Monaco completion items per language |
| Language servers (LSP) | ✅ (existing bridge) | Reuses `ide_lsp_*` |
| TextMate grammars | ⏳ Planned | Needs `vscode-textmate` + `vscode-oniguruma` (WASM) |
| Commands / webviews / `vscode.*` API | Originally ❌ "won't do"; now provided by the extension host | Runs in the Node sidecar, not in `vsix.rs` |

Theme syntax colors are approximate (TextMate scopes and the Monaco tokenizer are not 1:1); workbench colors carry the main palette.

**Licensing boundary**: users **bring their own `.vsix`** or use **Open VSX**, avoiding the VS Code Marketplace ToS.

---

## 11. Key risks and trade-offs

1. **Latency vs. quality**: Tab/Cmd-K use a single-shot lightweight path separate from the large models of Chat/Agent; M8's automatic
   model routing (plan→fast, agent→strong) is opt-in (`CODEZ_AUTO_MODEL_ROUTING=1`, needs at least two providers) and does not silently override the user's choice by default.
2. **Index cost/precision**: keyword ranking rather than semantic embeddings — zero dependencies and zero cost, but weaker recall than vectors;
   the schema leaves room to upgrade. Large repos are kept in check by ignored directories + 50-line chunks + caps (depth 12, 20,000 files).
3. **Multi-agent collaboration**: WorkZ offers two paths — **swarm**, driven by coordinator turns + `pool_org`/`pool_chat`, with wave order constrained by `org_spec` + `depends_on` + `workflow_hint`, patrol recovering stale/pending todos, and heartbeat launching a light coordinator turn when there are open todos; and **workflow**, executed step by step by a deterministic graph driver, with an independent pool per run so transcripts don't mix. Concurrency otherwise still comes from sub-agent delegation and multi-worktree task isolation.
4. **Autonomy boundary**: Agent tasks are fully isolated on worktree branches, so the main branch is never polluted directly; the `delegate`
   sub-agent is read-only and non-recursive; writable `task` sub-agents work in their own worktrees when in a git repo.
5. **Diff-apply consistency**: Cmd-K inline diffs and agent edits both go through the editor/journal, and Review·Undo
   can roll back a whole turn.

---

## 12. WorkZ collaboration (swarm vs. workflow)

| Mode | Driver | Convergence |
|---|---|---|
| **swarm** | Coordinator LLM turns + `pool_org` / `pool_chat` | Coordinator system protocol (`get_todos` → handle blocked/needs_review → `post_status`); member todo turns use the kernel's six-layer prompt contract ending with the **Stop Gate**; patrol recovers stale work; heartbeat adds a coordination round when attention is needed |
| **workflow** | Graph driver in `runtime/workflow` | Step/loop caps + driver mutual exclusion; an LLM branch with no match and no `default_to` fails |

**Waves**: not implemented by a separate scheduler — the coordinator is constrained in its system prompt by `workflow_hint` (waves / sequential / review); the actual order comes from `depends_on` and patrol activating pending todos.

---

## 13. CodeBuddy preinstall and slash commands (v0.6.0)

On first launch, `seed_preinstall_packs` copies `bundled/preinstall/` (generated by `npm run import:codebuddy`)
**non-destructively** into the global config directory:

| Type | Target path | Sentinel |
|---|---|---|
| Skills | `{config}/skills/installed/<slug>/` | `.preinstall-codebuddy-v1` |
| Agents | `{config}/agents/<id>/agent.json` | same |
| Slash commands | `{config}/commands/<id>/command.json` | same |

When the composer receives `/slash_id args`, `slash_commands_resolve` expands the prompt (substituting `$ARGUMENTS`) and
can optionally restrict the builtin tool set. Manifests support `description_zh` / `prompt_zh`; the frontend passes `prefer_zh`
into the chat turn based on the i18n locale.

When you open a project folder that has no `.agentz` yet, the UI offers a **project template** picker (rules + hooks, write-if-absent;
CodeBuddy hooks are converted into AgentZ `hooks.json` with `enabled: false` by default).

CI: `npm run verify:preinstall -- --strict` and `npm run lint:preinstall -- --strict` check bundled compatibility and manifests. See `THIRD_PARTY_NOTICES.md`.

### Compatibility grades (PASS / REWRITE / DELETE)

| Grade | Meaning | Handling |
|-------|---------|----------|
| **PASS** | Valid schema, all tool ids in the builtin catalog, unique `slash_id` | Kept in `bundled/preinstall/` |
| **REWRITE** | Contains CodeBuddy/Claude terminology or mappable tool names | Rewritten into AgentZ terms automatically by `remediate-preinstall` (`npm run remediate:preinstall`) / on import |
| **DELETE** | Empty prompt, duplicate slash_id/persona (lower score), hard dependency on a proprietary MCP/platform | Written to `bundled/codebuddy/exclude.json` and removed from the preinstall tree |

Audit output: `bundled/codebuddy/compatibility-report.json` (`npm run lint:preinstall`).

**Pool isolation**: `pool_sessions.team_id` isolates pools per team; workflow runs additionally get a `workflow_run_id`, and each run creates its own pool.

**ToolContext**: coordinator turns bind `workz_pool_id` to `pool_session_id`, so `pool_org` can still resolve the pool when `pool_id` is not passed explicitly.
