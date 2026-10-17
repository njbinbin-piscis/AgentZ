# AZ-024 Independent Review, Round 6

- Reviewer: independent-reviewer-az024-r6
- Task: AZ-024, task-level resource and cost budget
- Manifest: `docs/upgrade/evidence/AZ-024-closure-manifest.json` (revision closure-r6)
- Prior reviews: `AZ-024-independent-review.md` (r1), `-r2.md`, `-r3.md`, `-r4.md`, `-r5.md`
- Date: 2026-10-12
- Constraints followed: no source file and no `docs/upgrade/plan.json` modified. Only this review file was written. No scratch probe or temporary test was added (that would modify source). Findings come from code reading of the r6 code plus the two required test commands.

## Scope

Read end to end for r6:

- `src-tauri/src/runtime/task_budget.rs`: `PENDING`, `Pending`, `apply_pending`, `retry_entries`, `retry_pending_settlements`, `BudgetedClient::begin` (including the reserve loop, the start step, and both error arms), `finish`, `usage_for`, `settle_or_queue`, `recover_outstanding`, `Settlement`, `stream`, `complete`, tests.
- `crates/agentz-task-state/src/budget_reservation.rs`: `reconcile_orphans`, `check_budget`, `ReservationStore::{reserve, start, release_unstarted, finish_without_usage, settle, query, records}`.
- `crates/agentz-task-state/src/run.rs`: `validate_budget_origin`, `validate_budget_run` (read to decide whether release, settle, or query can fail for a stopped run).
- `crates/agentz-task-state/src/budget.rs`: `validate_usage`, `snapshot` (unchanged since r5).
- Call sites: `chat_turn.rs` (bind and guard), `delegate.rs`, `call_fish.rs`, `runtime/mod.rs` (unchanged).

## Manifest hashes

SHA-256 recomputed with `Get-FileHash -Algorithm SHA256` and compared with the manifest. All 7 match.

| File | SHA-256 (first 16) | Match |
| --- | --- | --- |
| `crates/agentz-task-state/src/budget.rs` | `CF572872...` | yes |
| `crates/agentz-task-state/src/budget_reservation.rs` | `B2416177...` | yes |
| `src-tauri/src/runtime/task_budget.rs` | `54D00522...` | yes (changed in r6) |
| `src-tauri/src/runtime/mod.rs` | `600B2F79...` | yes |
| `src-tauri/src/commands/chat_turn.rs` | `45BA4C01...` | yes |
| `src-tauri/src/tools/delegate.rs` | `50909C46...` | yes |
| `src-tauri/src/tools/call_fish.rs` | `2D88A4C4...` | yes |

Only `task_budget.rs` changed since r5. `budget_reservation.rs` is byte-identical to r5 (hash matches), so r5 line references into it still hold. Line numbers below are current.

## Answer to the brief: is R5-1 fixed?

Yes for cases (a) to (d). Case (e) is a residual, stated below. The fix is in `begin` (`task_budget.rs:316-390`) and `recover_outstanding` (`task_budget.rs:454-491`).

### R5-1 cases

| Case | Code path | Result |
| --- | --- | --- |
| (a) reserve committed, error returned | Non-denial reserve error reaches the general arm at `task_budget.rs:370-375`. `recover_outstanding` (`:373`) reads back the record by `operation_id` (`:462-471`). It is `Reserved`, so `release_unstarted(..., "start refused")` runs (`:482`), which sets `ReleasedUnstarted` (`budget_reservation.rs:765-823`). The call then runs unbudgeted, as a storage error does. | Fixed. Slot and budget freed when the query succeeds. |
| (b) start committed, error returned | Start error arm at `task_budget.rs:387-390`. `recover_outstanding` finds `Started`, so it calls `usage_for` with the reserved estimate and `"estimate-without-usage"` (`:474-476`), then `settle_or_queue` (`:441-449`). `settle` (`budget_reservation.rs:825-889`) records `Settled`. The overshoot check cannot trigger, because the measured values equal the bounds. | Fixed. Charged at the reserved estimate, which is conservative. |
| (c) start denied by policy | `check_budget` at `budget_reservation.rs:755-756` returns `budget denied:` before commit, so the transaction rolls back and the record stays `Reserved`. The start error arm calls `recover_outstanding` unconditionally (`task_budget.rs:389`), which releases it. Then `denied_or_open` returns the denial (`:390`). | Fixed. Store-level half is covered by `new_observer_consumption_before_start_keeps_reserved_when_balance_insufficient` (`budget_reservation.rs:1442-1453`). |
| (d) reserve denied | The guard at `task_budget.rs:371-372` skips recovery for `budget denied:`, so no query runs. The lookup is by `operation_id`, which is a fresh UUID per `begin` (`task_budget.rs:306`). Even if recovery ran, it could not match another call's record. The active-time denial at `:330-336` returns before any reserve, so it needs no recovery. | Confirmed. No false release is possible. |
| (e) query fails after a commit error | `recover_outstanding` logs `budget state unreadable after failed commit` (`task_budget.rs:465`) and returns. It does not queue anything. The record stays `Reserved` or `Started`. `reconcile_orphans` skips same-boot records (`budget_reservation.rs:374`), so it persists until restart. `check_budget` counts it under a limit (`:490`). Unlimited policies are not affected (`:440-455`). | Residual R6-3 (P3, not counted). See below. |

