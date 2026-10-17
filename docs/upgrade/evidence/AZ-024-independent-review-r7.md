# AZ-024 Independent Review, Round 7

- Reviewer: independent-reviewer-az024-r7
- Task: AZ-024, task-level resource and cost budget
- Manifest: `docs/upgrade/evidence/AZ-024-closure-manifest.json` (revision closure-r7, 10 files)
- Prior review: `AZ-024-independent-review-r6.md` (PASS, covered the core and host budget path)
- Date: 2026-10-12
- Constraints followed: no source file and no `docs/upgrade/plan.json` modified. The only file written is this review. No scratch probe or temporary test was added. Findings come from code reading, the two required test commands, and read-only grep and git queries.

## Scope

r7 delta over r6: task-bound auxiliary wiring.

- `src-tauri/src/commands/chat_turn.rs`: `maybe_llm_rename_session_title` takes `budget: BudgetScope` and guards its client (`:201-237`). The title spawn and the post-turn spawn each receive a clone of `budget_scope` (`:2927-2935`, `:2956-2968`). `budget_scope.bind` runs after `early_binding` (`:2518-2523`, `early_binding` at `:2134-2143`).
- `src-tauri/src/commands/post_turn.rs`: `run_post_turn_hooks` takes `budget`, guards `mem_client` (`:26-30`), and passes `budget` to `run_background_skill_review` (`:42-53`).
- `src-tauri/src/commands/skill_review.rs`: `run_background_skill_review` takes `budget` and guards its client (`:82-93`, `:144`).
- `src-tauri/src/commands/gateway.rs`: IM turn passes `BudgetScope::default()` (`:716-728`), an unbound scope, so it is transparent and is an explicit residual.
- Also read as callers of changed signatures: `tools/delegate.rs`, `tools/call_fish.rs`, and `chat_turn.rs` registry builders.

Review checks (from the brief):

1. Every call the wiring claims to guard is routed through the guard and the bound scope.
2. No compile-time or logic regression in call signatures (all callers grepped).
3. Post-turn ordering: calls after delivery are fail-open, and `AZ-024-closure-evidence.md` section 4 item 1 describes this accurately.
4. No new P0/P1/P2 in the 10 files.

## Manifest hashes

SHA-256 recomputed with `Get-FileHash -Algorithm SHA256` and compared with the manifest. All 10 match.

| File | SHA-256 (first 16) | Match |
| --- | --- | --- |
| `crates/agentz-task-state/src/budget.rs` | `CF572872...` | yes |
| `crates/agentz-task-state/src/budget_reservation.rs` | `B2416177...` | yes |
| `src-tauri/src/runtime/task_budget.rs` | `54D00522...` | yes |
| `src-tauri/src/runtime/mod.rs` | `600B2F79...` | yes |
| `src-tauri/src/commands/chat_turn.rs` | `42EFB393...` | yes (changed since r6, `45BA4C01...`) |
| `src-tauri/src/tools/delegate.rs` | `50909C46...` | yes |
| `src-tauri/src/tools/call_fish.rs` | `2D88A4C4...` | yes |
| `src-tauri/src/commands/post_turn.rs` | `62E88F04...` | yes (r7 change) |
| `src-tauri/src/commands/skill_review.rs` | `C9F89C3D...` | yes (r7 change) |
| `src-tauri/src/commands/gateway.rs` | `951AB1DA...` | yes (r7 change) |

The manifest lists the same 10 paths, and each recomputed hash equals its manifest entry.

## Check results

### Check 1: guard and bound-scope routing

| Call | Guard | Scope | Result |
| --- | --- | --- | --- |
| Auto-title, `maybe_llm_rename_session_title` | `chat_turn.rs:237` (`budget.guard(client)`) | `budget_title` clone of `budget_scope` (`:2927`, passed `:2935`) | Routed. `complete` at `:255` uses the guarded client |
| Post-turn memory extraction | `post_turn.rs:26` (`mem_client`) | `budget` from `chat_turn.rs:2956-2968` | Routed. `memory_extract.rs` uses only the passed client (`:12`, `:68`) |
| Background skill review | `skill_review.rs:144` | `budget` passed from `post_turn.rs:53` | Routed. The only `client.complete` in the function is at `skill_review.rs:157` |
| Sub-agent (delegate, call_fish) | `chat_turn.rs:1452` | `DelegateTool` / `CallFishTool` clone `budget_scope` (`:1240`, `:1254`, `:1264`), passed by `delegate.rs:93` and `call_fish.rs:146` | Routed (unchanged since r6) |
| Main turn | `chat_turn.rs:2523` | `budget_scope.bind` at `:2518` | Routed (unchanged since r6) |
| IM / Gateway turn | `post_turn.rs:26` guards, but scope is `default()` | `gateway.rs:728` | Transparent. Explicit residual |

