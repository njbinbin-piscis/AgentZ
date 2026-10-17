# AZ-024 Independent Review, Round 4

- Reviewer: independent-reviewer-az024-r4
- Task: AZ-024, task-level resource and cost budget
- Manifest: `docs/upgrade/evidence/AZ-024-closure-manifest.json` (revision closure-r4)
- Prior reviews: `docs/upgrade/evidence/AZ-024-independent-review.md` (r1, FAIL), `docs/upgrade/evidence/AZ-024-independent-review-r2.md` (r2, FAIL), `docs/upgrade/evidence/AZ-024-independent-review-r3.md` (r3, FAIL)
- Date: 2026-10-12
- Constraints followed: no source file and no `docs/upgrade/plan.json` modified. Only this review file was written. No scratch probe was run in this round; findings are from code reading plus the two required test commands.

## Scope

Round 4 covers the r4 change set (failed settlements queued in process and retried before each reservation) and the r3 state it must preserve:

- `src-tauri/src/runtime/task_budget.rs`: `PENDING`, `pending`, `settle_usage`, `retry_pending_settlements`, `begin`, `finish`, `Settlement`, `stream`, `complete`, tests.
- `crates/agentz-task-state/src/budget_reservation.rs`: `reconcile_orphans`, `check_budget`, `reserve`, `start`, `release_unstarted`, `finish_without_usage`, `settle`, `query`.
- `crates/agentz-task-state/src/run.rs`: `validate_budget_origin` and `validate_budget_run`, read to decide whether `settle` can fail deterministically for a started call.

### Manifest hashes

SHA-256 of the 7 manifest files was recomputed and compared with the manifest. All 7 match.

| File | SHA-256 (first 16) | Match |
| --- | --- | --- |
| `crates/agentz-task-state/src/budget.rs` | `CF572872...` | yes |
| `crates/agentz-task-state/src/budget_reservation.rs` | `B2416177...` | yes |
| `src-tauri/src/runtime/task_budget.rs` | `1B513A73...` | yes |
| `src-tauri/src/runtime/mod.rs` | `600B2F79...` | yes |
| `src-tauri/src/commands/chat_turn.rs` | `45BA4C01...` | yes |
| `src-tauri/src/tools/delegate.rs` | `50909C46...` | yes |
| `src-tauri/src/tools/call_fish.rs` | `2D88A4C4...` | yes |

Git note: `crates/agentz-task-state/` and `src-tauri/src/runtime/task_budget.rs` are untracked, so there is no committed baseline for these files.

## Answers to the brief's questions

**Does the settle retry fix R3-1 (settle path)?** Yes, for settlement. A settle that fails with a storage error is pushed to `PENDING` (`task_budget.rs:405-408`). The next `begin` calls `retry_pending_settlements()` (`task_budget.rs:268`) before its snapshot and reserve, so a later successful call frees the slot before the check runs. Retry also runs on every `begin` even when the call is then denied, so a limit denial does not stop the drain.

**Can a retried settlement double-charge?** No. Checked in `settle` (`budget_reservation.rs:825-890`):

- `settle` is one `IMMEDIATE` transaction. The usage insert and the record update commit together or not at all, so a failed attempt leaves no partial charge.
- `usage_id`, `operation_id` and `created_ms` are fixed once in `finish` and the retry uses `usage.clone()`. A retry therefore presents identical usage.
- Identical usage on an existing row returns success without a second insert (`budget_reservation.rs:862-871`). A commit that errored but actually committed is therefore harmless to retry.
- The existing-row lookup matches on `usage_id OR operation_id` (`budget_reservation.rs:862`). A different usage for the same operation is refused with `usage settlement identity conflict` (`budget_reservation.rs:865`).
- The overshoot flag is set only while status is not `Settled` (`budget_reservation.rs:879`), so a retry cannot re-flag.

