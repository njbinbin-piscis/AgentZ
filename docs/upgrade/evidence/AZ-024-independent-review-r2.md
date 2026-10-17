# AZ-024 Independent Review, Round 2

- Reviewer: independent-reviewer-az024-r2
- Task: AZ-024, task-level resource and cost budget
- Manifest: `docs/upgrade/evidence/AZ-024-closure-manifest.json` (revision closure-r2)
- Prior review: `docs/upgrade/evidence/AZ-024-independent-review.md` (independent-reviewer-az024, FAIL)
- Date: 2026-10-12
- Constraints followed: no source file and no `docs/upgrade/plan.json` modified. Only this review file was written. The scratch probe lives in `%TEMP%`, outside the repository. Cargo build output went to the repository's `target` directory (build artifacts only).

## Scope

SHA-256 recomputed for the 7 files listed in the closure-r2 manifest and compared with the manifest. All 7 match.

| File | SHA-256 (first 16) | Lines | Match |
| --- | --- | --- | --- |
| `crates/agentz-task-state/src/budget.rs` | `CF572872...` | 905 | yes |
| `crates/agentz-task-state/src/budget_reservation.rs` | `ED3CBA25...` | 1456 | yes |
| `src-tauri/src/runtime/task_budget.rs` | `4C40352D...` | 766 | yes |
| `src-tauri/src/runtime/mod.rs` | `600B2F79...` | 26 | yes |
| `src-tauri/src/commands/chat_turn.rs` | `45BA4C01...` | 3010 | yes |
| `src-tauri/src/tools/delegate.rs` | `50909C46...` | 105 | yes |
| `src-tauri/src/tools/call_fish.rs` | `2D88A4C4...` | 159 | yes |

The working tree contains many modifications outside AZ-024 (for example `src-tauri/src/commands/symbols.rs`, `approval.rs`, `skills/service.rs`). They were not audited, per the brief.

Related code read to trace behaviour: the kernel agent loop (`vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs`, read-only tool concurrency at lines 54 and 3900-3915), the harness (`vendor/piscis-engine/piscis-kernel/src/agent/harness/config.rs`), the run and owner-lease code (`crates/agentz-task-state/src/run.rs`, `src-tauri/src/runtime/owner_lease.rs`), and the auto-title client (`src-tauri/src/commands/chat_turn.rs:201-240`).

## Round 1 finding status