The bind happens at `chat_turn.rs:2518`, before both spawns (`:2927`, `:2956`). `BudgetScope` is `Arc<OnceLock<Bound>>` (`task_budget.rs:171`), so clones see the bound state. The guard reads the scope when it runs, and the spawned tasks run after the bind. Result: the three claimed calls are routed to the bound task.

Routing is confirmed by code reading. No test exercises the three auxiliary paths (see R7-3).

### Check 2: signatures and callers

Repo-wide grep (excluding `.cache`) for each changed function, with the result:

| Function | Callers | Arguments |
| --- | --- | --- |
| `maybe_llm_rename_session_title` | `chat_turn.rs:2929` (only caller) | passes `budget_title` (`:2935`) |
| `run_post_turn_hooks` | `chat_turn.rs:2958`, `gateway.rs:716` | `budget_post` (`:2968`); `BudgetScope::default()` (`gateway.rs:728`) |
| `run_background_skill_review` | `post_turn.rs:42` (only caller) | passes `budget` (`:53`) |
| `build_tool_registry` | `chat_turn.rs:1934` (main, `budget_scope`), `chat_turn.rs:1318` (sub-agent, `BudgetScope::default()` at `:1338`) | arity matches (last param) |
| `build_subagent_registry` | `chat_turn.rs:1400` only | unchanged |
| `run_subagent_research` | `delegate.rs:84` only | passes `self.budget.clone()` (`delegate.rs:93`) |
| `run_subagent_with_prompt` | `call_fish.rs:136`, `chat_turn.rs:1358` | passes `self.budget.clone()` (`call_fish.rs:146`) and `budget` |
| `DelegateTool { .. }` / `CallFishTool { .. }` literals | `chat_turn.rs:1239`, `:1253`, `:1263` | all three set `budget` |

No caller is missing the new argument. The lib test build (`cargo test -p agentz-desktop --lib`) compiled with these signatures, so there is no compile-time regression in the lib crate.

### Check 3: post-turn ordering and fail-open

Ordering in `run_agentz_turn`:

1. `run.finish(...)` at `chat_turn.rs:2842`. `ManagedRun::finish` moves the execution to a terminal state (`durable_run.rs:168-181`), and `transition` sets `delivery` to `awaiting_review` for `succeeded` or `not_ready` otherwise.
2. `dispatch` of `agent_final` and the status reset (`:2900-2918`).
3. Spawns for title (`:2927`) and post-turn (`:2956`), gated by `if !turn_failed`.

The spawns always come after `run.finish`. Therefore, by the time the budget is consulted:

- `reserve` runs `current()` (`budget_reservation.rs:672`), which requires `execution == running` and `delivery == not_ready` (`:353`). Both fail. Result: `Err("reservation requires current Running undelivered Attempt")`, which is not a denial, so `denied_or_open` fails open (`task_budget.rs:307-313`).
- If a limit has an exhausted active-time budget, the host pre-check at `task_budget.rs:330-336` runs before `reserve` and returns `budget denied: task active-time budget exhausted`. This is a denial, not fail-open (see R7-2).
- Stale binding cannot charge a later attempt. A retry creates a new attempt (`durable_run`), and `current()` compares `attempt_id` (`:349-353`).

Closure evidence section 4 item 1 says: "auto-title, memory extraction, and skill review, when triggered after turn end or after delivery, are refused by the reservation and pass fail-open, uncharged, with only a warning logged." This is accurate for the mechanism. It does not say that in the current code these three calls are always triggered after `run.finish`, and it omits the active-time denial exception. Both are wording gaps (R7-1, R7-2). The fail-open behaviour itself matches the code and is covered by the owner's accepted residual 1.

