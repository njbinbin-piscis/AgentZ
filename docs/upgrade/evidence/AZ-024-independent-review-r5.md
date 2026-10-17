# AZ-024 Independent Review, Round 5

- Reviewer: independent-reviewer-az024-r5
- Task: AZ-024, task-level resource and cost budget
- Manifest: `docs/upgrade/evidence/AZ-024-closure-manifest.json` (revision closure-r5)
- Prior reviews: `AZ-024-independent-review.md` (r1, FAIL), `AZ-024-independent-review-r2.md` (r2, FAIL), `AZ-024-independent-review-r3.md` (r3, FAIL), `AZ-024-independent-review-r4.md` (r4, FAIL on R4-1)
- Date: 2026-10-12
- Constraints followed: no source file and no `docs/upgrade/plan.json` modified. Only this review file was written. No scratch probe was run; findings come from code reading plus the two required test commands.

## Scope

Full AZ-024 change set, read end to end:

- `src-tauri/src/runtime/task_budget.rs`: `PENDING`, `Pending`, `apply_pending`, `retry_entries`, `retry_pending_settlements`, `BudgetScope` (`bind`, `guard`), `BudgetedClient::begin`, `finish`, `Settlement`, `stream`, `complete`, tests.
- `crates/agentz-task-state/src/budget_reservation.rs`: `reconcile_orphans`, `check_budget`, `ReservationStore::{reserve, start, release_unstarted, finish_without_usage, settle, query, revise_policy}`.
- `crates/agentz-task-state/src/budget.rs`: `validate_usage`, `snapshot` (`unknown_price_records`), `BudgetStore::configure`.
- `crates/agentz-task-state/src/run.rs`: `validate_budget_origin`, `validate_budget_run` (read to decide whether a release or settle can fail deterministically).
- `src-tauri/src/runtime/mod.rs` (`pub mod task_budget;`), `src-tauri/src/commands/chat_turn.rs` (bind and guard sites), `src-tauri/src/tools/delegate.rs`, `src-tauri/src/tools/call_fish.rs` (shared `BudgetScope`).

### Manifest hashes

SHA-256 recomputed with `Get-FileHash -Algorithm SHA256` and compared with the manifest. All 7 match.

| File | SHA-256 (first 16) | Match |
| --- | --- | --- |
| `crates/agentz-task-state/src/budget.rs` | `CF572872...` | yes |
| `crates/agentz-task-state/src/budget_reservation.rs` | `B2416177...` | yes |
| `src-tauri/src/runtime/task_budget.rs` | `8E507BFB...` | yes |
| `src-tauri/src/runtime/mod.rs` | `600B2F79...` | yes |
| `src-tauri/src/commands/chat_turn.rs` | `45BA4C01...` | yes |
| `src-tauri/src/tools/delegate.rs` | `50909C46...` | yes |
| `src-tauri/src/tools/call_fish.rs` | `2D88A4C4...` | yes |

Only `task_budget.rs` changed since r4 (its r4 hash differed). `budget_reservation.rs` is byte-identical to r4, so r4 line references into it still hold.

Git note: `crates/agentz-task-state/` and `src-tauri/src/runtime/task_budget.rs` are untracked, so there is no committed baseline for these files.

## Answer to the brief: is R4-1 fixed?

The discarded `release_unstarted` failure on the start-failure path is fixed. A failed release is now queued (`task_budget.rs:384-394`), the queue is drained first in every `begin` (`task_budget.rs:289`), and an entry that still fails stays queued (`task_budget.rs:267-277`). The release retry is a `Reserved` to `ReleasedUnstarted` transition, which `release_unstarted` accepts (`budget_reservation.rs:810-820`), so the retry frees the slot before the next `check_budget`.

R4-1 is not closed in full. r4 described a related shape under R4-1: a `start` whose commit errored after it was durable leaves a `Started` record with no ticket. r5 does not handle it, and the queued release can never succeed for that record, because `release_unstarted` refuses `Started` (`budget_reservation.rs:816-819`). The same gap exists on the `reserve` path, which r4 did not mention. Both are carried as new finding R5-1 (P2, open) below.

## Whole-flow review

