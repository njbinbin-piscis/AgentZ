# Background Tasks, Turn State and the Message Stream: Findings and Redesign

**English** | [简体中文](background-tasks-and-turn-stream.zh-CN.md)

Status: draft, pending review. Date: 2026-10-27. Re-verified against the code on 2026-11-02 (AgentZ 1.0.40, piscis-engine v0.8.71); see "Implementation status" below.

This document is based on a full read of the code at the time (line numbers refer to the 2026-10-27 workspace unless stated otherwise). It records confirmed defects, open problems, the proposed target design and a phased implementation plan. Conclusions marked "confirmed" are backed by code evidence; conclusions marked "suspected" need instrumentation or a reproduction to settle.

### Implementation status (2026-11-02)

Several parts of the target design have since landed. The investigation sections below are kept as the historical record, with inline notes where the code has changed.

| Item | Status in code |
|------|----------------|
| Unix detached reader threads bounded | Done: `spawn_std_detached` waits at most 5 s for the reader threads (`tools/background_process.rs` L136, L216–222) |
| Job/service intent | Done, but as a `service: true` boolean next to `background: true` (`BgKind::Job` / `BgKind::Service`, `background.rs` L207–226), not as a `mode` field |
| Kills never trigger a continuation | Done: `should_notify` returns `false` for any killed task (`background_continuation.rs` L63–86) |
| A service only reports ready / unexpected exit; a job that already reported ready only reports failure | Done (`should_notify`) |
| Notice delivered into a running turn instead of a new turn | Done: `turn_inbox::try_deliver` (`background_continuation.rs` L297–313); unconsumed notices are re-routed by `on_turn_closed` |
| Kill background processes on app exit / session delete | Done: `kill_all()` on exit (`src-tauri/src/lib.rs` L498), `kill_session()` on session delete (`commands/session.rs` L1263) |
| Tool-call timeout vs shell foreground wait conflict | Fixed: the loop timeout is now the tool's own timeout + 30 s slack (`agent/loop_.rs` L55–75) |
| Remote background tasks | Supported: `remote_dev` accepts `background` / `service` (`src-tauri/src/tools/remote_dev.rs` L131–160, `tools/remote_background.rs`) |
| Session scheduler, `session_state` event, stop by `turn_id` | Not implemented |
| Output written straight to files; handle-inheritance lockdown | Not implemented |
| `task_wait` / `task_output` / `task_stop` / `task_list` naming | Not implemented (tools are still `wait`, `process_wait`, `process_output`, `process_kill`) |
| Event sequence numbers, single reducer, segment-based persistence | Not implemented; CodeZ still records `textOffset` without flushing first |

---

## 1. Background: recently observed problems

| # | Symptom | Current assessment |
|---|---------|--------------------|
| P1 | Sending a message fails with `task queued/running or goal mutation active`, while the UI shows no running session | Confirmed: the continuation turn started automatically after a background task finishes (`host-bg-*`) holds the session gate but is invisible to the frontend (1.0.27 added `host_turn_started` as a mitigation) |
| P2 | After a background task starts a dev server, the agent prints one sentence in the same turn and then hangs forever; once the server is closed, the agent keeps iterating | Strongly suspected: after the foreground command ends we wait for pipe EOF, but the write end is held by a process outside the Job (the server). A 5-second cap has been added; not verified by reproduction |
| P3 | Pressing stop takes almost a minute to take effect | Not located. The bounds provable from code are about 8 s (tool grace) + 10 s (cancellation confirmation); the rest of the time is unexplained |
| P4 | Assistant text is cut mid-word by tool cards, while later paragraphs have no tools between them | Root cause not located (see §4.3). "Switching sessions" ruled out (user confirmed no switch) |

These problems share one trait: **state is spread across several places that advance independently, with no single source of truth**. Process state lives in the kernel registry; turn state lives in the gate + a frontend in-memory set + the database session row; the message-stream position is inferred from a frontend counter. Any of them falling out of sync shows up as a "hang" or "garbling".

