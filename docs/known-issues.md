# Known Issues — Agent Loop Robustness & Tooling

Status: **partially fixed** — the host-side work landed in *Fix batch 1–3*
(see the end of this file). Kernel-bound items have a concrete *Upstream patch
spec* instead of an in-repo fix. Collected during the CodeZ chat-rendering
refactor. Fix these after the current task queue drains.

Owner: Piscis. Created: 2026-09-29.

---

## 1. `agent turn failed: timed out after 600s` kills the whole turn

**Status (2026-09-29).** Host-side portion **fixed** — see *Fix batch 1* at the end. Per-step recovery inside the agent loop (return the timeout to the model) is still kernel-side.

**Symptom.** Long agent turns (many tool calls, or one slow step) die with
`agent turn failed: timed out after 600s` and the user sees a hard error. All
in-flight tool work is discarded.

**Where it happens.**

- `src-tauri/src/commands/chat_turn.rs:1741` — `default_timeout = 600s`.
- `src-tauri/src/commands/chat_turn.rs:2235-2252` — the whole `agent.run(...)`
  future is wrapped in `tokio::time::timeout(timeout, run_fut)`. On `Err(_)` it
  sets `cancel`, returns `Some("timed out after {}s")`, and the turn is marked
  `failed` (`chat_turn.rs:2264`).
- `src-tauri/src/commands/chat.rs:198` — `agent turn failed: {e}` is returned to
  the frontend as a terminal error.

**Why it is wrong.** A wall-clock budget on the *entire* turn conflates three
different situations:

1. One tool call is slow (should be the tool's own timeout / result, not the turn's).
2. The model is waiting on something (permission / human verification) — not a failure.
3. The LLM HTTP request genuinely timed out.

Only (3) should ever surface as a turn-level failure, and only after retries.

**Expected behavior.**

- Turn timeout should not abort mid-step. Prefer cancelling the *current step*
  and returning a structured, non-fatal `tool_result` error to the agent so it
  can decide (retry, pick another path, or stop and explain).
- A single wall-clock cap on the entire turn should be removed or raised
  substantially; the real guards belong at the per-step level (LLM request,
  tool exec).
- When the turn is genuinely aborted, emit a resumable/partial result rather
  than a bare `failed`.

---

## 2. LLM request timeout should retry 3× before failing

**Status (2026-09-29).** **Not fixable in this repo** — the LLM call lives in the external `piscis-kernel` crate. Requires an upstream `piscis-engine` change (retry wrapper); the host can only supply config.

**Symptom.** An LLM HTTP read timeout bubbles straight up as a turn failure
instead of being retried.

**Current state.**

- `src-tauri/src/commands/chat_turn.rs:196-215` builds the client with
  `llm_read_timeout_secs` (`>= 30`). There is **no retry wrapper** around the
  LLM call in this repo.
- Retry logic exists only for unrelated paths: `clawhub.rs:73`
  (`clawhub_get_with_retry`, 3 attempts, exponential backoff) and
  `gateway/wechat.rs:994` (3 attempts). Nothing equivalent for the agent's LLM
  calls.
- Note: the actual agent loop lives in the external `piscis-kernel` crate
  (`Cargo.toml:23-25`, pinned `v0.8.62`) — any retry must be added there or in a
  wrapper we own.

**Expected behavior.** Wrap the LLM request in a bounded retry (3 attempts,
exponential backoff + jitter). Only after all attempts fail should the turn
report an error, and that error should be attributed to the LLM request — not
to the turn as a whole. Distinguish retryable (timeout / 5xx / connection
reset) from non-retryable (4xx / 402 insufficient balance) — the latter must
fail fast (`AssistantPanel.tsx` already special-cases 402).

---

## 3. Tool call timeouts must be returned to the agent, not raised

**Status (2026-09-29).** **Kernel-side, still open.** Tool execution + the loop that feeds results back live in `piscis-kernel`; the host cannot intercept a tool timeout.

**Symptom.** "有时是工具调用超时" — a tool call times out and the turn ends in
error instead of the agent being told the tool timed out.

**Expected behavior.** A tool timeout is a normal `tool_result` with
`is_error = true` and a machine-readable reason (e.g. `{"error":"timeout",
"timeout_secs":N}`). The agent loop must feed that back as the tool's output so
the model can retry, narrow the command, or change approach. It must never
terminate the turn. Same rule for non-zero exit codes and truncated output.

**UI note.** The inline `ToolTrace` component (`src/components/ToolTrace.tsx`)
already renders an `error` status row with the message in an expandable body —
so a returned tool timeout will render correctly once the backend stops
converting it into a turn failure.

---

## 4. Tool call that triggers a UAC prompt (no user response) must not hard-fail

**Status (2026-09-29).** **Partly host-side, still open.** The host owns the `permission_responses` map (`interactive.rs`) and could add a bounded wait, but the actual elevation wait happens in the kernel/tool. Needs a kernel change plus a host-side timeout policy.

**Symptom.** "甚至有时是工具调用触发UAC而用户没有响应" — a shell/tool call requests
elevation, the Windows UAC dialog appears, the user does not answer it, and the
turn errors out.

**Current state.**