| r1 ID | r2 status | Evidence |
| --- | --- | --- |
| P1-1 stopped or timed-out call leaks reservation | Partially fixed | Drop path fixed. `Settlement` (`src-tauri/src/runtime/task_budget.rs:217-233`) settles on drop, and both `stream` (~383-400) and `complete` (~411-421) use it. Test `dropped_calls_are_settled_and_do_not_hold_slots_or_budget` passes. Open remainder: a failed settle or release leaves a same-boot reservation that holds a slot even under an unlimited policy. Reproduced by probe B. See N1. |
| P1-2 configured `max_active_ms` refuses every call | Fixed | `begin` sets the per-call bound to `limit.saturating_sub(known_active_ms)` and refuses at `<= 0` (`task_budget.rs:276-289`, reserve at ~300). Serial arithmetic is correct: a single call fits exactly (`check_budget` uses `>`). Tests `active_time_limit_admits_calls_with_a_bounded_reservation` and `exhausted_active_time_refuses_before_the_provider` pass. Concurrency over-reservation is a new defect. See N2. |
| P1-3 research sub-agent calls bypass budget | Fixed | Shared `BudgetScope` (`task_budget.rs:164-200`). Sub-agent client guarded at `chat_turn.rs:1448-1449` and passed to `into_agent_loop` at `chat_turn.rs:1472`. Registered via `delegate.rs:29,93` and `call_fish.rs:30,146`, and built with the parent's scope at `chat_turn.rs:1237-1261`. Parent binds at `chat_turn.rs:2515` and guards at `2520`. Test `sub_agent_scope_shares_the_parent_task_budget` passes. No other sub-agent client path found in `src-tauri/src` (`build_subagent_registry` callers are the two research entry points). |
| P2-1 stale-revision race fail-open | Partially fixed | Stale-revision errors retry up to 3 times with a fresh query (`task_budget.rs:263-317`). Safe: `reserve` checks the revision before inserting, inside the same `IMMEDIATE` transaction (`budget_reservation.rs:670-672` before the insert at ~702), so a retry cannot double-insert. Store-error fail-open at the reserve step is accepted per the user's explicit decision. The settlement-failure consequence is tracked as N1. |
| P2-2 overshoot blocks later calls | Residual accepted | User-approved design. Block is scoped to the revision where it occurred and only while a limit is enforced (`budget_reservation.rs:438-451`). Raising the limit creates a new revision (`revise_policy`). The denial text still gives no action; see N4. |
| P2-3 unknown price not shown | Residual accepted | User-approved residual. The host never exposes a cost limit (`max_cost: None` in `ensure_policy`, `cost: None` in every measurement), so no decision depends on price. `unknown_price_records` remains in the store snapshot (`budget.rs`). No UI surface yet. |
| P2-4 corrupt limits file means unlimited | Residual accepted | User-approved residual. Logged with `warn!` (`task_budget.rs:51-66`). |
| P2-5 recovered orphans have no duration | Not fixed (open) | Reproduced by code reading. See N3. |
| P3-1 unchecked revision increment | Fixed | `checked_add` with an error (`task_budget.rs:145`). |
| P3-2 Replay outcomes not denials | Partially fixed | `StartOutcome::Replay` is now a denial (`task_budget.rs:325`). `ReserveOutcome::Replay` is still treated as success (`task_budget.rs:311`, `Ok(_) => break`). Unreachable with fresh UUID operation ids. See N5. |
| P3-3 settlement provenance | Fixed | Label is now `estimate-without-usage` (`task_budget.rs:344`), which matches what is recorded. Charging the full estimate on failure is conservative by design. |
| P3-4 shared temporary file name | Fixed | Unique temporary name with a UUID (`task_budget.rs:76`). |
| P3-5 denial text is English and technical | Residual accepted | User-approved residual. Note the active-time path, N4. |

## New findings

### N1 (P2): same-boot unsettled reservations hold slots and block unlimited tasks

This is the open part of r1 P1-1. A reservation that a failed settle or release leaves in `Reserved` or `Started` under the current boot is never reconciled until restart. It counts against the slot cap, which is enforced whether or not a limit is set.

- Slot cap applied unconditionally: `crates/agentz-task-state/src/budget_reservation.rs:522-523` (`if slots > policy.max_in_flight || ...`).
- Same-boot `Started` and `Reserved` records counted as in-flight: `budget_reservation.rs:480-491`. `reconcile_orphans` skips same-boot records (`budget_reservation.rs:373`).
- Settlement failure is logged and dropped: `src-tauri/src/runtime/task_budget.rs:371-375`.
- Release failure is ignored: `src-tauri/src/runtime/task_budget.rs:328` (`let _ = store.release_unstarted(...)`).

Probe B (scratch, outside repo): an unlimited policy, four reservations started and never settled in the same boot, then a fifth call returns `Err("budget denied: task resource budget exhausted")`. This violates the acceptance criterion "unlimited default never denies". The trigger is a storage error during settlement or release, which is the non-denial path the user accepted as fail-open. Once it fires, the leak persists until restart.

Suggested fix: when the policy is unlimited, do not apply the slot cap. Reconcile same-boot `Started` and `Reserved` records whose operation is no longer in this process (for example, track live tickets in memory and settle orphans when the ticket is gone), or retry settlement before logging.

### N2 (P2): active-time reservation sized to full remaining budget refuses concurrent calls

`begin` reserves the whole remaining active budget for every call (`task_budget.rs:276-289, 300`). `check_budget` sums all outstanding `active_ms` plus the proposed one (`budget_reservation.rs:503-510`) and compares to the limit (`522-525`). So while one call is in flight, a second call is refused even when no active time has been used.

Read-only tools run concurrently, up to 4 at a time (`vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs:54, 3900-3915`). The `delegate` tool description invites parallel use. With `max_active_ms` set, two parallel `delegate` or `call_fish` calls therefore produce one refusal, `budget denied: task resource budget exhausted`, and the sub-agent fails.