Case (e) rationale: it needs an ambiguous commit (durable write, error returned) and then a read failure on the same connection immediately after. The stuck record can only hold budget under a limited policy and only for the rest of the process. It cannot cause overspend, because the reservation is conservative, and it is logged. A queued `Recover` retry would close it, but that is not required to pass.

## Whole-flow review (r6 items)

1. **Stale-revision retry** (`task_budget.rs:367-369`, `MAX_RESERVE_ATTEMPTS=3`). A stale error comes from `reserve` before the insert, so nothing was committed and `continue` (with the same `operation`) is safe. After the third stale error, the general arm runs `recover_outstanding` (finds nothing) and fails open. This is the accepted P2-1 residual.
2. **Replay refusal.** `reserve` Replay (`:363-365`) and `start` Replay (`:386`) both return a denial. `reserve` Replay cannot happen with a fresh UUID. `start` Replay happens only when the record is not `Reserved` (`budget_reservation.rs:745-746`). No writer other than this call starts an operation id, so that is also unreachable. Both arms skip `recover_outstanding`, which is inconsistent with the other failure arms. Noted as R6-2 (P3 note).
3. **Settlement drop.** `Settlement::settle` takes the ticket, so `Drop` sees `None` and does not double-settle. A dropped future settles at the estimate (`task_budget.rs:218-236`). No await sits between `begin` returning the ticket and the `Settlement` construction (`:498-502`, `:527-531`), so no cancellation window leaks a `Started` record.
4. **Unlimited early return.** `enforces_limit` gates the overshoot block (`budget_reservation.rs:440-451`), and `if !enforces_limit { return Ok(()) }` (`:453-455`) returns before the slot, token, duration, and cost checks. Unlimited never denies. `reserve` still runs `reconcile_orphans` and the record scan on each call, which is the cost note R6-4.
5. **PENDING queue retry in `begin`.** `retry_pending_settlements` runs first (`task_budget.rs:289`). It takes the entries under the lock, drops the guard, performs the I/O without holding the lock, and re-extends with the failures. No `pending()` guard is held across a store call or another `pending()` call, so there is no self-deadlock. Entries carry their project path, so a retry from any task applies to the right store. Idempotence: a `Settle` retry resubmits identical usage (`usage_id`, `operation_id`, `created_ms` fixed in `finish`), and `settle` accepts an identical existing row (`budget_reservation.rs:866-872`). A `Release` retry has the defect in R6-1.
6. **Lock handling.** `pending()` recovers from poisoning (`task_budget.rs:253`). The lock is held only for `Vec` operations.
7. **Started record without a Ticket.** The only `Started` creation is `start` returning `Granted` (`task_budget.rs:380-385`). Its ticket is always returned to `stream` or `complete` and settled. The non-granted paths are: start error (recovered, R5-1 cases b and e), and start `Replay` (R6-2). The Ticket path misses settlement only if `usage_for` fails on `now_ms` (`task_budget.rs:437`, R5-2, accepted). In that case the record is left `Started`.
8. **Sub-agents and guard sites.** Unchanged since r5 (hashes match). `budget.guard` is applied at `chat_turn.rs:1449` (sub-agent client) and `chat_turn.rs:2520` (main client). `BudgetScope` is held by `delegate.rs:29` and `call_fish.rs:30`.

## Per-finding status