- Permission plumbing is one-shot and in-memory:
  `src-tauri/src/commands/interactive.rs:23` `respond_permission_request`
  resolves a `oneshot::Sender<bool>` stored in
  `state.permission_responses` (`state.rs:52`). If nobody answers, the sender is
  simply never fired.
- `chat_turn.rs:2205-2211` emits `waiting_permission` when a
  `PermissionRequest` arrives, but there is no timeout/handling policy for it.
- The elevation itself happens inside the shell tool (external kernel); the
  repo only sees the resulting permission request / tool outcome.

**Expected behavior.**

- A UAC / elevation prompt that goes unanswered is **not** a turn failure. The
  tool should return `is_error = true` with a reason like
  `"elevation_prompt_unanswered"` (or a "requires elevation, user declined"
  result) so the agent can proceed without elevation, choose a non-privileged
  alternative, or explicitly ask the user.
- Add a bounded wait for `permission_responses`: if no response arrives within
  N seconds, resolve as "not approved" and hand a structured result back to the
  agent rather than hanging or aborting the turn.
- Never let UAC block the agent thread indefinitely.

---

## 5. Tooling: `file_edit` multi-line replacement fails on CRLF files

**Status (2026-09-29).** **Open** (tooling). Worked around again this session via a throwaway Python exact-once replacement script.

**Symptom (observed while editing this repo).** `file_edit` with a multi-line
`old_string` returns `match_not_found` even when the snippet is visibly present,
because the workspace files are CRLF on disk (`git ls-files --eol` → `w/crlf`)
while the tool matches LF-normalized text. Single-line edits and full
`file_write` work; multi-line edits are unreliable.

**Impact.** Slows down multi-hunk edits and caused several failed attempts in
this session (e.g. `AssistantPanel.tsx`, `TaskPanel.tsx`).

**Workarounds used.** (a) single-line edits frame-by-frame, or (b) a throwaway
Python script that reads with `newline=""`, joins the pattern with the file's
detected `\r\n`, and does an exact-once `str.replace`.

**Expected behavior.** `file_edit` should normalize line endings (or accept
CRLF/LF interchangeably) so multi-line `old_string` matches regardless of the
file's on-disk line ending. Until then, prefer single-line anchors or
`file_write` on this repo.

---

## 6. Verification gap in this environment

**Probed (2026-09-29).** `node`, `npm`, `npx`, `cargo`, `rustc`, `rustup`,
`cl`, `link`, `msbuild`, `winget`, `choco`, `scoop` are **all missing**. Present:
`git` (`C:\Program Files\Git\cmd\git.exe`) and `python` (3.12). There is no
`~/.cargo`, no `~/.rustup`, no `C:\Program Files\nodejs`, and no Visual Studio
installation — so **neither the frontend nor the Tauri backend can be built on
this machine**, and there is no package manager to install them with.

`node` / `npm` / `npx` are **not on PATH** and the repo has no `node_modules`,
so `npm run typecheck`, `eslint`, and `vitest` cannot be run from the agent
shell. Changes to TS/TSX are currently verified only by reading diffs. Consider
either (a) committing a vendored toolchain, or (b) documenting the exact
commands and requiring the user to run them, so "verified" claims can be backed
by real output.

---

## 7. Aborted turn (600s) drops plan/todo continuity for the next turn

**Status (2026-09-29).** Host-side portion **fixed** — see *Fix batch 1*. The store is still in-memory; cross-restart durability is not addressed.

**Symptom.** A turn dies from the 600s timeout while `plan_todo` items are still
`pending` / `in_progress`. The UI shows the `plan_todo` tool note
(*"请继续执行或将其标记为 cancelled … 只更新计划板，本身不算实际进展"*), but the next
user message is handled as if that note were never delivered — the agent starts
the new request and leaves the old todos dangling instead of deciding
(continue / cancel / clarify).

**Intended design (already present).**

- `src-tauri/src/commands/system_prompt.rs:194-211` — `active_todo_context()`
  renders the retained todos and instructs the agent to decide the relationship
  of the newest user message (preserve+continue / cancel with
  `plan_todo merge=true` / ask one clarification).
- `src-tauri/src/commands/chat_turn.rs:2021-2031` — on each turn, when
  `chat_mode == "agent"` and the session has `pending`/`in_progress` todos, that
  context is injected into the system prompt.

So a "reminder delivered to the agent" path exists — but the observed behavior
shows it did not take effect after an abnormal termination.

**Likely causes to investigate.**

1. The `plan_todo` UI note is **decoration, not a channel**. The only real
   re-engagement is the system-prompt injection above. If that injection is
   skipped, nothing re-asks the agent about the todos.
2. `plan_store` is **in-memory** (`state.rs:48`, `new_plan_store()`) and keyed by
   `session_id`. Any restart, session switch, or store reset loses the todos, so
   `active_todo_context` has nothing to inject (`chat_turn.rs:2022-2025` returns
   `unwrap_or_default()` → empty → no injection).
3. The timeout path (`chat_turn.rs:2235-2253`) sets `cancel`, returns
   `failed`, and **never reconciles `plan_store`**, nor emits an explicit
   "aborted with N open todos" signal. The aborted turn's open todos are left
   inconsistent with what the next turn is told.