---

## 2. Investigation of the current state

### 2.1 Process launch paths

| Path | Location | Output | Process-tree control | Bounded EOF wait? |
|------|----------|--------|----------------------|-------------------|
| Windows sandbox (`shell` / `code_run`) | `vendor/piscis-engine/piscis-kernel/src/proc_windows_sandbox.rs` `spawn_sandboxed_command` L451–638 | Anonymous pipes, `CreatePipe` with `bInheritHandle: true` (L517–524); the child inherits only three handles via `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` (L538–559) | Job + `KILL_ON_JOB_CLOSE` (L505–515) | Originally unbounded (`t.join()`); now `CAPTURE_DRAIN_TIMEOUT = 5s` (L862) |
| Unix background (`shell`/`code_run` only allow `background: true`) | `tools/background_process.rs` `spawn_std_detached` (L136 as of 2026-11-02) | Pipes | `setsid` new session, `killpg` | Was **unbounded** (reader-thread `join()`). *Update 2026-11-02: now bounded to 5 s per drain (L216–222)* |
| `process_control` launch | `tools/process_control.rs` L204–237 | Pipe or null | No Job; not tracked at all without a registry | Unbounded |
| IDE terminal | `src-tauri/src/commands/ide.rs` L1706–1895 | PTY | Windows Job | N/A |
| Others (git, LSP, rg, remote, …) | `piscis_kernel::proc::tokio_command`/`std_command`, a few direct `Command::new` | Varies | None | — |

**Confirmed structural problem**: the child-side pipe handles of the sandbox are briefly "inheritable" in the parent process. The sandbox's own `CreateProcessW` uses a handle allow-list, but any process started elsewhere in the same process with `std::process::Command` (on Windows the Rust standard library uses `bInheritHandles = TRUE` and no allow-list) that happens to land in this window inherits those write ends. The inheritor is not in the command's Job, cannot be killed when the command ends, and so the read end never sees EOF.

This is consistent with P2's "it continues only after the server is closed", but the timing (whether the server really started inside that window) is unproven. Another way to get P2 is a command that itself starts a process outside the Job (for example detaching via `start` or a service manager).

### 2.2 Background task model (kernel)

The core is in `vendor/piscis-engine/piscis-kernel/src/background.rs`:

- `BackgroundRegistry`: by default at most 4 per workspace (`AGENTZ_BG_MAX_PER_WORKSPACE`), default `max_runtime` 3600 s (`AGENTZ_BG_MAX_RUNTIME_SECS`).
- `BgEntry`: session, workspace, label, `notify_on` (`Exit`/`Ready`), `ReadySpec` (`Port`/`LogSubstring`), kill flag, `SharedStream` for stdout/stderr (256 KiB in-memory ring + 8 MiB log file `.agentz/logs/bg/{id}.log`).
- Events: `BgEvent::Ready` from `probe_ready`, `BgEvent::Terminal` when the execution thread ends.
- **Registry entries were never removed** (no `remove`); they only switch to a terminal state. *Update 2026-11-02: terminal entries beyond `MAX_RETAINED_TERMINAL = 64` are now pruned (`background.rs` L231, L556–557). Entries also carry a `kind` (`BgKind::Job` / `BgKind::Service`); services get an effectively unbounded runtime (`SERVICE_MAX_RUNTIME` = 30 days, L229).*

Tools visible to the agent (descriptions taken from the schemas):

| Tool | Behaviour |
|------|-----------|
| `shell` / `code_run` | Foreground default 120 s / 60 s, max 300 s; "on timeout the command moves to the background registry and is not killed"; `background: true` goes to the background immediately; accepts `ready_port`/`ready_log`/`notify_on`, and (added since) `service: true` for long-lived processes |
| `wait` | Host-side one-shot wait for background tasks to finish or become ready, default 120 s, max 600 s; can also be a plain delay |
| `process_wait` | Single-task wait, default 60 s, max 600 s |
| `process_output` | Incremental output reads by cursor |
| `process_kill` | Kills the process tree, waits up to 30 s for confirmation |

