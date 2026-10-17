# AZ044 terminal lifecycle checkpoint

2026-10-10. Read current terminal renderer and host registry; no Cargo/installer processes running. Existing AZ036/AZ021 reviewed candidates supply code dependencies; formal dependency/Gate acceptance remains pending.

Confirmed renderer reuses panel sequence 1 on every project change, so old cleanup/late creation can address a replacement project's same terminal ID. Changed allocation to monotonically increasing panel sequence across projects. Also added post-create cancellation cleanup and cancellation guard before scheduled fit/focus; startup errors now render in the terminal instead of becoming unhandled rejections.

This is an incomplete working change, not independently reviewed or accepted. TypeScript session 73029 pending. Visibility/callback effect cleanup still can restart a session under the same tab ID; host create/destroy dispatch ordering, reader event ownership, process descendants, close confirmation staleness and native multi-terminal GUI acceptance need further implementation/review. Do not infer a complete race fix from late cleanup alone. No native terminal runtime or process-reaping test executed.

## Visibility lifecycle working fix
Prior TypeScript session 73029 exited 0. Renderer now latches activation per project: hiding/revealing the panel and changing selection callback do not dispose/recreate its PTY. Project changes reset activation when hidden; output and scheduled focus reject cancelled effect callbacks. Callback/visibility refs provide current values without restarting initialization. New TSC 88137 and eslint 31579 pending; activation reset was refined while TSC ran so final exact-source check still required. Host duplicate-ID reservation, StrictMode restart and stop-before-create remain unresolved; no full lifecycle acceptance claimed.


## Serialized renderer ownership
88137 TypeScript exited 0; eslint31579 exited0 with two existing Fast Refresh warnings. Added renderer per-tab serialized create/dispose leases and integrated effect cleanup: pending creation completes its owned destroy before replacement creation; disposal is idempotent and cancellation before dispatch skips create. Session41461 two controlled async tests passed. Subsequent fail-closed refinement retains rejected queue IDs after creation/cleanup failure instead of allowing replacement over uncertain ownership; final test rerun and TSC11671 pending. This is renderer-local ordering, not cross-renderer/host authority or bounded IPC; unresolved native promises can still hold an ID indefinitely and no process-tree proof is claimed.


## Exact-source check correction and ledger drift
Final lease rerun74629 passed2. TSC11671 exited1: fixture Array.at unsupported by configured TS library; replaced with indexed access and added failed-cleanup replacement refusal test. Vitest98457 and final TSC74416 pending. Previous ledger update failed before save due shared CodeZ index.tsx fingerprint drift; preservation/audit recorded in AZ-044-shared-source-drift.md, no blind manifest refresh. Working edits remain limited to terminal component and new lifecycle helper/tests.


## Continued controlled verification
Final TypeScript74416 exited0. Added actual failed-create refusal and independent-ID progress tests; five-test rerun27258 and three-file ESLint39343 pending. The serialized helper protects only one renderer's effect ordering; does not prevent delayed old host output with reused ID, unknown external IPC actors or guarantee OS termination. Ledger summary itself refuses stale AZ036 source pins, so no numerical progress update asserted. Current shared source remains preserved; terminal work continues independently of deferred re-review.


## Actual component regression coverage
Session27258 five async lease tests passed; ESLint39343 exit0, two existing component Fast Refresh warnings. Added ReactTestRenderer fixtures exercising the real TerminalPanel with mocked xterm/Tauri/native surfaces: hide/show plus callback changes preserve one create and no destroy, and project switch allocates another ID and retires old session. Session15208 actual 7 tests passed (5 helper + 2 component), 6.61s. Final TypeScript70720 pending after adding component fixtures. This proves controlled renderer behavior only, not native PTY/GUI/process-tree acceptance. Ledger remains blocked by previously recorded unrelated reviewed-index fingerprint drift; no source reset or blind repin performed.


## Close confirmation ownership
TypeScript70720 exited0 on seven-test candidate. Added render-time project epoch guard around async close confirmation and membership guard inside tab update, preventing stale confirmation from acting on a replacement project/tab set. Existing seven tests rerun6183 passed. Added real component delayed-confirmation fixture; eight-test33525 result pending. Read-only independent current-index scoped review delegated to Codex subagent, no shared source edits permitted; origin of drift still unknown and ledger pins not blindly refreshed.


## Shared pin refresh withheld
Independent current-index review found no blocker within the existing navigation/editor scope, but all-consumer preflight found additional App.tsx/en.ts/zh.ts drift affecting AZ017/AZ036. No ledger save performed; exact current/expected hashes recorded in AZ-044-current-drift.json. Current-source independent review extended to these three files before any atomic refresh. Eight-test33525 passed; final TypeScript on updated close fixture started separately. Source/evidence old pins preserved. No acceptance/Gate promoted.


## Shared ledger consistency restored
Independent current-index plus App/relevant translation consumer reviews completed with exact SHA. Atomically refreshed AZ017/AZ036/AZ041 using new manifests and composite evidence, preserving previous pinned review files/history and uncertainty about historical drift origin. Final TypeScript87774 exited0; eight-test33525 passed. Terminal renderer/helper independent read-only review started; original terminal/OS/native acceptance remains pending.


## Independent review follow-up
Independent terminal review confirmed limited slice but identified same logical ID listener receiving retired native output. Working fix gives each effect a distinct crypto UUID backend ID and routes native listener/create/write/resize/destroy to it; logical tab lookup maps only current backend for close checks. Cleanup conditionally removes its own mapping. Original eight tests72120 passed; added retired-backend output refusal fixture (nine-test91006 pending). New read-only follow-up review started. Host APIs remain ID-based with no additional native authorization; process trees and cross-renderer permissions still unverified.


## Cleanup mapping and navigation fail-closed
69482 ten tests passed and63932 full TypeScript exit0 after mapping cleanup repair. Closed count-query failure and destroyAll swallow-error helpers: count failure now refuses navigation, destroyAll failure propagates to existing App catch; added controlled helper propagation case,47333 eleven tests passed. Scoped lint22787 found unused short-circuit expression (not a pass), replaced with explicit if and removed obsolete resize dependency; new lint running, final TypeScript2211 pending. Independent fail-propagation follow-up requested, no native kill success claimed.


## Final source freeze
76882 lint exited0 with2 existing FastRefreshwarnings;2211 TypeScript exited0. Exact four-source manifest generated; final 11-test rerun started after lint-only resize if/dependency cleanup. Final independent exact-source freeze follow-up pending. No Cargo/installer processes running; no native PTY/GUI acceptance. No source edits planned during final checks.


Final frozen rerun15715:11 tests passed,4.68s; final source manifest matches independent freeze review.
