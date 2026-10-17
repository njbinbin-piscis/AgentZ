# AZ043 frontend independent source review

2026-10-10, reviewer ssh_audit. Read-only review of another agent’s DapClient, DebugController and DebugStore. No source edits, Cargo, TypeScript build, frontend test execution, adapter launch or GUI acceptance by this reviewer. Fixture names below are inspected source, not independently executed results. Root owns execution evidence.

Reviewed snapshot SHA256:
- dapClient.ts: 4E497D15DCB3C10C36A74841F1A2CF01D91F14F23D70C1BA376AE6FE24125878
- debugController.ts: E533C74A33CA200271A6FF37A8DD11BCFF5A80186DA2A184FB0E647A4BBE9877
- debugStore.ts: E4D581446D5D9A961AE6B65F9887CD986D7511540FFE45A066E043AA29FFFFC6

## Verified by code inspection

The client allocates a crypto UUID before listen/start, sends camel-case sessionId invocation arguments and compares snake-case event/reply session_id. Mismatched replies fail and cleanup still targets its own UUID. Unknown channels are ignored. Requests have bounded timers, send failure removes the pending entry and timer, owned exit/dispose rejects and clears all pending requests. Exit revokes before notifying the controller. Late listener registration is unlistened without starting an adapter after disposal; late start completion enters failed-connect cleanup and repeats stop only for the same UUID.

The controller captures a structured-cloned launch config and the project before cleanup awaits. Synchronous detach clears ownership, increments lifecycle epoch and pause revision, rejects initialized wait and ends the store before bounded disconnect/dispose. A stale start/error/event cannot stop its replacement. Paused stack/scopes/variables use captured session and pause revision after every await; continued and newer stopped events invalidate old results. Step rejects preserve paused state; same-tick duplicate step is blocked. Store clears frames/variables/thread on running and clears variables on every stopped projection. The initialized wait is a failing deadline rather than silent configuration continuation; configurationDone is requested only for supported capability and errors propagate. Breakpoint paths are captured and filtered to the captured root, excluding recovery namespace and dot traversal.

Inspected fixtures cover old connect/EOF, synchronous stop ownership revocation, missing initialized, configuration capability/error, second pause/continued late queries, failed/double step and captured-project breakpoints; client fixtures cover old UUID response/exit, request timeout, send failure, exit/dispose cleanup, pending listen/start disposal and mismatched start reply. This reviewer did not rerun them.

## Actionable P2: unbounded connection acquisition

Client connect directly awaits both listen and dap_start. No request timer applies until state running, so a hung listener registration or host start leaves start pending indefinitely and UI active/running until explicit Stop. Stop itself now detaches and settles within its own bounds, so this is not the original Stop deadlock. Nevertheless repeated restarts can retain unresolved connect/start operations. Recommend a bounded connect acquisition with explicit late-result handlers: late listener must be unlistened; late start success/failure must stop the same UUID after timeout; timeout must revoke and make connect settle, not just stop displaying loading. Add real deferred mocked-IPC fixtures for never-returning listen/start and their later completion. Reported to root and client author before modifying any reviewed source.

## Cross-layer guarantee boundary

The broker only cancels an already-reserved pending session. Unknown UUID stop is a no-op. Under arbitrary IPC execution reordering, an old already-issued start whose host reservation occurs after an earlier disposal stop could still reserve and replace a newer session; repeating same-UUID stop when that old invoke eventually returns cleans the old session but does not restore a replacement already killed. The current generation protects reservation/spawn completion ordering and old stream/stop isolation, not arbitrary pre-reservation command reordering. Either document a proven transport-order assumption or add a bounded explicit cancelled-ID/start handshake strategy with a deterministic reversed-dispatch fixture. Do not label this candidate immune to every IPC ordering. This is a scoped qualification, not independently reproduced native transport behavior.

## Remaining acceptance scope

No source navigation is added: DebugView still renders stack source as non-clickable text. No exception breakpoint policy/exceptionInfo flow or real Node adapter replacement is included. Project capture prevents mid-start rebinding but does not automatically stop an already-running original-project session when the GUI changes project. Breakpoint filtering is lexical and is not a host filesystem authorization/CAS claim. Native real-adapter launch/step/exception/stop and source routing acceptance remain unrun.

## Follow-up after review (not an independent test pass)

Root halted the earlier host Cargo run (exit 1, not passing evidence) to address the reported cross-layer issue. The host author then added canonical-UUID lifetime used/cancelled tombstones, including stop-before-start, with a 4096-entry fail-closed budget; no tombstone eviction or ID reuse. Remote input is validated before reservation and invalid explicit remote input cannot become local execution. A short lock now covers generation validation, synchronous spawn and installation after remote preparation, so cancelled preparation does not spawn. Host snapshot dap.rs `2ACF7BA0DCB9A8FBCE320D83D7046B89E4209ED3A0807179F8C3B5768B9DE496`; parser unchanged `35AC101AC0035F66CE7C1A0F145132A5C34298ED8DCA0ED65D8870A9C9EAC1EA`. New fixtures were authored but not run by this reviewer. This supersedes the unknown-stop ordering gap in the earlier host snapshot; root still owns test execution and independent host review. The client author is separately adding bounded connection acquisition and deferred-result cleanup; its updated snapshot has not yet been independently reviewed here.
