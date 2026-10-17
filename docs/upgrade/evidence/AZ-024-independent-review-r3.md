# AZ-024 Independent Review, Round 3

- Reviewer: independent-reviewer-az024-r3
- Task: AZ-024, task-level resource and cost budget
- Manifest: `docs/upgrade/evidence/AZ-024-closure-manifest.json` (revision closure-r3)
- Prior reviews: `docs/upgrade/evidence/AZ-024-independent-review.md` (r1, FAIL), `docs/upgrade/evidence/AZ-024-independent-review-r2.md` (r2, FAIL)
- Date: 2026-10-12
- Constraints followed: no source file and no `docs/upgrade/plan.json` modified. Only this review file was written. The scratch probe lives in `%TEMP%\az024r3-probe`, outside the repository. Cargo was pointed at the repository `target` directory for build artifacts only.

## Scope

Round 3 covers the three r2 fixes (N1 unlimited early return, N2 per-call active bound, N3 duration on every host reservation), the Replay refusal on reserve, and the r1/r2 findings they touch. Read in full: `crates/agentz-task-state/src/budget_reservation.rs` (`check_budget`, `reconcile_orphans`, `reserve`, `start`, `finish_without_usage`, `settle`, tests) and `src-tauri/src/runtime/task_budget.rs` (`begin`, `finish`, `Settlement`, `stream`, `complete`, `ensure_policy`, tests).

### Manifest hashes

SHA-256 of the 7 manifest files was recomputed and compared with the manifest. All 7 match.

| File | SHA-256 (first 16) | Match |
| --- | --- | --- |
| `crates/agentz-task-state/src/budget.rs` | `CF572872...` | yes |
| `crates/agentz-task-state/src/budget_reservation.rs` | `B2416177...` | yes |
| `src-tauri/src/runtime/task_budget.rs` | `7898E5C9...` | yes |
| `src-tauri/src/runtime/mod.rs` | `600B2F79...` | yes |
| `src-tauri/src/commands/chat_turn.rs` | `45BA4C01...` | yes |
| `src-tauri/src/tools/delegate.rs` | `50909C46...` | yes |
| `src-tauri/src/tools/call_fish.rs` | `2D88A4C4...` | yes |

Git note: `crates/agentz-task-state/` and `src-tauri/src/runtime/task_budget.rs` are untracked (`??`), so no committed baseline exists for these files. No pre-r3 reservation records can exist in a committed store.

## Round 2 finding status

| r2 ID | Severity | r3 status | Evidence |
| --- | --- | --- | --- |
| N1 same-boot unsettled reservation blocks unlimited tasks | P2 | Fixed for unlimited. Open for limited (see R3-1). | Early return at `budget_reservation.rs:453-455`. Probe N1 unlimited: fifth reserve `Ok(Created)`. Test `unlimited_policy_is_not_blocked_by_an_overshoot` passes. Limited control in the same probe still denies (R3-1). |
| N2 concurrent calls refused under `max_active_ms` | P2 | Accepted residual (user-approved). Narrower than the r2 wording suggests; see the note below. | Per-call bound `min(remaining, PER_CALL_ACTIVE_MS)` at `task_budget.rs:283-290`. Probe N2: second concurrent reservation refused at limit 1,000,000 and at limit 60,000. |
| N3 recovered record without duration blocks a later limit | P2 | Fixed for new reservations. Residual: recovered calls are charged 10 minutes (R3-2). | Host always sends `active_ms: Some(active_ms)` (`task_budget.rs:304`). Recovery copies it (`budget_reservation.rs:406`). No test asserts the recovered duration (R3-3). |
| N4 active-time exhaustion reads as overshoot | P3 | Open, not blocking. | Overshoot still blocks with `budget denied: previous settlement exceeded reservation; reconciliation required` (`budget_reservation.rs:450`). The explanatory `task active-time budget exhausted` text (`task_budget.rs:288`) is reached only between calls. |
| N5 `ReserveOutcome::Replay` treated as success | P3 | Fixed. | `task_budget.rs:316-318` refuses with `budget denied: duplicate call operation`. `StartOutcome::Replay` refused at `task_budget.rs:333`. |
| N6 blocking SQLite work on the async path and in `Drop` | P3 | Open, unchanged, not blocking. | `task_budget.rs:242-343`, `345-386`. |
| N7 session auto-title call not budget-scoped | P3 | Open, unchanged, not blocking. | `chat_turn.rs` bytes unchanged since r2. |