The **capabilities** of this tool set are already close to the target design; the problem is unclear semantics and higher-level policy (see 2.4, 2.5).

### 2.3 Timing parameters

| Mechanism | Default | Location |
|-----------|---------|----------|
| Single tool-call timeout | 120 s (2026-10-27). *Now: the tool's own `timeout`/`timeout_secs`/`timeout_ms` + 30 s slack, at least 120 s, capped at 3600 + 30 s; 150 s when the call sets none* | `agent/loop_.rs` L55 `TOOL_TIMEOUT_SECS`, L61–75 `tool_timeout_for` |
| shell foreground wait | 120 s (≤300 s) | `shell.rs` L21–23 |
| code_run foreground wait | 60 s (≤300 s) | `code_run.rs` L14–15 |
| Cancellation grace | 8 s | `loop_.rs` L58 `CANCEL_GRACE_SECS` |
| Cancellation confirmation | 10 s | `post_turn.rs` L527–528 |
| Total turn duration | ~1 year when unset | `chat_turn/mod.rs` L856–858 |
| Max background runtime | 3600 s (jobs); 30 days for services | `background.rs` L28, L229 |
| Continuation parking | 30 min, polled every 2 s | `background_continuation.rs` L30 |
| Automatic continuation budget per session | 6 | `background_continuation.rs` L28 |

**Confirmed conflict (fixed since)**: the tool-call timeout (120 s) equalled the shell foreground default wait (120 s). If the tool layer timed out first, the sandbox worker could still be in `wait_foreground`/`finish`, interrupting the foreground-to-background hand-off. *The 30 s slack in `tool_timeout_for` now gives the tool time to finish moving the command to the background.*

### 2.4 Automatic continuation after a background task finishes

`src-tauri/src/runtime/background_continuation.rs`:

1. `on_event` emits a `background_task` UI event, then runs `handle_completion` asynchronously.
2. With `notify_on == Ready`, readiness triggers a continuation; any task ending triggers one. *Now gated by `should_notify` (see below).*
3. `try_claim("{bg_id}:{kind}")` deduplicates; `claim_budget` takes from the budget (6 per session, reset by any user turn).
4. A synthetic `<background_task_completed>` prompt is built, persisted as pending, and `dispatch_when_idle` polls until the gate is idle, then starts a full agent turn with `task_key = host-bg-{bg_id}`. *Now: if a turn is already running in the session, the notice is first delivered into that turn via `turn_inbox::try_deliver` and a `background_task_injected` event is emitted; only notices the turn never consumed are re-routed to a continuation turn (`on_turn_closed`).*

Confirmed defects (with status as of 2026-11-02):

- **The continuation turn is an ordinary turn**: it holds the session gate until it ends, and user messages are rejected meanwhile (P1). Since 1.0.27 the frontend shows it as running and queues via `host_turn_started`, but only CodeZ (`src/workspaces/codez/useAssistantSession.ts`) handles that event; WorkZ and other surfaces do not. *Still true.*
- **One service consumes the budget twice**: `ready` and `terminal` are separate claim keys, each charged once. *Mostly fixed: a service only reports `terminal` on an unexpected exit, and a job that already reported `ready` only reports a failed exit. The keys are still separate, so a ready-then-crashed service still uses two continuations.*
- **Inconsistent dedup key on restart restore**: `restore_pending` used a fixed `terminal` key (L336–337), which can cause duplicate continuations. *Now it uses `"{bg_id}:restored"` (L467), which is still a different key from `ready`/`terminal`, so the inconsistency remains.*
- **Services also trigger continuations**: a dev server being killed (even manually by the user) starts a new turn, which is exactly how the P1 incident was triggered. *Fixed: killed tasks never notify.*
- No background processes are cleaned up on app exit (no global kill of `background_runs` found); on the next launch the database marks them `interrupted`, but the OS processes may still be alive. *Fixed: `background_runs.kill_all()` runs on exit (`src-tauri/src/lib.rs` L498); pid/pgid-based cleanup of leftovers after a crash is still not implemented.*