### Check 4: new P0/P1/P2 in the 10 files

No P0, P1, or P2 found. Four P3 items are recorded below (none counted against PASS).

## Per-finding status

### r7 brief items

| ID | Sev | Status | Evidence |
| --- | --- | --- | --- |
| Check 1: claimed guards routed | n/a | Pass (code reading) | Table in Check 1 |
| Check 2: signatures and callers | n/a | Pass | Table in Check 2; lib test build compiles |
| Check 3: post-turn fail-open accurate | P3 | Accurate mechanism; wording gaps R7-1, R7-2 | `chat_turn.rs:2842` before `:2927`; `budget_reservation.rs:672`, `:353`; `task_budget.rs:330-336` |
| Check 4: no new P0/P1/P2 | n/a | Pass | Findings below are P3 |

### r6 items, carried forward (r7 changes do not affect them)

The r7 delta does not touch `budget_reservation.rs` or the core (hash matches). `task_budget.rs` changed only through r6 and is unchanged in r7 (hash matches). The r6 PASS basis still holds for those files.

| ID | Sev (r6) | r7 status |
| --- | --- | --- |
| R5-1 | P2 | Still fixed for (a) to (d). Case (e) residual R6-3 |
| R5-2, R5-3, R5-4 | P3 | Unchanged, accepted |
| R6-1, R6-2, R6-4, R6-5 | P3 | Unchanged, open or noted, not counted |
| N2, R3-2, R4-2, R4-4 | Accepted | Unchanged |

## New findings

### R7-1 (P3, doc precision, not counted): auxiliary calls described as "charged"; in practice never charged

- Location: `docs/upgrade/evidence/AZ-024-closure-manifest.json` (`note`); `docs/upgrade/evidence/AZ-024-closure-evidence.md` section 2 (`chat_turn.rs` bullet) and section 4 item 1; code at `commands/chat_turn.rs:2842` (`run.finish`) before `:2927` and `:2956` (spawns).
- Finding: the manifest note says the auto-title, post-turn memory, and skill review calls are "charged to the parent task scope". Section 2 says the same. In the current ordering, all three are spawned after the attempt is terminal, so `current()` always refuses them (`budget_reservation.rs:353`, `:672`). The wiring routes the calls through the guard but enforces nothing for them. They run unbudgeted with a warning.
- Impact: no overspend beyond what the owner already accepted (accepted residual 1). The risk is a misleading statement to the owner about coverage.
- Required wording fix before closure: state that, with the current `chat_turn.rs` ordering, these three calls are always refused by delivery state and are uncharged. Do not call them "charged".

### R7-2 (P3, doc precision, not counted): fail-open is not uniform across limit types

- Location: `runtime/task_budget.rs:330-336` (active-time pre-check, returns `budget denied:`); `budget_reservation.rs:672` (`current()` before `check_budget` at `:703`).
- Finding: with an exhausted `max_active_ms` limit, the active-time pre-check returns a denial before `reserve`, so the auxiliary call is refused with `budget denied:` and is not fail-open. With an exhausted token limit, `current()` runs first and refuses with a non-denial error, so the call is fail-open. Section 4 item 1 says fail-open without this exception.
- Impact: auxiliary calls can be skipped when the active-time budget is exhausted (no user-facing error; title or memory is skipped). No overspend.
- Fix: document the exception in section 4 item 1.

### R7-3 (P3 test gap, not counted): auxiliary guard routing has no executed test

- Location: `runtime/task_budget.rs` test module (10 tests run, none of which calls `maybe_llm_rename_session_title`, `run_post_turn_hooks`, or `run_background_skill_review`).
- Finding: the r7 routing (Check 1) is confirmed by code reading and compilation only. No test shows that a title, memory, or skill-review call reaches `BudgetedClient`. Matches R5-3 (no begin-level test).
- Not blocking: the calls are refused by `current()` in the current ordering, so a test would only confirm the refusal path.

### R7-4 (P3, pre-existing at HEAD, outside the r7 delta, not counted): sub-agent registry registers `delegate`