N2 note: the accepted residual covers "a concurrent set whose combined bounds exceed the remaining budget". In practice, with any `max_active_ms` at or below 600,000 ms, a second in-flight call is refused, because each call reserves the whole remaining budget up to the cap. Probe N2 (limit 60,000) shows this. The user accepted this residual; it is listed here so the wider reach is on record.

## Round 1 finding status

| r1 ID | r3 status | Evidence |
| --- | --- | --- |
| P1-1 stopped or timed-out call leaks reservation | Partially fixed. Open for limited policies under storage errors: R3-1. | Drop path settles (`task_budget.rs:219-236`, used by `stream` at 389-413 and `complete` at 417-428). Unlimited symptom fixed (N1). |
| P1-2 configured `max_active_ms` refuses every call | Fixed | Per-call bound at `task_budget.rs:283-290`. Test `active_time_limit_admits_calls_with_a_bounded_reservation` passes. |
| P1-3 research sub-agent calls bypass budget | Fixed (unchanged since r2) | Shared `BudgetScope`. Test `sub_agent_scope_shares_the_parent_task_budget` passes. |
| P2-1 fail-open is wider than store unavailable | Accepted residual (user-approved) | Stale-revision retry, 3 attempts (`task_budget.rs:320`). Store and busy errors still fail open. |
| P2-2 one overshoot blocks later calls | Accepted residual (user-approved) | Gated to limit and revision (`budget_reservation.rs:440-451`). |
| P2-3 unknown price not shown | Accepted residual (user-approved) | `unknown_price_records` in the snapshot (`budget.rs:363`). No UI. |
| P2-4 corrupt limits file means unlimited | Accepted residual (user-approved) | Logged with `warn!`. |
| P2-5 recovered orphans have no duration | Fixed for new reservations (N3) | See N3 above. |
| P3-1 unchecked revision increment | Fixed | `checked_add` with error in `ensure_policy`. |
| P3-2 Replay outcomes not denials | Fixed | See N5 above. |
| P3-3 settlement provenance | Fixed | Label `estimate-without-usage` (`task_budget.rs:352`). |
| P3-4 shared temporary file | Fixed | UUID in the temporary name. |
| P3-5 denial text is technical | Accepted residual (user-approved) | See N4 above. |

## Open findings and new findings

Counted against PASS: R3-1 (P2). Explicit accepted residuals (not counted): r1 P2-1, P2-2, P2-3, P2-4, P3-5 and r2 N2. Open P3 items (not counted): N4, N6, N7, R3-2, R3-3.

### R3-1 (P2, open): same-boot unsettled reservation under a limit blocks the task until restart, and raising the limit does not clear it

- Locations:
  - `crates/agentz-task-state/src/budget_reservation.rs:374`: `reconcile_orphans` skips records whose `boot_id` equals the current boot, so they are never reconciled in this process.
  - `crates/agentz-task-state/src/budget_reservation.rs:490-512`: `Reserved` and `Started` records count toward slots and toward every enforced limit.
  - `crates/agentz-task-state/src/budget_reservation.rs:526-527`: `slots > policy.max_in_flight` is applied whenever a limit is enforced.
  - `src-tauri/src/runtime/task_budget.rs:336`: a failed `release_unstarted` is ignored (`let _ =`).
  - `src-tauri/src/runtime/task_budget.rs:381-382`: a failed settlement is logged and dropped.
  - `src-tauri/src/runtime/task_budget.rs:158`: `max_in_flight` carries into every new revision, so a limit raise does not free the slot.