### 2.5 Turn state: three unsynchronised "truths"

| State | Where | Written by |
|-------|-------|------------|
| Gate `TaskActivity` | Backend memory, one lock per `(project, session_id)` | `chat.rs` turn entry, heartbeat, plan changes |
| Session row `status = running` | Project database | Turn start/end; on read, `is_active` corrects only the "database says running but gate idle" direction |
| `SessionTurnBookkeeping.running` | Frontend memory | Set only when the frontend itself calls `chatSend` |

The frontend decides "queue or send now" using only its own copy. Any backend-initiated turn (continuation, heartbeat), page reload or second window makes the frontend think the session is idle, send directly, and be rejected by the gate. The `host_turn_started` added in 1.0.27 only covers the continuation source.

### 2.6 Message stream (CodeZ)

- Text deltas: `text_delta` goes into `streamPendingRef` and is appended to the last assistant message by `flushStreamDelta` every 80 ms; `streamLenRef` is incremented immediately on receipt (`useAssistantSession.ts` ~L754–759; L800–806 as of 2026-11-02).
- Tool position: on `tool_start`, `upsertToolStep(prev, evt, streamLenRef.current)` records `textOffset` (~L761–763; L808 as of 2026-11-02) **without flushing first**, and the offset is read from the ref when React runs the updater, not when the event arrives. *Still true.*
- Rendering: `AssistantMessageList.tsx` uses the live `toolSteps` for the running last message (replacing the persisted tools); `ToolTrace.tsx` `interleaveTools` splits text at `min(text.length, offset)`.
- History: `src-tauri/src/commands/session.rs` inserts `\n\n` when merging adjacent assistant rows; offsets use Rust `chars().count()` (Unicode scalar values) while the frontend splits with UTF-16 indices.
- Comparison: WorkZ (`useWorkzSessionStore.ts` L738–768) flushes before any non-`text_delta` event and uses the committed text's `length` as the offset; BotZ appends synchronously per event. Neither has this "counter ahead of displayed text" structure.

---

## 3. Reference model: Cursor's shell tool contract

The following is the external contract of the shell tool Cursor exposes to its agent (internal implementation is out of scope):

1. **"How long to wait" and "how long the process lives" are two parameters.** Each command carries a "maximum blocking time"; beyond it the command keeps running in the background and is not killed. Starting a server sets it to 0.
2. **Output goes to a file, not read from a pipe until EOF.** Each terminal's output is continuously written to a file the agent can read at any time. Callers never need to "wait for the pipe to close" to return.
3. **Monitoring is active and bounded.** A dedicated wait tool waits for a background task to finish, or for a pattern to appear in output, with its own timeout. It can also notify only when output matches.
4. **Completion notifies; it does not start a turn on its own.** A finished background task posts a notification consumed by the current or next agent turn; it never seizes the session itself.
5. **Stopping is explicit.** Long-running processes are not killed by wait timeouts; if a hard limit is needed the agent sets it in the command itself (e.g. `timeout`) or ends it explicitly.

AgentZ already has most of points 1, 3 and 5. It lacks point 2 (the lower layer still depends on pipe EOF) and point 4 (completion automatically starts a new turn). *As of 2026-11-02, point 4 is partly covered: notices go into a running turn when there is one; an idle session still gets an automatic continuation turn.*

---

## 4. Problem localisation status

### 4.1 P2: foreground command hang (mitigated, pending verification)

- Done: after the Job is empty, `finish()` waits at most another 5 seconds for EOF; on timeout it returns the captured output and logs a warning (`proc_windows_sandbox.rs` `join_capture`, L868).
- Not done: the reader-thread `join()` in `spawn_std_detached` is still unbounded; the handle-inheritance window still exists. *Update 2026-11-02: `spawn_std_detached` is now bounded to 5 s; the handle-inheritance window still exists.*
- Verification: when reproducing, check the log for "pipe still held open by a process outside the command's Job".