Probe A (scratch, outside repo): first reservation `Ok(Created)`, second `Err("budget denied: task resource budget exhausted")`, `in_flight=1`, `known_active_ms=0`. The first reservation did not fail; the defect is only in the concurrent case.

Suggested fix: bound each per-call reservation by a per-call maximum (for example, the remaining budget divided across the concurrency cap), or account for concurrency in the bound. Add a host test that runs two parallel reservations under `max_active_ms`.

### N3 (P2): crash-recovered records without duration permanently block a later active limit (r1 P2-5)

Recovery writes a usage record with `active_ms: record.resources.active_ms` (`budget_reservation.rs:406`). For a call made while the task was unlimited, that is `None`. `budget.rs:349-353` then increments `unknown_duration_records`. Once the user sets `max_active_ms`, `check_budget` refuses every call with `historical duration is unknown` (`budget_reservation.rs:474-476`). Historical counters do not reset, so raising the limit does not lift the block. This contradicts the acceptance criterion "raising the limit lifts a block".

Reachable: a crash during an in-flight call on an unlimited task, followed later by a limit. Code-read finding, not probed.

Suggested fix: when recovering a record with no duration, record the elapsed time up to the crash, or record a conservative bound from the policy in force, so the counter is not left unknown.

### N4 (P3): active-time exhaustion usually surfaces as the overshoot message

A call admitted with `active_ms = remaining` that runs longer than `remaining` settles as an overshoot (`budget_reservation.rs:262-265`). That blocks the rest of the revision with `previous settlement exceeded reservation; reconciliation required` (`budget_reservation.rs:448-450`). The explanatory `task active-time budget exhausted` text (`task_budget.rs:284`) is therefore reached only if the budget is exhausted between calls. In the common case the stop reads as an internal reconciliation problem, not a budget stop. This is part of the P3-5 residual; the denial text should name the action (raise the limit).

### N5 (P3): `ReserveOutcome::Replay` is still treated as success

`task_budget.rs:311` (`Ok(_) => break`) treats a reserve replay as a fresh reservation and proceeds to `start`. The fix for P3-2 covered only `StartOutcome::Replay`. Unreachable with fresh UUID operation ids, but the code should refuse it the way `StartOutcome::Replay` is refused.

### N6 (P3): blocking SQLite work on the async path and in `Drop`

`begin` (`task_budget.rs:240-260`) and `finish` (`task_budget.rs:336-376`) open and write to SQLite synchronously inside the async `stream` and `complete`, and `finish` also runs in `Drop` on a user stop. Under contention, each call can block a tokio worker for up to the SQLite busy timeout. Performance concern, not a correctness defect.

### N7 (P3): session auto-title call is not budget-scoped

`maybe_llm_rename_session_title` (`src-tauri/src/commands/chat_turn.rs:201`) builds its own client (`chat_turn.rs:225`) and makes a model call outside the guarded client, so it is not reserved or counted. The call is small and runs once per session. Either guard it, or record it as out of scope for the task budget.

## Verified as correct

- Race between `BudgetScope.bind` and tool registration: none. The registry only clones the `Arc` (`chat_turn.rs:1237-1261`). Tools execute inside the agent loop, which starts after `bind` (`chat_turn.rs:2515` before `into_agent_loop` at `2522`).
- Sub-agent guard unbound when the parent failed to bind: fail-open, documented in the `BudgetScope` doc comment (`task_budget.rs:164-166`) and the module doc. Test `unbound_scope_is_transparent` passes. This is the user-accepted fail-open behaviour for a failed bind, which logs a warning.
- Settlement guard, panic path: `finish` has no `unwrap` or `panic` and returns `Result` everywhere, so a drop during unwinding does not panic.
- Double settlement: `Settlement::settle` consumes the guard and `take()`s the ticket, so `Drop` is a no-op afterwards. The store's `settle` is also idempotent on identical usage (`budget_reservation.rs:855-877`).
- Deadlock: none. No lock is held across an `await`; the reservation store opens a connection per call; `OnceLock` is never blocked on.
- Reserve retry loop: correct. Each attempt re-queries the snapshot. The stale check precedes the insert inside one `IMMEDIATE` transaction, so a retry does not duplicate a reservation. Three attempts are bounded.
- Remaining-active arithmetic for serial calls: `saturating_sub` plus the `<= 0` refusal is correct. An exact fit passes because `check_budget` uses `>`. Sums use checked helpers (`total`).
- Denied calls do not reach the provider on the guarded path (`task_budget.rs:306`, `325`, `328`).
- Main chat client: built at `chat_turn.rs:2324` and guarded at `2520`. Compaction, summary, and fallback-model calls use the same client inside the kernel loop.
- Vision: no separate vision client is passed to the main loop (`into_agent_loop(client, None, None)` at `chat_turn.rs:2522`).