**Recovery and state changes under a pending entry.** Recovery of a started record from another process (`reconcile_orphans`, `budget_reservation.rs:366-430`) settles it at the estimate under `usage_id = recovered-<operation>`. A later in-process retry then hits the identity-conflict refusal, so the estimate is not charged twice. The entry is refused forever (see R4-2). A `Reserved` record cannot reach `settle` (`budget_reservation.rs:857-861` refuses), and `ReleasedUnstarted` is also refused. Neither path can double-charge.

**Is a permanently failing entry acceptable?** Not as implemented. See R4-2 (P3, open). Permanent failures that keep a started record holding budget could not be found: `settle` checks for a started call are deterministic on host-built usage (`validate_usage` at `budget.rs:106-131`, `validate_budget_origin` at `run.rs:1633-1640`). `validate_budget_origin` does not require the run to still be `running`, so a run that finished does not make settle fail. The only permanent refusal found is the identity conflict after recovery, where the record is already `Settled` and holds no budget.

**Poisoning.** `pending()` recovers the guard with `into_inner` (`task_budget.rs:245-247`). Critical sections are only `Vec` take and push; no lock is held across SQLite work or an `await` (`let entries = std::mem::take(&mut *pending());` at `task_budget.rs:256` drops the guard at end of statement). Poisoning cannot lose entries or deadlock.

**Is the failing-entry retry logged?** Yes, with `warn!` at `task_budget.rs:259`, but the message does not include the operation id or the queue length, and it is emitted on every `begin` (R4-2).

## Round 1 finding status

| r1 ID | r4 status | Evidence |
| --- | --- | --- |
| P1-1 stopped or timed-out call leaks reservation | Partially fixed. Drop path and settle retry fixed. Release path still open: R4-1 (P2). | Drop settles (`task_budget.rs:232-236`). Settle retry (`task_budget.rs:255-263, 268, 405-408`). Release not queued (`task_budget.rs:362`). |
| P1-2 configured `max_active_ms` refuses every call | Fixed (unchanged) | Per-call bound `task_budget.rs:308-318`. Test `active_time_limit_admits_calls_with_a_bounded_reservation` passes. |
| P1-3 research sub-agent calls bypass budget | Fixed (unchanged) | Shared `BudgetScope`. Test `sub_agent_scope_shares_the_parent_task_budget` passes. |
| P2-1 fail-open wider than store unavailable | Accepted residual (user-approved, unchanged) | `denied_or_open` (`task_budget.rs:287-292`); stale revision retried 3 times (`task_budget.rs:320`, `budget_reservation.rs:674-676`). |
| P2-2 one overshoot blocks later calls | Accepted residual (user-approved, unchanged) | Gated by limit and revision (`budget_reservation.rs:443-451`). |
| P2-3 unknown price not shown | Accepted residual (user-approved, unchanged) | `unknown_price_records` in the snapshot only. |
| P2-4 corrupt limits file means unlimited | Accepted residual (user-approved, unchanged) | Logged with `warn!` (`task_budget.rs:60-70`). |
| P2-5 recovered orphans have no duration | Fixed for new reservations (r3 N3). Residuals R3-2 and R3-3. | Host always sends `active_ms` (`task_budget.rs:330`). Recovery copies it (`budget_reservation.rs:406`). |
| P3-1 unchecked revision increment | Fixed | `checked_add` in `ensure_policy` (`task_budget.rs:146-150`). |
| P3-2 Replay outcomes not denials | Fixed | `ReserveOutcome::Replay` refused (`task_budget.rs:342-344`); `StartOutcome::Replay` refused (`task_budget.rs:359`). |
| P3-3 settlement provenance | Fixed | Label `estimate-without-usage` (`task_budget.rs:376-379`). |
| P3-4 shared temporary file | Fixed (unchanged) | UUID temporary name in `save_limits`. |
| P3-5 denial text technical | Accepted residual (user-approved, unchanged) | See N4. |

## Round 2 finding status