1. **Begin.** `retry_pending_settlements()` runs before anything else (`task_budget.rs:289`). Then `open_existing`, token estimate, `query`. Under a limit, `remaining <= 0` refuses with `budget denied: task active-time budget exhausted` before any reservation (`task_budget.rs:330-336`). `reserve` runs with stale-revision retry (`task_budget.rs:366-369`, 3 attempts), and `ReserveOutcome::Replay` is refused (`task_budget.rs:363-365`). Denial is only returned for errors with the `budget denied:` prefix; all other errors fail open (`task_budget.rs:308-313`). `reserve` and `start` both prefix `check_budget` errors with `budget denied:` (`budget_reservation.rs:702-703`, `755-756`).
2. **Start and release on start failure.** `start` is an `IMMEDIATE` transaction that re-checks budget with the record excluded from `others` (`budget_reservation.rs:747-759`). Denied or rolled-back starts leave `Reserved` and release succeeds. A `Replay` from start is refused without release, which is correct because a `Replay` record is not `Reserved`. An `Err` from `start` after a durable commit is the R5-1 gap.
3. **Settlement drop path.** `Settlement::drop` calls `finish(ticket, None)` exactly once; `settle` takes the ticket so `drop` sees `None`. Dropped `stream` and `complete` futures therefore settle at the estimate (`task_budget.rs:213-236`). The test `dropped_calls_are_settled_and_do_not_hold_slots_or_budget` passes.
4. **Finish.** Measured usage is used only when the call succeeded (`task_budget.rs:458-471`). Otherwise the estimate is recorded with label `estimate-without-usage`. A failed `settle` is queued (`task_budget.rs:441`). `now_ms` failure silently drops the settlement (`task_budget.rs:416-417`, R5-2).
5. **Settle idempotency on retry.** `usage_id`, `operation_id` and `created_ms` are fixed once in `finish`, so a retried `Pending::Settle` presents identical usage. `settle` returns success for an identical existing row (`budget_reservation.rs:866-872`), so a commit that errored but was durable does not double-charge.
6. **BudgetScope bind and guard.** Bind happens after the run exists and before any tool that can call a model (`chat_turn.rs:2515-2520`). The shared `OnceLock` means sub-agents cloned before bind are charged to the parent task. The first bind wins. `delegate.rs:29,93` and `call_fish.rs:30,146` pass clones, and `chat_turn.rs:1449` guards sub-agent clients. A bind failure (`ensure_policy` error) leaves the scope unbound, so calls run unbudgeted (fail-open, P2-1).
7. **Unlimited early return.** The overshoot block is gated by `enforces_limit` (`budget_reservation.rs:440-451`). Then `if !enforces_limit { return Ok(()) }` (`budget_reservation.rs:453-455`) returns before any slot, token, duration or cost check. Unlimited therefore never denies. The early return covers cross-boot and same-boot `Reserved`, `Started` and `Unknown` records.
8. **Acceptance criteria.** Every denial the host returns starts with `budget denied:`. Resume does not reset: `ensure_policy` only configures when no policy exists, and a changed limit becomes a new revision over the same usage rows. Unknown price is explicit: `unknown_price_records` is counted in `budget.rs:363`, and the host never sets `max_cost`, so it does not deny. Unlimited never denies, as above.

## Per-finding status

### R4-1 and P0–P2 IDs

| ID | Sev | Prior status (r4) | r5 status | Evidence in r5 code and tests |
| --- | --- | --- | --- | --- |
| R4-1 | P2 | Open | Fixed for the discarded release error. Related ambiguous-commit shape open as R5-1 | Queue `task_budget.rs:381-394`; drain first in begin `:289`; retry failure stays queued `:267-277`; test `failed_reservation_updates_stay_queued_until_applied` (`:814`) passes. Ambiguous commit not handled, see R5-1 |
| R3-1 | P2 | Partially fixed (settle only) | Open via R5-1 | Settle queue `task_budget.rs:441`; release queue `:387-393`; ambiguous commit leftover (`budget_reservation.rs:714`, `:759`) not released |
| N1 | P2 | Unlimited fixed; limited partly open | Unlimited: fixed (`budget_reservation.rs:453-455`, test `unlimited_policy_is_not_blocked_by_an_overshoot` passes). Limited: open via R5-1 | As R3-1 |
| N2 | P2 | Accepted residual | Accepted residual (user-approved) | Per-call bound `min(remaining, 600000)` at `task_budget.rs:330-338` |
| N3 | P2 | Fixed (r3) | Fixed | Host always sends `active_ms` (`task_budget.rs:351`); recovery copies it (`budget_reservation.rs:406`); residual R3-2 |
| P1-1 | P1 | Partially fixed (release open) | Fixed for stopped, timed-out and errored calls (drop settle `task_budget.rs:221-236`, settle queue `:441`). Commit-ambiguous leftover open via R5-1 | `Settlement` tests pass |
| P1-2 | P1 | Fixed | Fixed (`task_budget.rs:330-338`; test `active_time_limit_admits_calls_with_a_bounded_reservation` passes) | |
| P1-3 | P1 | Fixed | Fixed (`delegate.rs:93`, `call_fish.rs:146`, `chat_turn.rs:1449`; test `sub_agent_scope_shares_the_parent_task_budget` passes) | |
| P2-1 | P2 | Accepted residual | Accepted residual (user-approved). Note the new fail-open path in R5-1 falls under this class | Fail-open `task_budget.rs:308-313`; stale retry `:366-369` |
| P2-2 | P2 | Accepted residual | Accepted residual (user-approved) | Overshoot block gated by limit and revision (`budget_reservation.rs:440-451`) |
| P2-3 | P2 | Accepted residual | Accepted residual (UI not surfaced). Store explicit via `unknown_price_records` (`budget.rs:363`) | |
| P2-4 | P2 | Accepted residual | Accepted residual (user-approved) | `load_limits` warn `task_budget.rs:60-70` |
| P2-5 | P2 | Fixed (r3) | Fixed | As N3 |

