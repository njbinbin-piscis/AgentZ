# AZ-024 Independent Review

- Reviewer: independent-reviewer-az024
- Task: AZ-024, task-level resource and cost budget
- Manifest: `docs/upgrade/evidence/AZ-024-closure-manifest.json` (revision closure-r1)
- Date: 2026-10-12
- Constraints followed: no source file and no `docs/upgrade/plan.json` modified. Only this review file was written. The scratch probe lived in `%TEMP%`, outside the repository.

## Scope

Files under review, SHA-256 re-computed and matching the manifest:

| File | SHA-256 (first 16) | Match |
| --- | --- | --- |
| `crates/agentz-task-state/src/budget.rs` | `CF572872...` | yes |
| `crates/agentz-task-state/src/budget_reservation.rs` | `ED3CBA25...` | yes |
| `src-tauri/src/runtime/task_budget.rs` | `160BC9BF...` | yes |
| `src-tauri/src/runtime/mod.rs` | `600B2F79...` | yes |
| `src-tauri/src/commands/chat_turn.rs` | `70C28D5C...` | yes |

Related code read to trace behaviour: the kernel `LlmClient` trait and cancellation race (`vendor/piscis-engine/piscis-kernel/src/llm/mod.rs`, `vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs`), the tool callers `src-tauri/src/tools/delegate.rs` and `src-tauri/src/tools/call_fish.rs`, and `src-tauri/src/runtime/task_details.rs`.

## What was checked

- Test suites named in the brief, re-run.
- Reservation lifecycle: reserve, start, settle, release and mark-unknown, and whether each path can double-settle, leak, or reach the provider.
- Concurrency: `BEGIN IMMEDIATE` on all write paths, and the revision check in `ensure_policy` and `reserve`.
- Orphan recovery at `reconcile_orphans`.
- Fail-open boundaries in the host guard.
- Acceptance criteria: explanatory stop, resume without reset, unknown price shown, unlimited default never denies, raising the limit lifts a block.
- SQL construction, path handling, and panics on user-controlled data.
- A scratch probe (outside the repo) that drives the real `ReservationStore` with the same request shape the host sends.

## Commands and results

| Command | Result |
| --- | --- |
| `cargo test -p agentz-task-state --lib budget` | ok. 25 passed, 0 failed, 125 filtered out |
| `cargo test -p agentz-desktop --lib task_budget` | ok. 4 passed, 0 failed, 275 filtered out |
| Scratch probe, 3 tests, against `agentz-task-state` by path | probe1 denial reproduced; probe2 denial reproduced; probe3 control accepted. 3 passed (the probe asserts the defect, so a pass means reproduced) |
| Manifest hash check (5 files) | all match |

Clippy was not run, per the brief.

Scratch probe output, verbatim:

- probe1 (unlimited, 4 calls reserved and started but never settled, same boot): next reserve `Err("budget denied: task resource budget exhausted")`, `in_flight=4`, `max_total_tokens=None`.
- probe2 (host-shaped reservation with `max_active_ms=1000`): `Err("budget denied: duration upper bound is unknown")`.
- probe3 (host-shaped reservation with `max_total_tokens=1000`): `Ok(Created)`.

## Findings

### P1-1: Stopped or timed-out model call leaks a reservation; unlimited default then denies

- Location:
  - `src-tauri/src/runtime/task_budget.rs:179-183`: `Ticket` has no `Drop` guard.
  - `src-tauri/src/runtime/task_budget.rs:305-337`: `finish` runs only when `stream`/`complete` return normally.
  - `vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs:3342-3360`: a user Stop wins a `biased` select against the LLM future and drops it.
  - `src-tauri/src/commands/chat_turn.rs:2730-2740`: the turn timeout drops the agent run future.
  - `crates/agentz-task-state/src/budget_reservation.rs:374`: `reconcile_orphans` skips records with the same `boot_id`.
  - `crates/agentz-task-state/src/budget_reservation.rs:522-530`: the slot cap (`max_in_flight`) is enforced regardless of whether any limit is set.