4. `chat_mode` gating: injection only runs for `chat_mode == "agent"`. Any mode
   drift silently disables it.

**Expected behavior.**

- When a turn terminates abnormally with open todos, persist the retained
  todos (not just in-memory) and inject a **mandatory continuation directive** on
  the very next turn — or auto-resume — so the agent must explicitly continue,
  cancel, or ask. Silence must not be an option.
- The `plan_todo` reminder should be routed as a real turn directive rather than
  a passive UI note that the model never receives.
- The aborted turn should emit a lifecycle event carrying the open-todo count so
  the host can decide to resume.

---

## 8. Turn aborted mid-plan leaves `plan_todo` items `in_progress` forever

**Status (2026-09-29).** **Open.** *Fix batch 1* surfaces the open-todo count but deliberately keeps the items open so the next-turn reconciliation can fire; auto-marking is not yet implemented.

Direct consequence of #7, recorded separately because it is also visible in the
Todo panel: an `in_progress` item from an aborted turn is never flipped to
`cancelled`/`pending`. Combined with the Todo panel hiding when empty and
defaulting to collapsed (`src/components/TaskPanel.tsx`), a user can easily miss
that a stale item is still marked active. Consider marking open items
`cancelled` (or `pending`) on abnormal turn end, and surfacing "N unfinished
todos" in the panel badge.

---

## Fix batch 1 (2026-09-29) — host-side turn robustness

Scope: what can be fixed **inside this repo** without touching the external
`piscis-engine` kernel. Files: `src-tauri/src/commands/chat_turn.rs`,
`src-tauri/src/commands/system_prompt.rs`.

**Changed.**

1. **Turn timeout no longer aborts the call.** `run_agentz_turn` now records
   `turn_timeout_secs` and, on expiry, returns `Ok(HeadlessCliResponse { ok: false, .. })`
   with the partial `response_text` instead of `Err("timed out after 600s")`.
   This removes the `agent turn failed: …` hard error from `chat.rs:198` and
   keeps the session/streamed text intact.
2. **Lifecycle tells the truth.** The turn-level timeout used to be reported as
   `cancelled` (because the timeout path sets the cancel flag). It is now a
   distinct `"timed_out"` state, and the `agent_lifecycle` event carries
   `open_todos` — the number of `pending`/`in_progress` plan items still held
   for the session.
3. **Calm, actionable notice.** The `agent_final` error for a timeout is now
   *"This turn hit the Ns limit and stopped. Partial progress and open todos were
   kept — send a message to continue."* instead of a bare `timed out after Ns`.
4. **Retained-todo directive strengthened.** `active_todo_context` is now a
   **MANDATORY** reconcile-before-acting block that states the prior turn was
   interrupted/timed out and that no open item may be silently dropped.

**Still open / kernel-side (unchanged).**