| r2 ID | r4 status | Evidence |
| --- | --- | --- |
| N1 same-boot unsettled reservation blocks tasks | Unlimited: fixed. Limited: the settle half is fixed by r4; the release half is open as R4-1 (P2). | Unlimited early return `budget_reservation.rs:453-455`. Test `unlimited_policy_is_not_blocked_by_an_overshoot` passes. |
| N2 concurrent calls refused under `max_active_ms` | Accepted residual (user-approved, unchanged) | Per-call bound `min(remaining, 600000)` at `task_budget.rs:316`. |
| N3 recovered record without duration blocks later limit | Fixed (r3). Residual R3-2. | `budget_reservation.rs:406`; `task_budget.rs:330`. |
| N4 active-time exhaustion reads as overshoot | Open, P3, not blocking (unchanged) | Overshoot text `budget_reservation.rs:450`; active-time text `task_budget.rs:314`. |
| N5 `ReserveOutcome::Replay` treated as success | Fixed | `task_budget.rs:342-344`. |
| N6 blocking SQLite work on the async path and in `Drop` | Open, P3, not blocking (unchanged; r4 adds retries on the same path, see R4-2) | `begin` `task_budget.rs:267-362`; `finish` `task_budget.rs:371-410`. |
| N7 session auto-title call not budget-scoped | Open, P3, not blocking (unchanged) | `chat_turn.rs` bytes unchanged (manifest hash matches). |

## Round 3 finding status

| r3 ID | r4 status | Evidence |
| --- | --- | --- |
| R3-1 same-boot unsettled reservation under a limit blocks until restart (P2) | Partially fixed. Settle failures are now retried on the next `begin`. Release failures are still not retried, so a same-boot `Reserved` record can still hold a slot and its reserved budget until restart. Open as R4-1 (P2). | `task_budget.rs:255-263, 268, 405-408` (settle, fixed). `task_budget.rs:359-364` (release, open). |
| R3-2 crash-recovered unlimited call charged 10 minutes (P3) | Accepted residual (listed per the brief, unchanged) | `task_budget.rs:310, 330`; `budget_reservation.rs:406`. |
| R3-3 missing tests (P3) | Accepted residual (listed per the brief). Gaps still present, and the new `PENDING` path has no test (R4-5). | No test references `PENDING`, `pending` or `retry_pending_settlements` (grep over both files). Orphan test `budget_reservation.rs:1249` does not assert `active_ms`. No host-level Replay refusal test. |

## Open and new findings

Counted against PASS: R4-1 (P2). Open P3 items, not counted: R4-2, R4-3, R4-4 (below), N4, N6, N7, and the R4-5 test gap.

### R4-1 (P2, open): a failed release leaves a same-boot `Reserved` record holding a slot under a limit

- Locations:
  - `src-tauri/src/runtime/task_budget.rs:359-364`: when `start` returns an error, the code calls `let _ = store.release_unstarted(...)` and discards any failure. The failed release is not queued, unlike settlement (`task_budget.rs:405-408`).
  - `crates/agentz-task-state/src/budget_reservation.rs:765-770, 781-824`: `release_unstarted` requires status `Reserved`. A storage failure leaves the record `Reserved`.
  - `crates/agentz-task-state/src/budget_reservation.rs:374`: `reconcile_orphans` skips records with the current boot id, so no later reconcile in this process clears it.
  - `crates/agentz-task-state/src/budget_reservation.rs:526-534`: `Reserved` records count toward `MAX_IN_FLIGHT` and toward every enforced limit.
- Trace: a `start` that fails (storage error after the 5-second busy timeout, or a refusal from `check_budget` when a concurrent settlement changed the sums) is followed by a `release_unstarted` that also fails. The `Reserved` record stays. It holds one of the 4 slots and its reserved tokens and active time for the rest of the process. `retry_pending_settlements` does not cover it, so the next `begin` does not clear it. Under a limit, after four such records every call is refused with `task resource budget exhausted`. A limit raise keeps `max_in_flight`, so it does not help.
- Related shape, same consequence: a `start` whose commit errored but actually committed leaves a `Started` record with no ticket and no queued settlement. The `start` error path releases it, but `release_unstarted` refuses `Started` (`budget_reservation.rs:814-818`), so it also stays until restart.
- Reachability: requires a release failure in the same process as the start failure. Both must be storage failures (busy beyond 5 seconds), or a refused start followed by a release storage failure. Narrower than the settle case, but within the r3 trigger class the user accepted as fail-open, and the consequence is the same persistent denial that r3 classified as P2.
- Why this is still open: r4 changed only the settle path. The r3 suggested fix named "settlement and release" with retry; the release half is not implemented.
- Suggested fix: queue the failed release with the same retry (store binding, operation id and reason, retry `release_unstarted`). For a `start` error that might have committed, read the record back and, if it is `Started`, queue it as a settlement at the estimate (or as `Unknown`) instead of discarding the ticket.

