# AZ044 terminal limited independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only inspection of four current terminal files and their controlled fixtures. No source, ledger or existing evidence was edited. Root reports final TypeScript session87774 exit0 and eight controlled tests passed; reviewer did not rerun these checks. No native PTY, actual xterm DOM, cross-renderer, OS descendant, GUI or rollback acceptance was performed.

Reviewed raw SHA256:
- src/workspaces/codez/Terminal.tsx: 866ada5ded7259d62f8c1c0818508d74ea7d60de13626128471eec843a56e1f8
- src/workspaces/codez/terminalLifecycle.ts: 4aec7aec3cee421e915e6b0c7d66ad6299cd953bc149c6e519209868186b4eaa
- src/workspaces/codez/terminalLifecycle.test.ts: 545a89eeb74a696019990e41882efa702c3137ad57d3f46c01a6fa7074cd1c1e
- src/workspaces/codez/Terminal.lifecycle.test.tsx: 84fd881e892f2f3b88e6dd75b8efea615f16e0a79e9ebb26cf9fac607cda1368

## Reviewed positive bounded behavior

acquireTerminal serializes create/cleanup per ID in a module-local promise chain. Synchronous dispose marks the lease cancelled before dispatch; pending create success performs its own cleanup before replacement creation can proceed. Disposal is idempotent. Rejected create or cleanup remains a rejected tail, so later same-ID creation cannot silently replace an uncertain backend session. Different IDs progress independently. Five real helper fixtures use deferred operations to check ordering, cancellation before dispatch, failed cleanup, failed create and independent IDs, rather than mirroring a copied implementation. Rejected tails remain retained indefinitely; there is no reconciliation/retry or global memory bound. A never-resolving create/cleanup blocks that ID without a deadline. This is fail-closed availability behavior, not verified native reclamation.

TerminalSession activation depends on first visible activation plus exact project identity. Hiding an activated same-project panel does not revoke activation or rerun the PTY effect, and changed selection callbacks are read through a ref rather than effect dependencies. Visibility refs constrain delayed focus; resize/theme concerns remain separate effects. Project change invalidates activation, and TerminalPanel allocates fresh increasing tab IDs on project reset. Local cancelled checks suppress callbacks belonging to a retired effect; late listener acquisition invokes its own unlisten. The three component fixtures mount the actual exported panel with mocked xterm/native APIs, checking visibility/callback preservation, different project IDs/old destroy calls and late close confirmation after project change.

closeTab captures a render-updated project epoch before awaiting alive/confirmation and checks it both before and within state mutation. Removed IDs and last-tab closure are rejected. This prevents an old project confirmation from deleting the replacement project's tab. It is not a native session lease check; same-project double-close and changed active selection are not tested. activeTabId is closure-captured, so selection made during an outstanding close can be redirected to the close callback's fallback when that previously active tab closes; this is a UI focus limitation, not proof of a wrong-project backend close.

## Required limits and remaining full-scope blocker

The output listener is installed before acquireTerminal.ready and matches only evt.id plus its effect's cancelled flag. A replacement effect using the same ID can listen while waiting for the older lease cleanup and can display output emitted by the old PTY. Host-delayed old same-ID output after replacement creation also cannot be identified. Module-local serialization does not supply a host generation token or cross-renderer ownership. Therefore old-event session isolation is not delivered by these changes and remains a blocker for full AZ044 acceptance; these eight fixtures do not cover or prove it. This finding was sent to root. Do not describe cancelled local callbacks as rejection of every prior-session native event.

terminalWrite/resize callbacks likewise send only ID, without native generation authority. Host create/delete ownership, partial-create side effects, process-tree termination and stop-before-IPC ordering require native lifecycle work and evidence. Panel IDs use a short random value, not collision-proof host-issued session identities. Local chains cannot protect separate JS realms.