### P3 items (not counted against PASS)

| ID | Status | Note |
| --- | --- | --- |
| R3-2 | Accepted residual | Unlimited recovered call charged 10 minutes |
| R3-3 | Open, P3, not counted | Test gaps. Partly addressed for `PENDING` (see R5-3) |
| R4-2 | Accepted residual | Unbounded queue, synchronous retry |
| R4-3 | Accepted residual | Transient invisibility during retry window |
| R4-4 | Accepted residual | Permanent failures retried with warnings |
| R4-5 | Partially addressed, not counted | One unit test covers `retry_entries`. No test for begin-level retry or the failed settle then success path |
| N4 | Open, P3, not counted | Active-time exhaustion text vs overshoot text (`task_budget.rs:333-335`, `budget_reservation.rs:449`) |
| N6 | Accepted residual | Blocking SQLite on async path and in `Drop` (`task_budget.rs:288`, `:403`) |
| N7 | Accepted residual (confirmed unguarded) | Auto-title call, see Residuals |
| P3-1, P3-2, P3-3, P3-4, N5 | Fixed | |
| P3-5 | Accepted residual | English denial text |

## New findings

### R5-1 (P2, open): an ambiguous commit leaves a same-boot `Reserved` or `Started` record that nothing releases or settles

- Locations:
  - `crates/agentz-task-state/src/budget_reservation.rs:714` (`reserve` commit): on error, `reserve` returns `Err` with no record of the operation.
  - `src-tauri/src/runtime/task_budget.rs:370` (`begin`): a `reserve` error goes to `denied_or_open` and the call runs unbudgeted. No release is attempted and nothing is queued.
  - `crates/agentz-task-state/src/budget_reservation.rs:759` (`start` commit) with `src-tauri/src/runtime/task_budget.rs:384-394`: a release after a failed start is attempted, but `budget_reservation.rs:816-819` refuses it because the record is `Started`. The `Pending::Release` entry therefore fails on every retry (`task_budget.rs:289`).
- Trace (start): the commit reports an error after it became durable. The record is `Started` with the current boot id. `reconcile_orphans` skips same-boot records (`budget_reservation.rs:374`). `check_budget` counts `Started` against slots, tokens and active time (`budget_reservation.rs:490`, `526`). The queued release can never succeed. The record holds one of the 4 slots and its reserved budget until restart. Under a limit, after four such records every call is refused with `budget denied: task resource budget exhausted`. A limit raise does not clear the slot cap.
- Trace (reserve): the same, except the record is `Reserved` and nothing is queued. `Reserved` records are counted the same way, so the hold persists until restart and the call itself runs unbudgeted.
- Permanent release failures reach the same end state. For example, `validate_budget_origin` (`run.rs:1633-1640`) failing at release time while the run state is inconsistent leaves the `Pending::Release` entry retried forever.
- Reachability: needs a `COMMIT` that reports an error after the write became durable (an I/O-level ambiguity). Narrow, but this is the same consequence r4 rated P2 under R4-1 (r4 described the `start` shape explicitly, and its suggested fix covered it). That fix was not implemented.
- Suggested fix: on a `start` or `reserve` commit error, read the record back by operation id. If it is `Reserved`, queue a release. If it is `Started`, queue a settlement at the estimate (`finish_without_usage` with `unknown` is an alternative, since `Unknown` also holds budget). If it is absent, do nothing. Queue all of these with the existing retry.

### R5-2 (P3, new, not counted): settlement silently dropped when `now_ms` fails

- Location: `src-tauri/src/runtime/task_budget.rs:416-417`. On `now_ms` failure `finish` logs `budget settlement skipped` and returns without queuing, so a started record is left to hold budget until restart. Reachability is effectively nil (the clock would need to be before 1970). Fold into the R5-1 fix (queue or read back instead of returning).

### R5-3 (P3, new, not counted): the `retry_entries` doc claim and test coverage