### R4-2 (P3, new): the retry queue is unbounded, retries are synchronous, and permanent failures are retried forever with a weak log

- Locations: `task_budget.rs:243-263` (`PENDING`, no cap, no give-up), `task_budget.rs:268` (retry on every `begin`, on the async path), `task_budget.rs:259` (`warn!` without operation id or queue size).
- Impact:
  - Each retry opens a fresh SQLite connection (`open_existing`) and runs a write transaction with a 5-second busy timeout. With a lock held by another writer, one `begin` can block a tokio worker for roughly (entries + 1) 脳 5 seconds. Queue length is not bounded.
  - Permanent failures are retried on every `begin`. The identity-conflict case (recovered record, see "Answers") has no budget effect, but its warning repeats forever with no operation id, which makes it hard to diagnose.
- Reachability: entries accumulate only when settlement fails while a later call is admitted, and a sustained lock is needed for the stall. Permanent entries arise only from cross-process recovery.
- Suggested fix: cap the queue (for example 64) and log the dropped operation id at `error`; include the operation id and entry count in the retry warning; move the retry off the request path or bound it per `begin` (for example retry at most N entries per call).

### R4-3 (P3, new): retry window hides queued entries from concurrent calls

- Location: `task_budget.rs:256` takes all entries, then retries them outside the lock (`task_budget.rs:257-262`).
- Trace: while thread A retries, its entries are not in `PENDING`, and their records are still `Started` in the store. A concurrent `begin` on thread B sees an empty queue and proceeds to reserve, so it can be denied transiently by a record that is about to settle. This is fail-closed and temporary, and no double charge is possible. If the retry panics (not expected in `settle_usage`), the remaining taken entries are dropped and the records hold budget until restart.
- Suggested fix: keep entries in the queue until each succeeds (retry in place by index), or hold a retry lock so concurrent `begin` calls wait for the drain.

### R4-4 (P3, new): recovered-then-retried record stays `Settled` with an identity-conflict warning forever

This is the permanent-failure case from R4-2, recorded separately so that it is not confused with a budget hold. Cross-process recovery settles the record at its estimate (`budget_reservation.rs:387-421`). The in-process retry then fails with `usage settlement identity conflict`. The record is `Settled`, so no slot or budget is held. No double charge occurs. Accepted as P3 only if R4-2's cap and log improvements land.

### R4-5 (P3, new): no test covers the `PENDING` retry path

- No test drives a settle failure followed by a successful `begin`, and none drives a failed release. The r3 test gaps (R3-3) remain, and the new path adds two more.
- Suggested tests: (1) inject a settle failure (for example a held `BEGIN IMMEDIATE` lock with a short busy timeout, or a removed store) and assert that the next `begin` retries and the slot is freed; (2) the same with a failed release (R4-1); (3) a permanently failing entry is refused and logged once with its operation id.

## Acceptance criteria

| Criterion | Result |
| --- | --- |
| Exhaustion yields an explanatory denial starting with `budget denied:` | Met for prefix (every policy denial carries it). Explanation terse (N4, P3-5). |
| Resume does not reset the task total | Met (unchanged). Test `exhausted_limit_stops_next_call_and_raised_limit_resumes_without_reset` passes. |
| Unknown price is explicit | Met in the store snapshot. Accepted P2-3 residual for UI. |
| Unlimited default never denies | Met for normal flow and for same-boot, cross-boot and unknown records (early return `budget_reservation.rs:453-455`). The release-path leak under a limit is R4-1, and it only matters once a limit is set. |
| Raising the limit lifts a block | Met for exhaustion and overshoot (new revision). Met for settle-path slot holds after r4. Not met for the release-path slot hold (R4-1). |