Existing confirmTerminalCloseOnProjectChange returns true when terminalCount fails, and destroyAllTerminals catches destroy-all failures. Those helper functions retain best-effort behavior, so callers cannot infer verified absence or cleanup success from them. Per-tab failures poison the ID but cleanup errors are swallowed at React teardown; they are not user-visible remediation. These are explicitly open native/full-navigation requirements, not newly verified guarantees.

## Conclusion

The limited visibility/callback lifetime, per-ID renderer ordering and stale project close-confirmation candidate is reviewable with the recorded boundaries. No additional blocker was found for that restricted slice. It must remain preparing rather than full AZ044 complete: generation-owned old-event isolation, cross-renderer/native authority, failure reconciliation, bounded deadlines and actual OS/desktop acceptance remain open. Any source change requires a new fingerprint/follow-up review; historical results do not validate changed files.


# AZ044 backend identity follow-up independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only follow-up of current four terminal files; historical AZ-044-terminal-independent-review.md is preserved. No tests, typecheck, native PTY/GUI, cross-renderer or OS cleanup checks were run by reviewer. Root reports earlier eight tests passed; latest nine-test session91006 was pending at review request, and older TypeScript27225 does not cover final resize/test changes. No pending result is counted passed here.

Observed raw SHA256:
- Terminal.tsx: acb49ba68f7c174f3fdbef6f0e15beb546ac1e4b5637f7816baa54cffdca1276
- terminalLifecycle.ts: 60240eaae210cb2f0dc997e17f717253366cc2b29b28e2207e7974ca3cb9b912
- terminalLifecycle.test.ts: 545a89eeb74a696019990e41882efa702c3137ad57d3f46c01a6fa7074cd1c1e
- Terminal.lifecycle.test.tsx: f874d3c2a454b7dda3ef88c741b008d2755129fe871648fd30cf9459b1dd29a5

Every activated TerminalSession effect now allocates a fresh crypto.randomUUID-based native backend ID. Its listener filters that exact ID; term onData/onResize and create/destroy closures capture the same ID. Delayed fit uses backendIdRef for the current effect, and cleanup clears that ref. Logical tab ID is retained only for renderer serialization and logical lookup. An old destroy closure therefore targets its captured old backend rather than a replacement backend. Installing a new listener before old cleanup no longer permits old differently identified output to pass its filter. This fixes the concrete old-event same-ID pollution by allocating distinct native IDs rather than adding native ownership authority.

The new component fixture injects an old backend event into the new listener and verifies no write, then verifies matching new backend writes. It covers actual exported component filtering under project replacement with mocked APIs/xterm, not StrictMode replay/native event transport or all input routing. Earlier queue fixtures still cover logical ID ordering and fail-closed poisoned tails. closeTab translates logical tab ID through terminalBackendId before alive query and retains the project-epoch checks; it does not destroy a backend directly, so cleanup uses the corresponding effect closure.

## Blocking mapping cleanup finding at this observed revision

backendIds is installed before create and intentionally retained on uncertain create/destroy failure. But successful ordinary dispose only destroys and sets created=false; it never conditionally deletes backendIds. Matching deletion exists solely in the late-created-already-disposed ready path. Ordinary retired tabs therefore leave stale lookup entries and growing retained state. This contradicts claimed successful matching-map cleanup and requires a matching backendIds.get(id) === backendId deletion after successful cleanup, without deleting a newer owner's mapping or clearing failed-cleanup uncertainty. Finding sent to root; source was not modified by reviewer. A meaningful mapping cleanup/ownership fixture and new exact-hash follow-up are required before freezing this revised slice.

Backend identity freshness reduces accidental event/write/destroy reuse but still grants no opaque native authority, cross-renderer lease or process-tree proof. Module-local poisoning/deadline and existing terminalCount/destroyAll best-effort limitations from the previous review remain. Close confirmation is for a logical tab and does not prove native generation remained unchanged during the dialog. No full AZ044 or stable release acceptance follows from this review.