### R5-1 and R4-1

| ID | Sev | Prior status (r5) | r6 status | Evidence |
| --- | --- | --- | --- | --- |
| R5-1 | P2 | Open | Fixed for (a) to (d). Residual (e) as R6-3, P3, not counted | `task_budget.rs:370-375`, `:387-390`, `:454-491`; `budget_reservation.rs:765-823`, `:755-756` |
| R4-1 | P2 | Fixed for the discarded release error; ambiguous commit open | Closed. Discarded release error still queued (`task_budget.rs:482-490`). Ambiguous start and reserve handled by R5-1 fix | Queue `:484-490`; drain `:289`; retry-failure stays queued `:267-277` |

### Earlier P0 to P2 IDs

| ID | Sev | Prior status (r5) | r6 status | Evidence |
| --- | --- | --- | --- | --- |
| R3-1 | P2 | Open via R5-1 | Closed. Settle queued (`:441-449`), release queued (`:482-490`), recovery read-back (`:454-491`) | As above |
| N1 | P2 | Limited open via R5-1 | Closed for limited policies, except (e) residual R6-3. Unlimited fixed | `budget_reservation.rs:440-455` |
| N2 | P2 | Accepted residual | Accepted residual (user-approved) | Per-call bound `min(remaining, 600000)` at `task_budget.rs:330-338`; in-flight cap 4 |
| N3 | P2 | Fixed | Fixed. Host always sends `active_ms` (`task_budget.rs:351`) | |
| P1-1 | P1 | Open via R5-1 | Closed. Drop settle `task_budget.rs:221-236`; stop and timeout paths tested | `dropped_calls_are_settled_and_do_not_hold_slots_or_budget` passes |
| P1-2 | P1 | Fixed | Fixed (`task_budget.rs:330-338`) | `active_time_limit_admits_calls_with_a_bounded_reservation` passes |
| P1-3 | P1 | Fixed | Fixed. Shared `BudgetScope` (`chat_turn.rs:1449`, `:2520`; `delegate.rs:29`; `call_fish.rs:30`) | `sub_agent_scope_shares_the_parent_task_budget` passes |
| P2-1 | P2 | Accepted residual | Accepted residual. Fail-open on storage error and on stale-revision exhaustion (`task_budget.rs:367-369`, `:308-313`) | |
| P2-2 | P2 | Accepted residual | Accepted residual (user-approved overshoot block; gated `budget_reservation.rs:440-451`) | |
| P2-3 | P2 | Accepted residual | Accepted residual. Unknown-price UI not surfaced; store explicit via `unknown_price_records` (`budget.rs`) | |
| P2-4 | P2 | Accepted residual | Accepted residual (corrupt limits file falls back to unlimited with a warning, `task_budget.rs:60-70`) | |
| P2-5 | P2 | Fixed | Fixed | As N3 |

### P3 items (not counted against PASS)

| ID | Status | Note |
| --- | --- | --- |
| R5-2 | Accepted residual (not counted) | `usage_for` returns on `now_ms` failure (`task_budget.rs:437`); `finish` logs and returns (`:410`). Also reachable from the recovery path via `recover_outstanding` (`:477-479`). Unchanged in r6 |
| R5-3 | Accepted residual (not counted) | `retry_entries` still performs I/O (`task_budget.rs:267-277`). No begin-level test exists for the R5-1 recovery path or for the retry-then-free-slot path. Does not block (see below) |
| R5-4 | Note (not counted) | In-memory queue (`task_budget.rs:243`) and cross-boot `Started` refusal (`budget_reservation.rs:490-492`) |
| R4-2 | Accepted residual | Unbounded queue, synchronous retry |
| R4-3 | Accepted residual | Transient invisibility during the retry window (`task_budget.rs:281-284`) |
| R4-4 | Accepted residual | Permanently failing entries retried with warnings. A permanent release failure holds a slot until restart (same-boot records skipped, `budget_reservation.rs:374`) |
| R4-5 | Partially addressed | `retry_entries` unit test only. Still no begin-level test (R5-3) |
| R3-2 | Accepted residual | Unlimited recovered call charged 10 minutes |
| N4 | Open, P3, not counted | Active-time exhaustion text vs overshoot text |
| N6 | Accepted residual | Blocking SQLite on the async path and in `Drop` (`task_budget.rs:288`, `:404`) |
| N7 | Accepted residual (confirmed unguarded) | Auto-title call. See unguarded scope section |
| P3-1 to P3-4, N5 | Fixed | |
| P3-5 | Accepted residual | English denial text |