- Location: `src-tauri/src/runtime/task_budget.rs:267-277` (`retry_entries`) and `:814` (test). The brief calls `retry_entries` a pure function. It is not: it performs SQLite writes (`apply_pending`, `:255-263`) and logs. The test checks only that a failing entry (missing store) is returned, not that a `begin` retries a settle or release and then frees the slot. Correct the description and add the begin-level test requested in R4-5.

### R5-4 (P3, note, carried): retry window and persistence are in-memory only

- The queue lives in process memory (`task_budget.rs:243`). A queued settlement lost by a crash leaves a `Started` record from another boot, which the pre-existing cross-boot `Started` refusal (`budget_reservation.rs:490-492`) blocks until the run is no longer `running`. This is r4's pre-existing residual. It is not counted, but the queue's durability limits should be documented.

## Residuals

Accepted (not counted):

- r1 P2-1 (store-error fail-open, stale-revision retry), P2-2 (overshoot block), P2-3 (unknown price UI not yet surfaced), P2-4 (corrupt limits file falls back to unlimited with a warning), P3-5 (English denial strings).
- r2 N2 (concurrent small active limits).
- R3-2 (unlimited recovered call charged 10 minutes).
- R4-2 (queue unbounded, synchronous retry), R4-3 (transient invisibility), R4-4 (permanently failing entries retried with warnings).
- User-approved overshoot block design.
- Pre-existing cross-boot `Started` refusal (r4 residual, see R5-4).

Unguarded model call sites outside the guarded turn. Yes, I found these; none is guarded by `BudgetScope`:

- Auto-title: `src-tauri/src/commands/chat_turn.rs:225-257` (`maybe_llm_rename_session_title`) builds its own client and calls `complete`. It runs after the first round of a turn (`chat_turn.rs` ~2924).
- Post-turn hooks: `src-tauri/src/commands/post_turn.rs:25-38` (`mem_client` for memory extraction, then background skill review). Called from `chat_turn.rs:2947` and `src-tauri/src/commands/gateway.rs:716`.
- Other direct client builds: `src-tauri/src/commands/skill_review.rs:143`, `src-tauri/src/commands/curator.rs:345`, `src-tauri/src/commands/memory_consolidate.rs:90`, `src-tauri/src/commands/edit.rs:67` and `:175` (inline edit and completion).
- Gateway and Koi turns: `src-tauri/src/commands/gateway.rs:688` (`run_piscis_turn`) and `src-tauri/src/runtime/koi.rs:421` (`run_piscis_turn_cancellable`). Both build their own client in the kernel headless runner (`piscis-kernel/src/headless/mod.rs:274`). A configured limit does not bound these turns. This is a coverage gap the user should know about even though it is outside the chat turn.

Guarded (verified): main chat turn (`chat_turn.rs:2515-2520`), sub-agent runs (`chat_turn.rs:1449`, used by delegate and call_fish), and heartbeat and chat through `run_agentz_turn`. No separate client construction was found in `src-tauri` for the in-loop kernel calls (compaction, vision, fallback), and `vision_delegate` is not set anywhere in `src-tauri`. I did not audit the kernel internals beyond that.

## Commands and results

| Command | Result |
| --- | --- |
| SHA-256 of 7 manifest files (`Get-FileHash -Algorithm SHA256`) | All 7 match the manifest. |
| `cargo test -p agentz-task-state --lib budget` | `ok. 25 passed; 0 failed; 125 filtered out`. Exit 0. |
| `cargo test -p agentz-desktop --lib task_budget` | `ok. 10 passed; 0 failed; 275 filtered out`. Exit 0. Includes `failed_reservation_updates_stay_queued_until_applied`. |
| Scratch probe | Not run. R5-1 is a code-read finding. |
| Clippy | Not run, per the brief. |

## New-finding summary

| ID | Severity | Status | Location |
| --- | --- | --- | --- |
| R5-1 | P2 | Open (counted) | `budget_reservation.rs:714, 759, 816-819`; `task_budget.rs:370, 384-394` |
| R5-2 | P3 | Open (not counted) | `task_budget.rs:416-417` |
| R5-3 | P3 | Open (not counted) | `task_budget.rs:267-277, 814` |
| R5-4 | P3 | Note (not counted) | `task_budget.rs:243`; `budget_reservation.rs:490-492` |

## Verdict basis

R4-1 as scoped by the brief (the discarded release failure) is fixed. The fix is correct and its test passes. The related ambiguous-commit shape that r4 described under R4-1 is not fixed, and the `reserve` path has the same gap. Under R4-1's rating (P2), that leaves an open P2 (R5-1), so the round does not pass. R5-2 to R5-4 and R4-2 to R4-5 are P3 and do not block.

VERDICT: FAIL