# AZ044 final bounded terminal candidate independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only follow-up of final terminalLifecycle.ts and its test change, with all four candidate hashes recomputed. Previous independent and follow-up evidence is preserved. No source or ledger changes; only this new evidence file is written. Root reports final session69482 ten tests passed (six helper/four component) and full TypeScript session63932 exit0. These are root-run results, not reviewer reruns or native acceptance.

Final raw SHA256:
- src/workspaces/codez/Terminal.tsx: acb49ba68f7c174f3fdbef6f0e15beb546ac1e4b5637f7816baa54cffdca1276
- src/workspaces/codez/terminalLifecycle.ts: 60d7e6eab42c4dceea6d120c6dfc66a381b08031193cc66c846a77548f1acd53
- src/workspaces/codez/terminalLifecycle.test.ts: 812140fc9ffd182478416adc351c70bab9f285eeac5d5d04e5bd028659b017e5
- src/workspaces/codez/Terminal.lifecycle.test.tsx: f874d3c2a454b7dda3ef88c741b008d2755129fe871648fd30cf9459b1dd29a5

The reported normal-dispose mapping leak is closed in current source: after any required successful destroy, cleanup conditionally deletes the lookup only when its logical tab still maps to its own backend ID. A thrown destroy never reaches deletion; rejected create/cleanup tail still refuses replacement and preserves uncertainty. Late-created-disposed cleanup retains the matching check as before. Idempotent repeated disposal reuses its promise; a delayed old lease cannot delete a different replacement ID. Successful tail deletion itself is also guarded by promise identity.

The additional fixture consumes real acquireTerminal/terminalBackendId, waits actual ready, observes installed physical lookup, disposes and verifies undefined. It covers the concrete successful-cleanup regression. Despite its title, that fixture alone does not test concurrent replacement ownership; that safety is supported by source matching checks and the separate existing deferred queue-ordering fixture. Test evidence is not overstated.

Terminal.tsx and component fixture hashes are unchanged from the backend-ID follow-up. The previously inspected UUID-specific output/write/resize/create/destroy routing and project-epoch close guard still apply. The prior same-ID old-output finding is addressed by distinct backend IDs per effect, with controlled listener evidence; this is not host-issued permission or native generation enforcement.

No further blocker was found for the limited renderer visibility/callback preservation, logical-ID serialization, successful lookup reclamation, backend-ID event separation and stale-project close-confirmation slice. It may be frozen at these exact four hashes and consumed as a bounded implementation candidate. Full AZ044 remains unaccepted: host/cross-renderer authority, uncertain creation/cleanup reconciliation, bounded deadlines, failed-ID retained-state bounds, terminalCount/destroy-all best-effort behavior, native PTY/OS descendants, actual StrictMode desktop journeys and rollback remain open. Historical findings and these limits must stay attached; do not mark the task or milestone complete from the ten mocked/controlled tests.


# AZ044 navigation helper failure propagation add-on review

2026-10-10. Reviewer: Codex current_source_review. Independent read-only review limited to two terminal helper changes, the added helper fixture and existing App callers. No source/ledger edits or test/native/GUI execution by reviewer. Historical final review remains valid version history, not the hash of this newer Terminal revision. Root session47333 was running at review request; no result is asserted here.

Current raw SHA256:
- src/workspaces/codez/Terminal.tsx: 87d4ac873b7c34f295a443ac6c5797d7e617425daf1f8e3001476434781f248d
- src/workspaces/codez/Terminal.lifecycle.test.tsx: 08fac00a9f1b23cb98275f3fadd2833f7549236d7761cdff012ee9259cfec4f3
- src/App.tsx: 81d99ad3755dfaff43bd74343ba2bf0cc21bf2e9df5960358745646bba78f471