- Trace: Stop or timeout drops the future, so `finish` never runs. The reservation stays `Started` under this process's boot id. Same-boot records are never reconciled, so they count against the slot cap (`MAX_IN_FLIGHT = 4`, set for every task) for the rest of the process. The fifth reserve is refused, even when no limit is configured. Raising a limit does not free these records.
- Evidence: probe1 reproduced it (see above).
- Impact: breaks "unlimited default never denies" and "raising the limit lifts a block". Four Stop presses in one session block all model calls for that task until the app restarts.
- Suggested fix: a `Drop` guard on `Ticket` that settles or marks the reservation unknown when dropped unsettled. Also reconcile same-boot `Started` records whose run is no longer `running`. Keep the slot cap from applying to unlimited policies.

### P1-2: Any configured `max_active_ms` refuses every model call

- Location:
  - `src-tauri/src/runtime/task_budget.rs:225`: every host reservation sets `active_ms: None`.
  - `src-tauri/src/runtime/task_budget.rs:117`, `:126`, `:143`: `max_active_ms` from the global limits is passed into the store policy.
  - `crates/agentz-task-state/src/budget_reservation.rs:505-508`: under `policy.max_active_ms.is_some()`, `resources.active_ms.ok_or("duration upper bound is unknown")` fails.
- Evidence: probe2 reproduced it. The host's own tests use token limits or no limit, so they miss it.
- Impact: setting an active-time limit in global settings makes every model call for the task fail with `budget denied: duration upper bound is unknown`. Nothing is recorded and no call succeeds.
- Suggested fix: the host must bound `active_ms` per call, or the reservation core must not require a per-call bound when only settled duration is enforced. Add a host test that sets `max_active_ms`.

### P1-3: Research sub-agent model calls bypass the budget entirely

- Location:
  - `src-tauri/src/commands/chat_turn.rs:1430-1462`: `run_subagent_with_prompt` builds its own client and calls `into_agent_loop` with no `guard_client`.
  - Callers: `src-tauri/src/tools/delegate.rs:82` and `src-tauri/src/tools/call_fish.rs:134`, both invoked from inside a parent turn.
- Impact: sub-agent calls are not reserved, not denied when the task budget is exhausted, and not recorded. A parent turn over budget can keep spending through `delegate` or `call_fish`. This is the "denied call still reaches the provider" path the brief asked about.
- Suggested fix: wrap the sub-agent client with `guard_client` using the parent binding, or refuse delegation while the budget is exhausted. No test covers this path.

### P2-1: Fail-open is wider than "store unavailable"; accounting can be skipped while the store is healthy

- Location:
  - `src-tauri/src/runtime/task_budget.rs:230-238`: `denied_or_open` turns every non-`budget denied:` error into an unbudgeted call.
  - `src-tauri/src/runtime/task_budget.rs:196-201`: a revision read error also fails open.
  - `src-tauri/src/runtime/task_budget.rs:162-166`: an `ensure_policy` failure runs the whole turn unbudgeted.
  - `src-tauri/src/runtime/task_budget.rs:296-299`: a settlement failure only logs, so the usage is dropped and the reservation stays `Started`.
  - `crates/agentz-task-state/src/budget_reservation.rs:353`: `current()` requires `execution == "running"`. A run that stops running yields a non-denial error, so the call runs unbudgeted.
  - Race: `begin` reads the revision at `task_budget.rs:196`, outside the reservation transaction. A concurrent `ensure_policy` revision then makes `reserve` fail with `stale reservation policy revision` (`budget_reservation.rs:671-672`). That is not a denial, so the call runs with no reservation and no settlement, with a healthy store and a configured budget.
  - SQLite busy/locked errors after the 5-second busy timeout take the same path.
- Impact: contradicts "do not silently skip accounting when the budget IS configured and the store is healthy". The only signal is a `warn!` line.
- Suggested fix: retry on stale revision (re-read, then reserve again) and on busy. When a budget is configured, treat any reservation error as a denial. Surface settlement failure rather than dropping it.

### P2-2: One settlement above its estimate blocks every later call in that revision