## New findings

### R6-1 (P3, not counted): a retried release can fail forever after an ambiguous release commit

- Location: `task_budget.rs:261` (retry uses reason `"release retried after storage error"`), `task_budget.rs:482` (first attempt uses `"start refused"`), `budget_reservation.rs:810-816` (`finish_without_usage`).
- Trace: the first release commit lands but returns an error. The record is now `ReleasedUnstarted` with reason `start refused`. The retry calls `release_unstarted` with a different reason. The early return at `budget_reservation.rs:810` needs both the same status and the same reason, so it does not fire. The next check then refuses anything not `Reserved` (`:813-816`). The entry stays in `PENDING` and warns on every `begin`.
- Impact: no budget effect. The record is already released and holds nothing. The cost is permanent log noise and one queued entry.
- Fix: compare on status alone for an idempotent retry, or reuse the same reason string.

### R6-2 (P3 note, not counted): `start` Replay arm skips recovery

- Location: `task_budget.rs:386`.
- Every other failure arm calls `recover_outstanding`. The Replay arm returns a denial without it. Because `start` Replay means the record is not `Reserved` (`budget_reservation.rs:745-746`), a `Started` record here would have no ticket. Reachability is nil: only this call starts this fresh UUID, and cross-process reconciliation can only move `Reserved` to `ReleasedUnstarted`, which `current()` also refuses earlier. Defence in depth: call `recover_outstanding` on Replay too.

### R6-3 (P3 residual, not counted): case (e), query failure after an ambiguous commit

- Location: `task_budget.rs:462-467`. No queue.
- See the R5-1 table. Stated as residual. Optional fix: queue a `Recover { binding, operation }` item in `PENDING` and retry it in `begin` with the same semantics as `recover_outstanding`.

### R6-4 (P3 note, not measured): each model call rescans the task history

- Location: `budget_reservation.rs:274-341` (`records`), called from `reconcile_orphans` (`:366`), `reserve` (`:654-715`), `start` (`:717-762`, twice), `settle` (`:825-889`), and `query` (`:891-923`). Each row calls `run::validate_budget_origin` (`budget_reservation.rs:302`), which runs several queries.
- Cost grows with the number of calls in the task, so long tasks pay O(n) validation on every call. Not measured. Not a correctness finding. Owner to decide whether it is acceptable.

### R6-5 (P3 doc accuracy, not counted): module doc overstates coverage

- Location: `task_budget.rs:1-6`. The doc says every model call in the task, including delegated ones, is charged to the task total. That is true for the guarded sites only. The unguarded sites in the next section are not in the task scope. Narrow the doc, or add the residuals.

## Residuals (accepted or explicit; not counted)

- N2: concurrent calls near small active limits can be refused. Accepted by user (per-call bound and in-flight cap).
- R3-2: an unlimited recovered call is charged 10 minutes. Recovery only, and no effect without a limit.
- R4-2: the queue is unbounded and retries are synchronous. It grows only on storage failure.
- R4-3: a transient invisibility window during retry. Next `begin` drains before checking.
- R4-4: a permanently failing entry retries with warnings. A permanent release failure holds a slot until restart.
- R5-2: a `now_ms` failure drops the settlement. Clock failure before 1970 is not realistic.
- R5-3: `retry_entries` is not pure as the brief describes, and there is no begin-level test. Does not block; see verdict basis.
- R5-4: the queue is in memory only. A crash can leave a cross-boot `Started` record, which blocks the task until its run stops.
- R6-3: case (e), a read failure after an ambiguous commit. Stuck record until restart, under a limit only, no overspend.
- P2-1: fail-open on storage error and on stale-revision exhaustion. Accepted.
- P2-2: overshoot block design. User-approved.
- P2-3: unknown-price UI not surfaced. Store is explicit.
- P2-4: corrupt limits file falls back to unlimited with a warning. Accepted.
- P3-5: English denial strings. Accepted.
- N6: blocking SQLite on the async path. Accepted.
- Pre-existing cross-boot `Started` refusal (`budget_reservation.rs:490-492`). Accepted (R5-4).

## Unguarded model call sites: scope statement