### 4.2 P3: slow stop (not located)

The provable upper bound is about 18 seconds; the user observed about 60 seconds. Candidates:

- A batch of concurrent read-only tools (`READ_TOOL_MAX_CONCURRENCY`) or serial write tools, each consuming its own grace period;
- Tools that do not check the cancellation flag (waiting on subtasks, blocking waits in `wait`/`process_wait`) can only wait for the grace period to expire;
- `finish()` after a foreground command ends (root exit 5 s + Job empty 5 s + EOF 5 s) runs outside the grace period;
- `cancellation_completion.wait_requested` running to its full limit.

Instrumentation needed: time of the cancel request, name and end time of every in-flight tool, time of `agent turn finished`.

### 4.3 P4: text cut by tools (not located)

Ruled out:

- Session switching (user confirmed no switch).
- Backend event reordering: the kernel sends "all text deltas for this round → tool start" in order (`loop_.rs` `llm_call_unified` L2228–2286, `ToolStart` in `execute_single_tool` L2505–2511), and the host collector forwards in order (`post_turn.rs` L323–435).
- Offset running ahead due to 80 ms coalescing: this only places the tool **temporarily** at the end of the flushed text; after the flush `text.length` catches up with the offset and the position corrects itself. That is flicker, not persistent misplacement.

Persistent misplacement as in the screenshot requires the offset to be **smaller** than its true position: either some text was never counted in `streamLenRef`, or `streamLenRef` was reset mid-turn while the text was kept. In the code, `streamLenRef` is reset only at turn start, on `host_turn_started`, and on session switch/creation; the text is replaced wholesale only by `syncMessagesFromDb` (turn end, continuation end, error paths). No path has been found that triggers these during a running turn without switching sessions.

Hypotheses to verify:

1. Error path: a message the user sends during a running turn is rejected by the gate; `runTurn`'s `finally` then calls `syncMessagesFromDb` when "the queue is empty and the session is being viewed", loading the running turn's persisted content without syncing the running turn's counter.
2. Two producers emitting events to the same session, e.g. delegation/subtask tool events interleaved with the parent turn.
3. A mid-turn model retry or fallback re-streaming output, so the count and the display diverge.

Instrumentation needed: for every `agent_event`, record the sequence number, type, `streamLenRef`, the last message's `text.length` and the current `taskKey`, plus every call site that replaces `setMessages` wholesale. Exporting this log at the next reproduction will settle it.

---

## 5. Target design

### 5.1 Principles

1. **The backend is the single source of truth.** Whether a turn is running, which one, and background task state are all pushed as snapshots by the backend; the frontend only renders and never infers.
2. **Waiting is separate from lifetime.** "How long the agent waits" never decides "how long the process lives".
3. **Never wait unboundedly.** Every wait (locks, EOF, process exit, turns) has a limit and an observable timeout result.
4. **Background events are inputs, not triggers.** A finished background task produces a notification; the turn scheduler decides when and how to hand it to the agent, rather than directly starting a new turn that seizes the session.

### 5.2 Process layer (proposal, not implemented)

- **All output goes to disk**: each process's stdout/stderr is redirected straight to a log file (on Windows a file handle as the child's standard handle; on Unix a `File`), and the host reads by following the file. A foreground command is "done" based solely on process (Job) state, no longer on pipe EOF. This removes the P2 class of problems at the root.
- **Lock down handle inheritance**: every process launch goes through the `piscis_kernel::proc` wrappers; on Windows always use `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`, and globally lock the "create inheritable handles → launch → close" window within a process; forbid bare `std::process::Command` (enforced by a CI lint/grep check).
- **Process-tree ownership**: always a Job on Windows, always a new process group on Unix; record pid/pgid so leftovers can be cleaned up after an app restart.