confirmTerminalCloseOnProjectChange now returns false when terminalCount rejects instead of assuming safe navigation. preflightWorkspaceNavigation treats false confirmation as cancelled; App switch/close return before destructive cleanup or route changes when preflight is not ready. closeProject's wrapper also returns false immediately before its browser confirmation. Zero/nonpositive successful counts and explicitly confirmed positive counts retain their existing meaning; runtime count shape/NaN validation is not added by this change.

destroyAllTerminals now awaits terminalDestroyAll without swallowing rejection. Both App callers await it within their outer try/catch, before revokeProjectScope and setProjectDir. Thus an explicit destroy-all error reaches catch and prevents subsequent scope revocation and project route commit. switchProject returns falsy after catch, so openProjectInCodeZ does not change mode and pickFolder can reject/revoke the newly selected scope. Both callers release the navigation mutex in finally. This is rejection propagation, not a rollback: partial backend destruction preceding an error remains possible and cannot be restored by the catch.

The additional fixture invokes the actual exported helper functions against rejected count/destroyAll mocks and checks false/rejection. It verifies the concrete helper contract, not a mounted App navigation or native failure journey. No test result is inferred merely from the fixture's presence.

No blocking source issue found within these two failure-propagation changes. Once owner verifies the exact final sources, this add-on can accompany the bounded candidate freeze and supersede the previous review's count-error/destroy-all-swallowing limitation specifically. Host authority, uncertain per-tab failure reconciliation, timeouts, retained poisoned IDs, cross-renderer/OS descendants, native response truthfulness and GUI acceptance remain open. These improvements do not certify native terminal absence or atomic process cleanup.


# AZ044 exact final freeze independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only final follow-up; no source/ledger/historical evidence changes, tests or GUI/native execution by reviewer. Root reports final eleven tests session47333 passed, TypeScript2211 exit0 and lint76882 exit0 with two existing Fast Refresh warnings. These are root-owned verification results.

Exact final SHA256 recomputed:
- src/workspaces/codez/Terminal.tsx: fe130f8463b4f261c426f9065d779a18ed769ae173190fc1bf23fd49e03cb55e
- src/workspaces/codez/terminalLifecycle.ts: 60d7e6eab42c4dceea6d120c6dfc66a381b08031193cc66c846a77548f1acd53
- src/workspaces/codez/terminalLifecycle.test.ts: 812140fc9ffd182478416adc351c70bab9f285eeac5d5d04e5bd028659b017e5
- src/workspaces/codez/Terminal.lifecycle.test.tsx: 08fac00a9f1b23cb98275f3fadd2833f7549236d7761cdff012ee9259cfec4f3

Inspected final fitTerminal lint edits: the backendIdRef.current short-circuit resize expression is now an explicit if, retaining the same condition, current backend ID, dimensions and rejection handling. The callback has no terminalId reference and reads only stable refs, so removing terminalId from its dependency array does not change native ID routing. Its effect's explicit terminalId/project/activation dependencies remain responsible for session lifecycle. These edits do not invalidate the previous bounded behavioral conclusions.

The helper and helper-test hashes match AZ-044-terminal-final-review.md; the component-test hash matches AZ-044-terminal-failure-addon-review.md. These reviews and AZ-044-terminal-independent-review.md / AZ-044-terminal-followup-review.md remain preserved historical finding/fix evidence. The final bounded candidate may be frozen and registered at the four exact hashes above: visibility/callback preservation, logical-ID create/cleanup sequencing, matching backend lookup reclamation, distinct backend event routing, stale-project close confirmation and navigation helper error propagation.

This does not complete AZ044 or any milestone. Native host authorization and cross-renderer ownership, truthful backend replies, uncertain creation/cleanup reconciliation, operation deadlines, poisoned-ID retained-state bounds, OS descendant termination, real desktop/StrictMode journeys and rollback remain unverified. Partial cleanup is not rolled back by failure propagation. Controlled/mock tests must not be reported as native PTY/GUI acceptance. Any further source mutation requires refreshed verification and review fingerprints.


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