- Trace: a settle or release that fails with a storage error leaves a same-boot `Started` or `Reserved` record. It holds one of the 4 slots (`MAX_IN_FLIGHT`) for the rest of the process. After four such records, every call on a limited task is refused with `budget denied: task resource budget exhausted`. The denial text names a token or time budget, but the real cause is the slot cap. A revision raise keeps the slot cap, so the block persists.
- Evidence: probe, limited control. After four same-boot reservations were started and never settled, the fifth reserve under `max_total_tokens = 1000` returned `Err("budget denied: task resource budget exhausted")`. The same flow under no limit returned `Ok(Created)`. The r3 fix covers only the unlimited case.
- Reachability: requires a storage error (for example, a busy timeout beyond 5 seconds) on settle or release, or a commit error whose transaction actually committed. This is the same fail-open trigger the user accepted in P2-1, but the consequence here is a persistent denial, not a skipped accounting step.
- Acceptance impact: "raising the limit lifts a block" is not met for this path.
- Suggested fix (either):
  - Track live operation ids in process memory (set on `reserve`, removed on settle or release). Reconcile same-boot records that are not live, or
  - Retry settlement and release with backoff before giving up, and state the limitation in the stop text.
  - Alternatively, the user may explicitly accept this as an additional residual. The reviewer does not accept on the user's behalf.

### R3-2 (P3, new): crash-recovered unlimited calls are charged a 10-minute active duration

- Locations: `crates/agentz-task-state/src/budget_reservation.rs:406` (recovery copies `record.resources.active_ms`), `src-tauri/src/runtime/task_budget.rs:283-284` and `:304` (unlimited reservation is `PER_CALL_ACTIVE_MS = 600000`, from `task_budget.rs:35`).
- Impact: a crash during an unlimited call records 600,000 ms of active time for that call, regardless of the real duration. If a limit is set later, these phantom minutes count against `known_active_ms`. The effect is conservative, not a denial by itself.
- Suggested fix: record elapsed time up to the crash when it is known, or document the conservative charge in the task-details text.

### R3-3 (P3, new): test gaps for the r3 changes

- No test runs an unlimited policy with same-boot unsettled slots. The probe covers it (`%TEMP%\az024r3-probe`), but the suite does not.
- The orphan test (`budget_reservation.rs:1249`) asserts status, reason and tokens, not the recovered `active_ms`. N3 therefore has no regression test.
- No test covers `ReserveOutcome::Replay` refusal (`task_budget.rs:316-318`) or `StartOutcome::Replay` refusal.
- No host test runs two concurrent `begin` calls under a limit (N2 residual).
- Suggested: add these four tests when R3-1 is addressed.

## Audit of the early return in `check_budget`

Early return (`budget_reservation.rs:453-455`) fires only when `max_total_tokens`, `max_active_ms` and `max_cost` are all `None`. Every check below it applies only to an enforced limit, so no limited policy skips a check.

| Check (in order) | Location | Skipped for unlimited? | Needed by a limited policy? |
| --- | --- | --- | --- |
| Overshoot block | `budget_reservation.rs:440-451` | Already gated by `enforces_limit` before the early return | Yes; unchanged |
| Cost price and currency | `:460-470` | Only `max_cost` set (never for unlimited) | Yes; unchanged |
| Unknown token records | `:477` | Only with `max_total_tokens` | Yes; unchanged |
| Unknown duration records | `:478-480` | Only with `max_active_ms` | Yes; unchanged |
| Record loop: `Unknown` or cross-boot `Started` refusal | `:483-496` | Yes, by design (r3 N1) | Only needed to keep sums knowable under a limit |
| Token, active and cost sums | `:498-520` | Yes | Yes; unchanged |
| Slot cap | `:526` | Yes, by design (r3 N1) | Yes; see R3-1 |

Unlimited path: `begin` (`task_budget.rs:242-343`), `finish` (`:345-386`), `Settlement::drop` (`:232-236`) and `settle` (`budget_reservation.rs` settle function) are not gated on the limit. The test `unlimited_default_records_usage_and_never_denies` passes and asserts `usage_count = 3` and `known_input_tokens = 6`.

Consistency of reserve, start, settle and drop:

- `reserve` inserts only `Reserved` records, after the same-operation replay check and the occupied-usage check.
- `start` runs `check_budget` against all other outstanding records and marks `Started` only after the check passes. A denial leaves the record `Reserved`, and `begin` then releases it (`task_budget.rs:336`).
- `settle` accepts `Started`, `Unknown` or `Settled` records and rejects `Reserved`. Recovery settles `Started` and releases `Reserved`, which is consistent with that rule.
- `Settlement` consumes its ticket exactly once, through `settle` or through `Drop`, so a dropped call is charged once.
- No `await` lies between `begin` and the `Settlement` guard, so a drop cannot fall between reserve and start.