The only `BudgetScope` guard sites are `chat_turn.rs:1449` and `chat_turn.rs:2520`. `delegate.rs` and `call_fish.rs` carry clones of the scope. Every site below is outside the task budget scope. None is wired to a `BudgetScope`, and none can be bounded by a configured limit.

| Call site | Path | Scope |
| --- | --- | --- |
| Auto-title | `chat_turn.rs:201-255` (client at `:225`, `complete` at `:255`), invoked at `:2926` | Not in task scope. Explicit residual N7 (accepted in r4) |
| Post-turn memory extraction and background skill review | `post_turn.rs:25-38`, called from `chat_turn.rs` (after the turn) and `gateway.rs:716` | Not in task scope. Not in the brief's accepted list. Owner must accept it as an explicit residual |
| Skill review | `skill_review.rs:143` | Not in task scope. Owner must accept it as an explicit residual |
| Curator | `curator.rs:345` | Not in task scope. Owner must accept it as an explicit residual |
| Memory consolidate | `memory_consolidate.rs:90` | Not in task scope. Owner must accept it as an explicit residual |
| Inline edit and completion | `edit.rs:67`, `edit.rs:175` | Not in task scope. Owner must accept it as an explicit residual |
| Gateway turn | `gateway.rs:688` (`run_piscis_turn`, headless runner) | Not in task scope. Owner must accept it as an explicit residual |
| Koi turn | `koi.rs:421` (`run_piscis_turn_cancellable`, headless runner) | Not in task scope. Owner must accept it as an explicit residual |

Guarded (verified): main chat turn (`chat_turn.rs:2520`), sub-agents (`chat_turn.rs:1449`, used by `delegate.rs` and `call_fish.rs`), and heartbeat and chat through `run_agentz_turn`. Per brief, these unguarded sites are not fixed in this round.

## Commands and results

| Command | Result |
| --- | --- |
| SHA-256 of 7 manifest files (`Get-FileHash -Algorithm SHA256`) | All 7 match the manifest |
| `cargo test -p agentz-task-state --lib budget` | `test result: ok. 25 passed; 0 failed; 0 ignored; 125 filtered out`. Exit 0 |
| `cargo test -p agentz-desktop --lib task_budget` | `test result: ok. 10 passed; 0 failed; 0 ignored; 275 filtered out`. Exit 0. Includes `failed_reservation_updates_stay_queued_until_applied` |
| Git note | `crates/agentz-task-state/` and `src-tauri/src/runtime/task_budget.rs` are untracked, so no committed baseline exists. Comparison is against the manifest hashes |
| Scratch probe or fault-injection test | Not run. Code reading only. No test in the suite exercises the new `recover_outstanding` paths (cases a to d) |
| Clippy | Not run, per the brief |

Note: PowerShell reported the cargo "Finished" line as a native-command error. The cargo exit codes were 0 and the test results above are from the test output.

## New-finding summary

| ID | Severity | Status | Location |
| --- | --- | --- | --- |
| R6-1 | P3 | Open (not counted) | `task_budget.rs:261, 482`; `budget_reservation.rs:810-816` |
| R6-2 | P3 note | Open (not counted) | `task_budget.rs:386` |
| R6-3 | P3 | Residual (not counted) | `task_budget.rs:462-467` |
| R6-4 | P3 note | Note (not counted) | `budget_reservation.rs:274-341`, `:302` |
| R6-5 | P3 | Open (not counted) | `task_budget.rs:1-6` |

No new P0, P1, or P2 findings.

## Verdict basis

- R5-1 cases (a) to (d) are handled by the r6 code. Case (e) is a residual that needs two storage failures in a row and cannot overspend. It is stated, not counted.
- R4-1 and the earlier P1 and P2 items remain closed or accepted.
- No new P0, P1, or P2 was found in the rest of the flow (stale retry, Replay, Settlement drop, unlimited early return, PENDING retry, lock handling, Ticket path).
- R5-2 and R5-3 do not block. R5-3 is still a test gap. The r6 recovery code has no executed test, so the R5-1 fix is confirmed by code reading and the two test suites only, not by a fault-injection run. Before closure, add fault-injection tests for cases (a) and (b), and fix R6-1.
- The unguarded call sites are outside the task budget scope. They do not change the verdict under this round's brief, but the owner must accept them explicitly before the closure statement claims full model-call coverage.

VERDICT: PASS