- Tool-call timeout → returned to the agent (issue #3).
- UAC / elevation prompt unanswered → returned to the agent, not failed (issue
  #4).
- LLM HTTP timeout retried 3× before failing (issue #2).
- Per-step recovery: the deepest fix — a slow step should not consume the whole
  turn budget — needs the agent loop in `piscis-kernel`.
- Cross-restart durability of the retained todos (the store is in-memory).

**Verification.** These edits were **not compiled**: `cargo`/`rustc` are not on
this machine's PATH and the external git dependency is not vendored locally.
They follow existing in-file patterns, but `cargo check` must be run before
trusting them (the workspace denies warnings, so a stray warning is a hard
error).

---

## Fix batch 2 (2026-09-29) — newest message hidden in the chat history

**Symptom.** With the bounded/lazy-loaded message history, the newest message
went missing / was not visible.

**Root cause.** Two independent problems in `src/workspaces/codez/AssistantMessageList.tsx`
and its CSS:

1. **`content-visibility: auto` defeated bottom-anchoring.** `.agentz-msg-virtual`
   (`AssistantPanel.css`) used `contain-intrinsic-size: auto 160px`. Off-screen
   rows are laid out at the 160px estimate, so long markdown replies are
   massively under-measured. `scrollTo({ top: scrollHeight })` therefore landed
   *short*, and once real heights applied the true bottom moved down — pushing
   the newest reply below the fold. Repeated streaming deltas kept re-triggering
   it.
2. **The window was an absolute start index that could be re-anchored.** The
   trim effect (`Math.max(0, len - 50)` / `Math.min(prev, next)`) could adjust
   `visibleFrom` in ways that discarded rows the user had just loaded, and the
   scroll compensation used `scrollHeight - prevTop` rather than
   `scrollTop + Δ` (correct only when `scrollTop ≈ 0`).

**Fix.**

- **Bottom-anchored window by count.** `visibleCount` (default 50) replaces the
  absolute `visibleFrom`; rendered rows are `messages.slice(len - visibleCount)`.
  The newest message is structurally always mounted, and the window only grows
  backwards on scroll-up / "load older". The DOM stays bounded.
- **Dropped `content-visibility: auto`** from message rows (the mounted window is
  the real memory guard). The CSS rule is replaced by a NOTE explaining why.
- **Correct scroll anchoring** when loading older:
  `scrollTop = prevScrollTop + (newScrollHeight - prevScrollHeight)`.
- **Stick-to-bottom after layout** (`AssistantPanel.tsx`): the auto-scroll now
  runs inside `requestAnimationFrame` instead of synchronously, so it measures
  settled row heights.

**Verification.** Static only — see issue #6 (no `node`/`npm`, so no
`typecheck`). Reviewed by reading the diff; `visibleFrom` and `agentz-msg-virtual`
have no remaining references in `src/`.

---

## Fix batch 3 (2026-09-29) — no-click auto-resume after a timeout

**Goal.** After a turn-level timeout, continue the unfinished work without
requiring the user to click anything.

**Design.** The backend only *signals*; the frontend drives the continuation.
This keeps the Rust change to two JSON fields (no signature changes — important
because nothing here can be compiled), and reuses the existing queue/drain path.

**Backend** (`src-tauri/src/commands/chat_turn.rs`)
- The failure `agent_final` payload now carries `timed_out: bool` and
  `open_todos: usize`, alongside the existing `ok` / `error`.
- The timeout notice reads *"Turn exceeded its Ns limit — continuing
  automatically with the unfinished task."* and is cleared by the next turn's
  `setError(null)`.

**Frontend** (`src/workspaces/codez/AssistantPanel.tsx`)
- `agent_final` with `timed_out && open_todos > 0` enqueues a continuation turn
  (bounded by `MAX_AUTO_RESUMES = 2`). Because the event can land before
  `busy=false` commits, it is pushed onto the existing `queueRef` and drained by
  the `finally` block — no new scheduling path.
- `QueuedTurn.displayText` lets the bubble read *"（自动继续：上一轮超时）"* while the
  model still receives the full directive (`chat.autoContinuePrompt`). All
  `setQueuedView` call sites now use the `queuedLabels()` helper.
- The counter resets on every user-initiated submit, so the budget is per user
  request.
- **Stop still wins.** A user cancel ends the turn without `timed_out`, so
  auto-resume cannot fight an explicit cancellation.
- New i18n keys: `chat.autoContinuePrompt`, `chat.autoContinueLabel` (zh + en).

**Limits (by design).**
- Auto-resume lives in the CodeZ assistant panel, so it only fires while that
  panel is mounted (not for background WorkZ/heartbeat runs).
- Two consecutive auto-resumes maximum; a third timeout stops and reports.

**Verification.** Static only (issue #6). Reviewed via diff; all four
`setQueuedView(queueRef.current.map((q) => q.text))` sites were replaced (zero
remaining), and `autoResumeRef` is declared, guarded, and reset.

---

## Upstream patch spec — kernel-bound items (#2, #3, #4)

These cannot be fixed in this repo: the agent loop, tool execution, and LLM calls
all live in the external `piscis-kernel` crate (`Cargo.toml:23-25`, pinned
`v0.8.62`). This is the concrete change set to apply upstream in
`piscis-engine`.

### A. LLM request retry (issue #2)

- In the kernel's LLM call path, wrap each request in a bounded retry:
  **3 attempts**, exponential backoff (e.g. 1s, 2s) with jitter.
- Retry only on: read timeout, connection reset, 5xx, 429 (honour `Retry-After`).
  Do **not** retry 4xx (auth/validation) or 402 (insufficient balance) — those
  must fail fast; the UI already special-cases 402
  (`AssistantPanel.tsx` → `chat.llmBalanceError`).
- After the final attempt fails, return an error attributed to the **LLM
  request**, and let the host decide (it already returns a partial result for
  timeouts in `chat_turn.rs`).
- Reference implementation already in this repo for shape:
  `src-tauri/src/commands/clawhub.rs:73` `clawhub_get_with_retry` (3 attempts,
  backoff, `retry-after` handling).
- Host-side mitigation available **today** without a kernel change: raise the
  `llm_read_timeout_secs` setting (consumed at `chat_turn.rs:1966` and friends);
  the host owns the client but not the retry semantics.

### B. Tool timeout returned to the agent (issue #3)

- In the kernel's tool-dispatch path, a tool timeout must resolve to a normal
  tool result with `is_error = true` and a machine-readable body, e.g.
  `{"error":"timeout","timeout_secs":N}`, and be fed back to the model as that
  tool's output.
- It must never terminate the turn. Same treatment for non-zero exit codes and
  truncated output.
- The UI already renders this correctly: `src/components/ToolTrace.tsx` shows an
  `error` status row with the body in an expandable section.

### C. UAC / elevation prompt unanswered (issue #4)

- Add a **bounded wait** for the confirmation channel. The host owns the map
  (`src-tauri/src/commands/interactive.rs` → `state.permission_responses`,
  `state.rs:52`), but the `await` happens inside the kernel's tool.
- On expiry, resolve as **not approved** and return
  `is_error = true` with `{"error":"elevation_prompt_unanswered"}` so the agent
  can proceed unprivileged, choose an alternative, or explicitly ask the user.
- Never block the agent thread indefinitely on a UAC dialog.
- Host-side plumbing that must be honoured: `chat_turn.rs:2205-2211` emits
  `waiting_permission`; the kernel must time out that wait and continue.

### D. Per-step recovery / removing the whole-turn budget (issue #1, deepest)

- A slow step should consume the **step's** budget, not the turn's. Move the
  guard to per-step (LLM request timeout, tool timeout) and drop the single
  wall-clock cap on the entire `agent.run()`.
- Until then, the host behaviour is already improved: the cap no longer aborts
  the call, and the UI auto-resumes (Fix batches 1 and 3).

---

## Fix batch 4 (2026-09-30) — pending `chat_ui` card leaked into a new session

**Symptom.** A `chat_ui` form (e.g. the wizard from `plan_mode_ui` / any
interactive tool) is on screen; clicking **＋** (CodeZ *New chat*) or **＋ 新建**
(WorkZ *New task*) shows the same form again in the brand-new, otherwise-empty
session.

**Root cause.** Interactive cards are not stored in the message list — they live
in `src/hooks/useInteractiveCards.ts` (`cards` state), which is owned by the
panel component and therefore **survives** a session switch (the panel is not
remounted). Every reset path cleared messages / plan items / artifacts but not
the cards, so the only place that dropped them was the `projectDir`-change
effect. Concretely:

- `src/workspaces/codez/AssistantPanel.tsx` — `newSession`
  (`:896`) and `switchSession` (`:913`) called `clearSessionArtifacts()` but
  never `clearCards()`.
- `src/workspaces/workz/index.tsx` — `newTask` (`:761`) and `openTask`
  (`:789`) likewise reset the view without `clearCards()`.

WorkZ additionally filters incoming events to the bound foreground session
(`:446`), which is why the leak there only showed via a new/opened task rather
than via a background run.

**Fix.** Call `clearCards()` from every handler that abandons the current
conversation view, alongside `clearSessionArtifacts()`:

- `AssistantPanel.tsx`: `newSession`, `switchSession` (which also backs
  `fork` / `forkFromCheckpoint`), with `clearCards` added to both dep arrays.
- `index.tsx`: `newTask`, `openTask`, with `clearCards` added to both dep arrays.

`restoreToCheckpoint` is intentionally left alone — it stays in the same
session, so a card that still belongs to it should remain answerable.

**Known trade-off (follow-up).** A card is cleared even when the backend turn is
still *blocked* awaiting the `chat_ui` response (`interactive_responses` in
`state.rs`), so that orphaned request can only end via the turn timeout. A more
complete design would key each card by session id (the event envelope already
carries `session_id`) and render only cards for the active session, or
explicitly cancel the pending request when its session is abandoned. Out of
scope for this fix; the leak itself is fixed.

**Verification.** `tsc --noEmit -p tsconfig.json` → exit 0;
`eslint src/workspaces/codez/AssistantPanel.tsx src/workspaces/workz/index.tsx`
→ exit 0. Manual repro (open a `chat_ui` form → ＋) still needs to be confirmed
in a running app.

---

## Tool verification audit (2026-09-30) — run on the rebuilt app

**Build outcome.** The full `tauri dev` cold build succeeded: node v24.19.0,
cargo/rustc 1.98.1 (msvc), VS 2022 BuildTools, 437 crates downloaded, Rust build
**0 errors / 0 warnings** (matters — `Cargo.toml:6` denies warnings), `tsc --noEmit`
clean. The app launched and the live log shows the agent loop running on the new
binary (`agent loop starting` → `LLM response: ... tool_calls=5` → `executing
tool ...`), i.e. this audit ran *inside* the fixed build.

### Verified working

`file_read` / `file_write` / `file_edit` (with the CRLF caveat, #5),
`file_search` (glob + grep), `code_run`, `shell`, `process_control`, `recall`,
`chat_ui`, `web_fetch`/`web_search`. `call_fish` returns 3 sub-agents
(scout / summarizer / extractor). `delegate` works. `terminal_read` correctly
reports "no terminal sessions are running" when the IDE terminal is closed.

### Newly confirmed defects

**9. LSP bridge is unreachable.** `lsp diagnostics` on a Rust file returns
`Failed to connect to LSP bridge on port 60770: IO error 10061 (connection
refused)`. The bridge process is not listening; the tool leaks a raw OS error and
there is no retry or restart.

**10. `lsp` advertises languages it cannot serve.** The same tool reports
`Supported languages: rust, typescript, python, c/c++`, yet `lsp diagnostics` on
`.ts` and `.tsx` returns `No LSP server available for file`. `.tsx` is the
dominant type under `src/`. The list is produced by
`LspManager::supported_languages()` (external `piscis-ide-tools`), consumed at
`src-tauri/src/commands/ide.rs:1536` — so the advertised set and the real set
disagree.

**11. `lsp hover` never initializes.** Returns `LSP init: no response received` —
the initialize handshake times out.

**12. `read_lints` is a no-op for TS/TSX.** Returns
`no LSP server available for this file type`. It is re-exported from the external
crate (`src-tauri/src/tools/mod.rs:18`), yet the agent system prompt *instructs*
its use after edits (`system_prompt.rs:44`, `:103`). The guidance promises what
the tool cannot deliver.

**13. `codebase_search` ranks docs above code.** The query *"agent turn timeout
handling and auto resume"* returned **`docs/known-issues.md` as the top 5 hits**,
ahead of the implementation in `chat_turn.rs`. The embedding index includes docs
and weights prose heavily, and offers no kind/path filter.

**14. `graph_explore` has no blast radius for Rust.** For `chat_turn.rs` (which is
imported by `chat.rs` and `heartbeat.rs`) it reported
_No indexed importers in 1-hop (Phase 0: import edges only)_. Impact analysis —
the tool's headline feature — does not cover the Rust backend, precisely where the
riskiest edits happen. Consistent with the graph warning "many isolated modules
(no cross-deps)".

**15. Kernel runtime warnings observed live** (upstream, but real):
- `sanitize_tool_use_result_pairing: stripping unsatisfied ToolUse ids [...] (satisfied={})`
  — **repeated ~4x per request, with `satisfied={}` (nothing ever matched)**.
  Tool-use/tool-result pairing is effectively broken, so the sanitizer strips tool
  calls out of history on every request.
- `Checkpoint stale for session ... (base hash/count mismatch); ignoring` —
  checkpoints silently fail to restore.

**17. Two disconnected LSP bridges; the frontend one is dead code.** The
frontend keeps a WebSocket LSP session pool (`src/services/tauri/lspSession.ts`),
and its unit tests assert `ws://127.0.0.1:9999` (all 38 `vitest` tests pass). The
agent's `lsp` tool, however, dials the Rust bridge on port **60770** and gets a
refused connection. So there are two LSP paths — a TS WebSocket pool pointed at a
port with nothing behind it, and a Rust bridge that never starts — and neither
serves the agent. The editor may drive its own path; the agent-facing one is
non-functional.

**16. `delegate` sub-agents are workspace-restricted.** Investigating the LSP
bridge required reading the `piscis-ide-tools` crate inside
`~/.cargo/git/checkouts`, which the sub-agent could not access. For a repo whose
engine is a git dependency, this is a structural blind spot: the most important
code is invisible to delegated research.

### Why the agent silently avoids these tools (root cause)

The system prompt instructs `read_lints` after every substantive edit
(`system_prompt.rs:44,103`) and recommends `graph_explore`/`codebase_search` for
structure. But:

- `read_lints` returns **nothing useful** for TS/TSX (defect 12).
- `lsp` fails with a raw OS error for Rust (defect 9) and a contradiction for TS
  (defect 10).
- `graph_explore`'s blast radius is empty for Rust (defect 14).
- `codebase_search` returns docs instead of code (defect 13).

An agent that tries these once and gets an error or an empty result learns to
route around them — which is exactly the observed behaviour (falling back to
`grep` / `file_read` / `file_edit`). **The avoidance is a symptom of the tools
being broken or low-signal, not of poor tool design in the abstract.** Fixing the
tools (below) is what makes an agent actually use them.

### Programming-support improvement backlog

**P1 — make the IDE tools trustworthy**
1. LSP bridge lifecycle: start on demand, health-check, retry/restart; replace the
   raw `os error 10061` with an actionable message.
2. Wire a real TypeScript server and map `.ts/.tsx/.js/.jsx/.mjs/.cjs` →
   `typescript` / `typescriptreact`. Until then, drop `typescript` from
   `supported_languages()` so the advertised surface matches reality.
3. Give `read_lints` a fallback: when no LSP server exists, run the project's own
   checker (`tsc --noEmit`, `eslint`) and return those diagnostics. It is the
   single most-instructed tool and currently yields nothing for TS.
4. Fix the `lsp` initialize handshake (defect 11).

**P2 — make search and impact analysis useful**
5. `codebase_search`: exclude/de-prioritize `docs/**` for code queries; add a
   `kind` / `path_filter` parameter.
6. Index Rust `use` edges so `graph_explore` blast radius works for the backend,
   and resolve the "isolated modules" warning.
7. Expose "who imports this file" for Rust in the same shape TS already gets.

**P3 — agent-loop correctness (kernel)**
8. Fix `sanitize_tool_use_result_pairing`: `satisfied={}` means the pairing map is
   never populated; stripping every `tool_use` corrupts context.
9. Fix stale-checkpoint detection so restores do not silently no-op.

**P4 — ergonomics**
10. Surface `delegate` / `call_fish` when a task is result-heavy (nothing currently
    suggests them).
11. Allow `delegate` read-only access to dependency sources (`~/.cargo/git/checkouts`)
    so upstream investigation is possible.
12. Let `lsp diagnostics` run without `line` / `character` (they are meaningless
    for that action).

---

## Issues 18–21 (2026-09-30) — streaming UI & DeepSeek protocol

### 18. Every streamed token is rendered twice (fixed)

**Symptom.** While a reply streams, each token appears twice
(`消息消息输出输出时时…`). After the turn ends and the session is reloaded from the
DB the text is correct.

**Root cause.** The DB copy is right, so the backend emits each `text_delta`
once (single `emit_session` site, `chat_turn.rs`). The duplication is a
**leaked duplicate event listener** on the frontend:
`onChatEvent(...)` returns a `Promise<UnlistenFn>`, and the effects did
`onChatEvent(cb).then(fn => { unlisten = fn }); return () => unlisten?.()`.
If the effect cleans up *before* the promise resolves (React StrictMode
double-mount in dev, or `applyEvent` changing identity because
`handleAgentEvent` changed), `unlisten` is still `undefined`, the cleanup is a
no-op, and the late-resolving subscription is never removed. Two live listeners
=> every delta appended twice.

**Fix.** `disposed` flag in every `onChatEvent` subscription: if cleanup ran
first, the freshly resolved `fn` is called immediately. Files:
`codez/AssistantPanel.tsx`, `workz/index.tsx`, `workz/WorkflowRunPanel.tsx`,
`workz/CollabBoard.tsx`.

### 19. Larger UI font scale leaves blank strips right/bottom (fixed)

**Root cause.** `.agentz-app` uses CSS `zoom: var(--ui-font-scale)` and then
set `width/height: calc(100% / scale)`. With standardized zoom (current
WebView2/Chromium) percentages are *not* multiplied by the zoom factor, so
`100% / 1.25 = 80%` of the parent stayed 80% wide/high after zooming — the shell
shrank and exposed the background. (The earlier `100vw/scale` lines were
overridden by the later percent declarations anyway.)

**Fix.** `width: 100%; height: 100%;` in `src/index.css`. The counter-zoom on
editor/terminal/browser wrappers is unchanged.

**Verify manually** at 112.5% / 125% and after a window resize.

### 20. Tool calls should be interleaved with the text (fixed)

**Before.** One `ToolTrace` block was rendered above the whole reply.

**Now.** Each tool call records `textOffset` (chars of assistant text streamed
when `tool_start` arrived). `interleaveTools(text, tools)`
(`components/ToolTrace.tsx`) splits the reply at those offsets, so the message
reads `text → tool rows (grey result preview) → text → …`, and the running
tool (spinner) sits at the tail. Rows now show a one-line grey result preview
when collapsed; click to expand full input/output. Applied to CodeZ
(`AssistantMessageList.tsx`, `AssistantPanel.tsx` via `streamLenRef`) and WorkZ
(`agentTools.ts` `applyToolStart(…, last.text.length)`, `workz/index.tsx`).

**Limit.** Tool steps live in React state only for the current turn; after a
reload from the DB the history shows text only (same as before). Persisting
per-message tool trace is a follow-up.

### 21. DeepSeek: `tool_call_ids did not have response messages` (open — kernel)

**Error.** `An assistant message with 'tool_calls' must be followed by tool
messages responding to each 'tool_call_id'. The following tool_call_ids did not
have response messages: call_00_…` (HTTP 400, `invalid_request_error`).

DeepSeek uses the OpenAI-compatible client of `piscis-kernel`
(`DeepSeekClient` wraps `OpenAiClient`), so this is an engine bug. Reading
`OpenAiClient::convert_messages`:

- The "defense: skip orphaned tool_result" branch only accepts a tool result
  if `result.last()` is an assistant message *with `tool_calls`*. When one
  assistant turn issues **N parallel tool calls** and results arrive as more
  than one history message (or a message with several `ToolResult` blocks that
  are emitted as several `role:"tool"` entries), the 2nd..Nth result sees
  `last = tool` and is **dropped as "orphaned"**. The assistant message keeps
  all N `tool_calls` => DeepSeek rejects it. This matches the live warning in
  #15 (`stripping unsatisfied ToolUse ids … satisfied={}`).
- The pre-pass also treats a block as satisfied only for *immediately following*
  `Blocks` messages; any interleaved message (e.g. injected system/user text,
  vision flush) makes it "unsatisfied".

**Upstream patch spec (piscis-engine `openai.rs`).**
1. Replace the `result.last()` heuristic with a set of `pending_tool_ids`
   populated when emitting an assistant `tool_calls` message and drained as
   `tool` messages are emitted; accept a tool result iff its id is pending.
2. After conversion, for every emitted assistant `tool_calls` message, verify
   all ids have a `tool` reply; if not, either synthesize a
   `{"role":"tool","content":"[tool result missing: interrupted]"}` reply or drop
   the assistant `tool_calls` (keep its text). Never send an unpaired call.
3. Keep tool replies contiguous right after the assistant message; move
   vision/user messages after the last reply.
4. Synthesize the missing replies on turn cancel/timeout too (this repo's new
   `timed_out` path leaves half-finished calls in history).

### 22. DeepSeek / "think" compatibility audit (open — kernel)

- `is_deepseek_thinking_model()` matches only `deepseek-v4*` and
  `deepseek-reasoner`. Thinking-capable DeepSeek models reached through other
  names/gateways (`deepseek-v3.x` thinking variants, OpenRouter/SiliconFlow
  `deepseek-ai/DeepSeek-…`, `*-think`) are **not** covered; their
  `reasoning_content` is not replayed => 400 on multi-turn tool use.
  Recommend: case-insensitive match on `deepseek` + a per-model
  `supports_reasoning` setting; always send `thinking: {type: disabled}` for
  DeepSeek unless reasoning traces are persisted.
- Reasoning traces are neither persisted nor shown. Streamed
  `reasoning_content` deltas must be ignored (never appended to visible text),
  and inline `<think>…</think>` blocks from some gateways should be stripped or
  routed to a collapsible "thinking" block instead of the reply.
- Same class for DashScope Qwen (`enable_thinking:false` already handled) and
  other providers; there is no UI for showing thinking at all
  (`chat.thinking` is only a placeholder).
- Host mitigation: none possible in this repo (client is in the kernel).
  Users can pick `deepseek-chat` (non-thinking) until the patch lands.

---

## Fix batch 5 (2026-09-30) — CRLF `file_edit` (#5) and restore-order / lazy history

### #5 CRLF multi-line `file_edit` — fixed upstream (local commit)

`file_edit` lives in `piscis-engine` (`piscis-kernel/src/tools/file_write.rs`),
so it was fixed in a local clone at `C:\Projects\piscis-engine` and committed
(`19357cd`, "fix(file_edit): tolerate CRLF/LF mismatch…"). If `old_string` does
not match verbatim, it is retried with its line endings converted to the file's
style (CRLF vs LF), and `new_string` is converted the same way so the file
keeps a consistent EOL. Exact matches are untouched. Unit tests
(`eol_tests`) pass. **Not pushed / not yet consumed here:** this repo still pins
`v0.8.62`; to use it, push + tag the engine and bump the `rev` in the root
`Cargo.toml`, or enable the commented `[patch]` path block.

### Restore order / lazy history — fixed

- New command `chat_get_messages_page(session_id, project_dir, limit, offset)`
  (`session.rs`, registered in `lib.rs`) reads **newest-first** via
  `Database::get_messages_older` (rowid order, immune to clock skew) and returns
  each page oldest→newest with `next_offset` / `has_more`. Pages are trimmed at
  their old edge to begin on a user message so an assistant turn is never split
  across pages.
- `AssistantPanel.tsx`: restoring a session loads only the latest page
  (`HISTORY_PAGE = 120` raw rows), sets stick-to-bottom and scrolls to the newest
  message. Post-turn `syncMessagesFromDb` re-reads at least what is already
  loaded, so older pages are kept.
- `AssistantMessageList.tsx`: scrolling to the top (or "load older") first
  reveals already-loaded rows, then fetches the next-older page and prepends it
  with scroll anchoring preserved.
- `chat_get_messages` (full history) is unchanged and still used by WorkZ /
  PoolActivityFeed — those do not paginate yet.

---

## Fix batch 6 (2026-10-01) — work lost after an errored turn; stuck toasts

**Symptom.** After a red error interrupted a turn, the tool calls and LLM output
produced since the last completed turn disappeared once the user sent the next
message.

**Root causes.**
1. The kernel persists each LLM response when it *completes*; the reply that was
   streaming when the error hit was never stored (lost on the post-turn DB sync).
2. Tool calls/results were UI-only state: the history DTO dropped tool rows
   (`messages_for_ui`), so any DB re-sync — which the next turn always does —
   erased every tool row.
3. If the *first* turn of a new session errored, `chatSend` rejected before
   returning its session id, so the next message opened a **new** session and the
   old work looked gone.

**Fix.**
- `chat_turn.rs`: the collector tracks the text streamed since the last
  tool/response boundary; on error / timeout / user stop it is persisted as an
  assistant message (skipped if the kernel already stored it).
- `session.rs`: `messages_rich` rebuilds bubbles **with tool calls + results**
  from `tool_calls_json` / `tool_results_json` (`MessageDto.tools`, results capped
  at 20k chars). Used by both `chat_get_messages` and the paged command. A call
  with no stored result (interrupted) is shown as an error row.
- Frontend: `messageFromDto` maps `tools`; `AssistantMessageList` renders persisted
  tools interleaved by `text_offset` (live steps still drive the in-flight turn).
  `AssistantPanel` adopts the session id from the event stream when `chatSend`
  failed before returning it.
- WorkZ toasts now use one timer effect (a stale timeout can no longer dismiss or
  strand a newer toast). App-level exit/settings toast fixed earlier (batch 5+).

**Verify manually.** Force an error mid-turn (e.g. bad key on step 2), send another
message: previous tool rows and streamed text must stay. `cargo check` (0 warnings)
and `tsc --noEmit` pass; not exercised in a running app.

---

## Fix batch 7 (2026-10-01) — DeepSeek tool-call pairing (#21/#22), local engine commit

Fixed in the local `piscis-engine` clone (`C:\Projects\piscis-engine`, commit `af31d8d`); not yet pushed or consumed by this repo:

- `openai.rs` `convert_messages`: the orphan check now uses a set of pending
  `tool_call_id`s instead of "previous message has tool_calls", so the 2nd..Nth
  results of a parallel batch are no longer dropped. The vision flush no longer
  splits a batch of tool replies. Regression test added (18 openai tests pass).
- `is_deepseek_thinking_model` also matches gateway-prefixed / `think` / `r1`
  DeepSeek names.
- To consume: push + tag `piscis-engine`, bump `rev` in the root `Cargo.toml`
  (or enable the commented `[patch]` block). Includes the earlier CRLF
  `file_edit` commit (`19357cd`).