## Verified r3 state (still holds)

- Unlimited early return: `budget_reservation.rs:453-455`, after the overshoot block that is gated by `enforces_limit` (`budget_reservation.rs:440-451`).
- Per-call active bound: `task_budget.rs:308-318`. Unlimited uses `PER_CALL_ACTIVE_MS = 600_000` (`task_budget.rs:35`). Limited uses `min(remaining, 600_000)`, and refuses at `remaining <= 0` (`task_budget.rs:311-317`).
- Always-present active time: `active_ms: Some(active_ms)` on every host reservation (`task_budget.rs:330`).
- Replay refusal: `ReserveOutcome::Replay` (`task_budget.rs:342-344`) and `StartOutcome::Replay` (`task_budget.rs:359`) both return `budget denied: duplicate call operation`.
- Denied calls do not reach the provider (`begin` returns `Err` before the inner call, `task_budget.rs:415, 443`).

## New-finding summary

| ID | Severity | Status | Location |
| --- | --- | --- | --- |
| R4-1 | P2 | Open (counted) | `task_budget.rs:359-364`; `budget_reservation.rs:765-824, 374, 526-534` |
| R4-2 | P3 | Open (not counted) | `task_budget.rs:243-263, 268, 259` |
| R4-3 | P3 | Open (not counted) | `task_budget.rs:256-262` |
| R4-4 | P3 | Open (not counted) | `budget_reservation.rs:387-421, 865` |
| R4-5 | P3 | Open (not counted) | Tests: none for `PENDING` |

## Commands and results

| Command | Result |
| --- | --- |
| SHA-256 of 7 manifest files (`Get-FileHash -Algorithm SHA256`) | All 7 match the manifest. |
| `cargo test -p agentz-task-state --lib budget` | `ok. 25 passed; 0 failed; 125 filtered out`. Exit 0. |
| `cargo test -p agentz-desktop --lib task_budget` | `ok. 9 passed; 0 failed; 275 filtered out`. Exit 0. Tests: `unbound_scope_is_transparent`, `missing_store_fails_open_and_reaches_the_provider`, `limits_round_trip_and_reject_negative_values`, `exhausted_active_time_refuses_before_the_provider`, `sub_agent_scope_shares_the_parent_task_budget`, `active_time_limit_admits_calls_with_a_bounded_reservation`, `exhausted_limit_stops_next_call_and_raised_limit_resumes_without_reset`, `unlimited_default_records_usage_and_never_denies`, `dropped_calls_are_settled_and_do_not_hold_slots_or_budget`. |
| Scratch probe | Not run in r4. R4-1 is a code-read finding. |

Clippy was not run, per the brief.

## Accepted residuals

Listed per the brief, not counted against PASS:

- r1 P2-1 (store-error fail-open, stale-revision retry), P2-2 (overshoot block), P2-3 (unknown price not shown), P2-4 (corrupt limits file), P3-5 (English denial text).
- r2 N2 (concurrent calls refused under `max_active_ms`, reserved to full remaining budget).
- R3-2 (crash-recovered unlimited call charged 10 minutes, P3).
- R3-3 (missing tests, P3). Extended by R4-5 to the `PENDING` path.
- Pre-existing, not changed by r4: a started record from another process blocks a limited task until its run is no longer running (cross-boot `Started` refusal, `budget_reservation.rs:490-492`). The in-process queue cannot clear it.
- No new residuals are accepted by this reviewer. R4-1 is not accepted; it needs a fix or an explicit user acceptance.

## Verdict basis

R4-1 is an open P2. It is the release half of R3-1, which r4 did not fix. The settle half is fixed: a settle failure is retried on the next `begin`, the retry cannot double-charge, and the permanent-failure cases do not hold budget. The round therefore does not pass. R4-2 to R4-5 are P3 and do not block.

VERDICT: FAIL