## Acceptance criteria

| Criterion | Result |
| --- | --- |
| Exhaustion yields an explanatory denial starting with `budget denied:` | Met for prefix. Every policy denial carries the prefix (`budget_reservation.rs:704, 756`; `task_budget.rs:288, 317, 333`). Explanation is still terse (N4, P3-5). Note: the token path also says "exhausted" when known usage is below the limit, because the check counts the worst case of the next call (reserved estimate plus `max_tokens`). |
| Resume does not reset the task total | Met. `ensure_policy` revises only when limits differ (`task_budget.rs` `ensure_policy`). Test `exhausted_limit_stops_next_call_and_raised_limit_resumes_without_reset` passes and asserts `known_input_tokens = 2` after the revision. |
| Unknown price is explicit | Met in the store snapshot (`budget.rs:363`). Accepted residual P2-3 for UI. |
| Unlimited default never denies | Met for normal flow and for same-boot, cross-boot and unknown records (N1). Exception: R3-1 applies only under a limit; storage-error fail-open is accepted. |
| Raising the limit lifts a block | Met for exhaustion and overshoot (new revision). Not met for the slot-cap block (R3-1). Met for recovered duration (N3). |

## New findings summary

| ID | Severity | Status | Location |
| --- | --- | --- | --- |
| R3-1 | P2 | Open | `budget_reservation.rs:374, 490-512, 526`; `task_budget.rs:158, 336, 381-382` |
| R3-2 | P3 | Open | `budget_reservation.rs:406`; `task_budget.rs:35, 283-284, 304` |
| R3-3 | P3 | Open | Tests: `budget_reservation.rs:1249`; no test for R3-3 items |

## Commands and results

| Command | Result |
| --- | --- |
| SHA-256 of 7 manifest files | All 7 match the manifest. |
| `cargo test -p agentz-task-state --lib budget` | `ok. 25 passed; 0 failed; 125 filtered out`. Exit 0. |
| `cargo test -p agentz-desktop --lib task_budget` | `ok. 9 passed; 0 failed; 275 filtered out`. Exit 0. Tests: `unbound_scope_is_transparent`, `limits_round_trip_and_reject_negative_values`, `missing_store_fails_open_and_reaches_the_provider`, `sub_agent_scope_shares_the_parent_task_budget`, `exhausted_active_time_refuses_before_the_provider`, `active_time_limit_admits_calls_with_a_bounded_reservation`, `exhausted_limit_stops_next_call_and_raised_limit_resumes_without_reset`, `unlimited_default_records_usage_and_never_denies`, `dropped_calls_are_settled_and_do_not_hold_slots_or_budget`. |
| Scratch probe `%TEMP%\az024r3-probe` (path dependency on `agentz-task-state`, `CARGO_TARGET_DIR` set to repo `target`, lock file copied from repo) | See below. |

Probe results (`cargo run`, exit 0):

```
== N1 unlimited: 4 same-boot unsettled Started, then 5th reserve ==
  start op0..op3: Ok(())
  fifth reserve (unlimited): Ok(Created)
== N1 control: same flow under max_total_tokens=1000 (limited) ==
  start op0..op3: Ok(())
  fifth reserve (limited tokens): Err("budget denied: task resource budget exhausted")
== N2: two concurrent-shaped reservations, max_active_ms=1_000_000 ==
  first reserve 600000ms: Ok(Created)
  second reserve 600000ms: Err("budget denied: task resource budget exhausted")
== N2: two concurrent-shaped reservations, max_active_ms=60_000 (host bound = remaining) ==
  first reserve 60000ms: Ok(Created)
  second reserve 60000ms: Err("budget denied: task resource budget exhausted")
```

Clippy was not run, per the brief.

## Verdict basis

No P0 or P1 is open. One P2 is open: R3-1, a limited-policy variant of r1 P1-1 and r2 N1 that the r3 fix does not cover. It is not in the user's accepted-residual list, so the round does not pass until it is fixed or explicitly accepted. The accepted residuals (r1 P2-1 through P2-4, P3-5, and r2 N2) are listed above and are not counted.

VERDICT: FAIL