- Location: `crates/agentz-task-state/src/budget_reservation.rs:440-451` (overshoot block in `check_budget`), `:335-337` and `:876` (overshoot recording). The estimator is `piscis_kernel::llm::estimate_request_input_tokens` (`task_budget.rs:207-209`).
- Trace: an estimate that undercounts input tokens marks the reservation `settlement exceeded reservation`. Every later reserve under that revision then fails with `previous settlement exceeded reservation; reconciliation required`, even when cumulative usage is far below the limit (the store's own test `overshoot_is_recorded_and_stops_next_reservation` asserts this, total 2 of 10). The only exit is a limit change, and the message gives no such path.
- Impact: likely false stops in production whenever the heuristic estimate is below actual input. The stop is explanatory in form only.
- Suggested fix: drop the per-call overshoot block, since the cumulative check already covers the limit. Otherwise name the action to take in the denial text.

### P2-3: Unknown price is recorded but never shown

- Location: `crates/agentz-task-state/src/budget.rs:363` (`unknown_price_records`) and `:390` (`observed_limits_exceeded`). `src-tauri/src/runtime/task_details.rs:56-59` exposes only `usage_count` and the revision. No `src-tauri` code reads `unknown_price_records` or `observed_limits_exceeded`.
- Impact: the host never sets cost, so every usage record has an unknown price, and the user never sees that. "Unknown price shown explicitly" is met only inside the store snapshot.
- Suggested fix: surface the unknown-price count in task details, or in the denial and stop text.

### P2-4: A corrupt or unreadable limits file silently means unlimited

- Location: `src-tauri/src/runtime/task_budget.rs:51-62` (`Ok(_) | Err(_)` falls back to default), `:76-80`.
- Impact: a user-configured limit is dropped, with only a `warn!` line. The task runs unbudgeted with no UI notice.
- Suggested fix: fail closed on a parse error, or show a visible status.

### P2-5 (latent): Recovered orphans have no duration, so a configured active limit blocks permanently once P1-2 is fixed

- Location: `crates/agentz-task-state/src/budget_reservation.rs:406` (`active_ms: record.resources.active_ms`, always `None` from the host), feeding `unknown_duration_records` and the check at `budget_reservation.rs:474-476` (`historical duration is unknown`). The counter is historical, so a limit raise does not clear it.
- Status: found by code reading, not probed. It is hidden today by P1-2, which denies earlier. Becomes live as soon as P1-2 is fixed.

### P3-1: Unchecked revision increment

- `src-tauri/src/runtime/task_budget.rs:141`: `current.revision + 1`. Use `checked_add`.

### P3-2: Replay outcomes are not treated as denials

- `src-tauri/src/runtime/task_budget.rs:240-243`: `ReserveOutcome::Replay` is ignored. `:252`: `StartOutcome::Replay` returns `Ok(None)`, so the call reaches the provider with no ticket. Unreachable with fresh UUID operation ids, but it should be a denial.

### P3-3: Settlement provenance and charge

- `src-tauri/src/runtime/task_budget.rs:269`: a successful call with no `Done` chunk is settled at the reserved estimate and labelled `error-reserved-estimate`. The label is misleading.
- `:325` and `:336`: a failed provider call is charged the full estimate. This is conservative, but repeated failures can exhaust a token limit.

### P3-4: Shared temporary file for limits

- `src-tauri/src/runtime/task_budget.rs:70`: the fixed name `task_budget.json.tmp` lets concurrent saves clobber each other. Use a unique temporary name.

### P3-5: Denial text is technical and not user-facing

- `budget denied: task resource budget exhausted` and similar strings are English, carry no numbers, and surface as an agent error. The kernel classifies them as `Unknown` (`vendor/piscis-engine/piscis-kernel/src/llm/error_class.rs`), so there is no fallback loop. This does not meet the spirit of an explanatory stop.

## Verified as correct

- No double settlement: `settle` is idempotent on identical usage and rejects conflicting usage (`budget_reservation.rs:855-877`).
- Denied calls do not reach the provider on the guarded path: `begin` returns `Err` before the inner call (`task_budget.rs:306`, `:330`).
- SQL is parameterised. The only `format!` interpolates constant table names (`budget.rs:181`, `budget_reservation.rs` schema check).
- Write paths use `BEGIN IMMEDIATE`. Competing reserve and start are covered by tests.
- Revision compare-and-set happens inside the transaction (`budget.rs:258-290`).
- Recovery of `Reserved` (released) and `Started` (settled at estimate, only when the run is not `running`) matches the brief, except the same-boot gap in P1-1.
- No panics found on user-controlled data. Integer arithmetic goes through checked helpers (`total`, `add`).

## Test coverage gaps

- No test for Stop or timeout dropping an in-flight call (P1-1).
- No host test with `max_active_ms` (P1-2).
- No test for the sub-agent path (P1-3).
- No test for stale-revision or busy errors in the guard (P2-1).

VERDICT: FAIL