## Acceptance criteria

| Criterion | Result |
| --- | --- |
| Explanatory stop on exhaustion | Partially met. Token exhaustion gives `budget denied: task resource budget exhausted`, which names the budget but gives no numbers or action (accepted P3-5 residual). Active-time exhaustion usually surfaces as the overshoot text (N4). |
| Resume does not reset the task total | Met. `ensure_policy` only revises the policy when limits differ and never resets usage. Test `exhausted_limit_stops_next_call_and_raised_limit_resumes_without_reset` passes. Usage is read from the store on each call, so it survives restart. |
| Unknown price explicit | Met only in the store snapshot. Accepted P2-3 residual: no cost limit is exposed, and no UI surface yet. |
| Unlimited default never denies | Not met under storage errors (N1). The unlimited path is clean in the normal flow. Test `unlimited_default_records_usage_and_never_denies` passes. |
| Raising the limit lifts a block | Met for overshoot and exhaustion (new revision). Not met for recovered-duration records (N3). |

## Commands and results

| Command | Result |
| --- | --- |
| `cargo test -p agentz-task-state --lib budget` (clean re-run) | ok. 25 passed, 0 failed, 125 filtered out. Exit 0. |
| `cargo test -p agentz-desktop --lib task_budget` (clean re-run) | ok. 9 passed, 0 failed, 275 filtered out. Exit 0. Test list: `unbound_scope_is_transparent`, `limits_round_trip_and_reject_negative_values`, `missing_store_fails_open_and_reaches_the_provider`, `sub_agent_scope_shares_the_parent_task_budget`, `exhausted_active_time_refuses_before_the_provider`, `active_time_limit_admits_calls_with_a_bounded_reservation`, `exhausted_limit_stops_next_call_and_raised_limit_resumes_without_reset`, `unlimited_default_records_usage_and_never_denies`, `dropped_calls_are_settled_and_do_not_hold_slots_or_budget`. |
| SHA-256 of 7 manifest files | All 7 match the manifest. |
| Scratch probe (`%TEMP%\az024r2-probe`, path dependency on `agentz-task-state`, `CARGO_TARGET_DIR` pointing at the repo's `target`) | probe_a: first `Ok(Created)`, second `Err("budget denied: task resource budget exhausted")` (N2). probe_b: fifth reserve `Err("budget denied: task resource budget exhausted")` under an unlimited policy (N1). probe_b panicked on its `is_ok` assertion, which is the defect being demonstrated. Result: 1 passed, 1 failed (expected reproduction). |

Clippy was not run, per the brief.

## Summary of open items

| ID | Severity | Status | Action needed |
| --- | --- | --- | --- |
| N1 | P2 | Open (reopens r1 P1-1 residual) | Unlimited policy must not hit the slot cap; reconcile same-boot orphans |
| N2 | P2 | Open | Bound per-call active reservation for concurrency; add parallel test |
| N3 | P2 | Open (r1 P2-5) | Give recovered records a duration bound |
| N4 | P3 | Open | Explanatory text for active-time exhaustion |
| N5 | P3 | Open | Refuse `ReserveOutcome::Replay` |
| N6 | P3 | Open | Move blocking store work off the async path (or document) |
| N7 | P3 | Open | Guard or scope the auto-title call |

Accepted residuals (user-approved, not counted against PASS): r1 P2-1 store-error fail-open (reserve step), P2-2 overshoot, P2-3 unknown price, P2-4 corrupt limits file, P3-5 English denial text.

Verdict basis: N1, N2, and N3 are open P2 defects that are not accepted residuals, so the round does not pass.

VERDICT: FAIL