### 5.3 Task model: three intents

The agent must declare intent when launching a command; the tool schema expresses it with a `mode` field. *Implementation note (2026-11-02): implemented in a reduced form as `background: true` (job) plus `service: true` (service) booleans on `shell`, `code_run` and `remote_dev`; there is no `mode` field and no `max_runtime_secs` parameter (jobs use the registry default of 3600 s).*

| Mode | Purpose | Wait | Lifetime | On end |
|------|---------|------|----------|--------|
| `foreground` | One-off commands (build, test, script) | Up to `wait_secs` (default 60, max 300) | **Automatically becomes a `job`** past the wait time; not killed | Return the result |
| `job` | Long but bounded tasks | Return `bg_id` immediately | `max_runtime_secs` (required, default 1800, max 4h); the process tree is killed on expiry | Emit a `task_finished` notification |
| `service` | Long-running services (dev server, watch) | Return immediately, or wait until `ready` (port/log pattern, bounded) | No limit; bound to the session or app and cleaned up when the session closes or the app exits | Notify only on **unexpected exit**; no notification when the user/agent stops it |

Monitoring tools keep their current capabilities, with unified naming and semantics (proposal, not implemented):

- `task_wait(ids | scope, until = exit | ready | pattern, timeout_secs)`: host-side blocking wait that checks the cancellation flag (200 ms granularity) and returns one of "satisfied / timed out / cancelled".
- `task_output(id, cursor, max_bytes)`: read log-file increments.
- `task_stop(id)`: kill the process tree with a bounded confirmation.
- `task_list()`: snapshot of the current session's tasks.

The tool descriptions state explicitly: start services with `service`, do not start services with `&`/`start` in foreground commands; wait with `task_wait`, do not poll with `sleep`. When the host detects common backgrounding idioms in a foreground command it returns a hint instead of executing.

### 5.4 How background notifications reach the agent

Drop the default "task finished → start a new turn" behaviour and instead:

1. **During a turn**: the notification enters the session's notification queue and is attached to the model as a system message with the next tool result ("background task X finished, exit code Y, output tail …"). The agent handles it within the same turn without a new one. *Implemented via `turn_inbox` (2026-11-02).*
2. **Session idle** (not implemented; an idle session still gets an automatic continuation turn):
   - `job` finished: by default only show a notification card in the UI and the session, and let the user decide whether to "let the agent handle it"; an "auto-continue" setting enables continuation turns.
   - `service` unexpected exit: same, show only, no automatic continuation.
3. Automatic continuation turns and user turns share **the same scheduling queue** (see 5.5) and never bypass the queue to seize the gate. *Not implemented: continuations still poll the gate and retry on `task queued/running`.*
4. The dedup key is `bg_id` (at most one continuation per task); the budget is charged only when a turn is actually started. *Partly: the budget is charged in `park_and_dispatch` (only when a turn is about to be dispatched), but dedup keys are still per event kind.*

### 5.5 Turn scheduling and state sync (proposal, not implemented)

- **Backend session scheduler**: one FIFO queue per `(project, session_id)`. User messages, continuations and heartbeats are all enqueued; the head of the queue gets the gate. `chat_send` no longer errors when the gate is taken; it returns "queued" with a position. The gate becomes an internal detail of the scheduler.
- **Session state event**: the scheduler broadcasts `session_state { session_id, running: { turn_id, task_key, origin: user|bg|heartbeat, started_at }, queue: [...] }` on every change. The frontend's `running`/queue display is driven entirely by it; on startup the frontend calls `session_state_list` once for a snapshot.
- **Stop**: targets a `turn_id` rather than the `taskKey` the frontend remembers, so stop works for turns of any origin. A stop request immediately broadcasts a `stopping` state so the UI reacts at once.

### 5.6 Message stream (proposal, not implemented)