- Location: `commands/chat_turn.rs:1231-1243` (plan-mode block, `DelegateTool` registration at `:1239`); `:1318` (sub-agent registry calls `build_tool_registry` with `chat_mode = "plan"`); comments at `:1217-1219` and `:1301-1304` say the sub-agent registry omits `delegate`.
- Finding: because `build_subagent_registry` passes `"plan"`, the plan-mode block runs and registers `DelegateTool` for the sub-agent. `git show HEAD:src-tauri/src/commands/chat_turn.rs` shows the same plan-mode registration, so this predates r7. I did not trace whether `run_subagent_with_prompt` filters the tool out later. A grep for `"delegate"`, `unregister`, `retain(` and `remove(` in `chat_turn.rs` found no filter.
- Impact: sub-agent recursion is still charged to the same `BudgetScope` (the scope is shared), so there is no budget bypass. Depth is bounded only by iteration and timeout limits.
- Action: owner to track separately. Not part of AZ-024 r7.

## Residuals (owner-listed; not counted as open)

Listed by the owner as accepted pending decision. Not counted against PASS.

- Delivery-state fail-open for post-turn calls (see R7-1 and R7-2 wording).
- Unguarded global maintenance: curator, memory consolidate, inline edit.
- Unbound IM / Gateway turns (`gateway.rs:716-728`).
- Koi: no client constructor found in `src-tauri`.
- N2: small concurrent active limits.
- R3-2, R4-2, R4-4, R5-2, R6-1, R6-2, R6-5.
- Corrupt limits file falls back to unlimited.
- English denial strings.
- No UI for limits.
- Cost limits not enabled.

## Commands and results

| Command | Result |
| --- | --- |
| SHA-256 of 10 manifest files (`Get-FileHash -Algorithm SHA256`) | All 10 match the manifest |
| `cargo test -p agentz-desktop --lib task_budget` | `test result: ok. 10 passed; 0 failed; 0 ignored; 275 filtered out`. Exit 0 (PowerShell printed a `NativeCommandError` wrapper for cargo's stderr "Finished" line; the exit code is 0) |
| `cargo test -p agentz-task-state --lib budget` | `test result: ok. 25 passed; 0 failed; 0 ignored; 125 filtered out`. Exit 0 (same PowerShell wrapper note) |
| Repo-wide grep for changed function and struct callers (excluding `.cache`) | Table in Check 2 |
| `git show HEAD:src-tauri/src/commands/chat_turn.rs` grep for plan-mode `DelegateTool` | Registration present at HEAD (supports R7-4) |
| `git diff --stat HEAD` on the six changed `src-tauri` files | 6 files, 451 insertions, 70 deletions. `docs/upgrade/plan.json` is untracked and was not modified |
| Clippy | Not run, per the brief |
| Fault-injection or aux-path test | Not run. No code change allowed |

## New-finding summary

| ID | Severity | Status | Location |
| --- | --- | --- | --- |
| R7-1 | P3 | Open (doc wording, not counted) | `AZ-024-closure-manifest.json` note; `AZ-024-closure-evidence.md` §2, §4 item 1; `chat_turn.rs:2842` before `:2927`, `:2956` |
| R7-2 | P3 | Open (doc wording, not counted) | `task_budget.rs:330-336`; `budget_reservation.rs:672`, `:703` |
| R7-3 | P3 | Test gap (not counted) | `task_budget.rs` tests; no aux-path test |
| R7-4 | P3 | Pre-existing, outside r7 delta (not counted) | `chat_turn.rs:1231-1243`, `:1318`, `:1217-1219`, `:1301-1304` |

No new P0, P1, or P2 findings in the ten reviewed files.

## Verdict basis

- All 10 manifest hashes match. Both required test filters pass (10/10 and 25/25).
- Check 1: the three auxiliary calls are routed through `budget.guard` on the bound scope. The IM turn is transparent and explicitly residual.
- Check 2: every caller of the changed signatures passes the new argument. The lib test build compiles with them.
- Check 3: the ordering is confirmed (`run.finish` before the spawns). The fail-open mechanism in section 4 item 1 matches the code. Two wording gaps are recorded as R7-1 and R7-2, neither a code defect.
- Check 4: no new P0, P1, or P2 in the ten files.
- Owner should know before accepting closure: with the current ordering, the r7 auxiliary wiring never charges the task (R7-1). The accepted residual covers this, but the wording in the manifest and in section 2 should be corrected.

VERDICT: PASS