- **Events carry sequence numbers**: each turn's `agent_event` carries `(turn_id, seq)`. The frontend applies events in order and fills gaps from the persisted stream (`durable_stream`) instead of guessing.
- **Single reducer**: a turn's text and tools live in one state object advanced by a pure function `reduce(turnState, event)` (following WorkZ/BotZ):
  - `text_delta` appends to the current text segment;
  - `tool_start` closes the current text segment and opens a tool segment;
  - the next `text_delta` opens a new text segment.
  This removes the need for a "text offset". Display coalescing (80 ms throttle) affects rendering only, not reducer state.
- **Isomorphic persistence**: backend history is also returned as a "segment list" (text segments, tool segments), replacing `text_offset` and eliminating the `\n\n` and UTF-16 vs Unicode-scalar discrepancies.
- **Wholesale replacement only at turn boundaries**: `syncMessagesFromDb` is called only when no turn is running; when re-entering a session with a running turn, load the persisted segments first, then continue subscribing from the persisted stream's last sequence number.

---

## 6. Phased implementation

| Phase | Content | Risk | Acceptance |
|-------|---------|------|------------|
| 0 Instrumentation | Event log for §4.2 and §4.3; cancellation timing log; `join_capture` timeout warning already exists | Low | Next reproduction can locate P3/P4 from logs |
| 1 Stop the bleeding | Bound the reader-thread wait in `spawn_std_detached` too *(done)*; CodeZ flushes before non-`text_delta` events and captures the offset synchronously on arrival *(not done)*; stopping a service does not trigger a continuation *(done)*; `ready` and `terminal` share one continuation slot *(partly)*; fix the `restore_pending` dedup key *(not done)* | Low | Unit tests + manual scenarios |
| 2 Turn scheduler | Backend session queue, `session_state` event, stop by `turn_id`; frontend drops its own running inference and subscribes instead | Medium | User messages are queued, not rejected, whenever any backend turn is running; consistent across windows |
| 3 Process layer | Output to disk; handle-inheritance lockdown and lint; cleanup on app exit *(done)* and of leftovers after restart | Medium-high (cross-platform) | Foreground commands return on time even with holders outside the Job; no orphan processes |
| 4 Task model | `foreground/job/service` modes *(partly, as booleans)*, unified tool naming and descriptions, notification delivery attached within the turn *(done)* | Medium | The agent can start a service and keep talking, monitoring via `task_wait`/`task_output` |
| 5 Message stream | Event sequence numbers, single reducer, segment-based persistence | Medium | Text and tool order identical live and in history |

Phases 0 and 1 can run in parallel; phase 2 is the prerequisite for all later UI-state issues and should take priority over 3–5.

---

## 7. Test plan

- **Process layer** (Windows first): start an external process that inherits the foreground command's pipe write end, assert the foreground command returns within the limit; start a process outside the Job with `start` inside the command, assert no hang; assert no leftover processes after app exit.
- **Scheduler**: user sends while a continuation turn runs, assert it is queued and executed in order; stop works on continuation turns; after a frontend reload, state is restored from the snapshot.
- **Continuation policy**: a service stopped with `task_stop` produces no continuation; `ready` + `terminal` charge the budget once; restart restore does not duplicate continuations.
- **Message stream**: reducer unit tests for "text—tool—text", concurrent read-only tools, retry re-streaming and gap filling; live rendering and history loading produce the same segment list for the same turn.
- **Cancellation**: end-to-end stop time with several in-flight tools stays within a limit (target ≤ 10 s).

---

## 8. Open questions

1. Is a `service`'s lifetime bound to the session or to the project window? Does it survive a session switch? *(Current code: services live until killed, the session is deleted, or the app exits.)*
2. Should automatic continuation be off by default? Turning it off changes the experience for existing users.
3. With output on disk, what is the disk usage and rotation policy for large output (the current log cap is 8 MiB)?
4. Should remote development be part of the same model? *(Update 2026-11-02: `remote_dev` no longer rejects background tasks; it registers them in the same background registry via `tools/remote_background.rs`.)*
