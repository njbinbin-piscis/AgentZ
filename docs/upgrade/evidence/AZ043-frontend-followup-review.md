# AZ043 frontend deadline follow-up review

2026-10-10. Independent read-only inspection of the client author’s final connection-deadline changes. No source edits, tests, Cargo, TypeScript build, adapter launch or GUI were run by this reviewer. Root owns the running host/frontend/TypeScript checks and their eventual results.

Full reviewed SHA256:
- `src/extensions/debug/dapClient.ts`: ACB362B4DDBEA5CF7EB8AF2707463AE0897AE4109E7DA896CB4F6B0FC904D509
- `src/extensions/debug/dapClient.test.ts`: F5DD7872DC2C8BEF200DAEF4447522A38E9C505F6897DEB25128E4C8C403E7D3
- Unchanged controller: E533C74A33CA200271A6FF37A8DD11BCFF5A80186DA2A184FB0E647A4BBE9877
- Unchanged store: E4D581446D5D9A961AE6B65F9887CD986D7511540FFE45A066E043AA29FFFFC6
- Host used for contract comparison: 2ACF7BA0DCB9A8FBCE320D83D7046B89E4209ED3A0807179F8C3B5768B9DE496 (reviewer authored host; this is not independent host acceptance).

## Result

No new blocking defect found in the final client deadline changes. The earlier unbounded acquisition finding is addressed at the JavaScript operation boundary. Listener registration and host start each race a validated positive deadline (default 10 seconds per phase, not one combined 10-second connection deadline). Phase timers clear in finally on success, failure or timeout. Connection failure revokes local state/listener/pending request ownership and performs same-UUID bounded stop before rejecting. Cleanup can add the configured stop timeout to the phase deadline; no promise of instantaneous timeout or immediate OS reaping is made.

The real IPC promises retain explicit late handlers instead of being abandoned by Promise.race. Listener timeout does not assign the listener to the client: its late successful registration calls its returned unlisten once, and no dap_start is subsequently sent. Successful registration followed by disposal calls the local returned unlisten before throwing, while registered owned listeners are cleared by revoke. Start timeout revokes and stops its captured UUID immediately; late success repeats stop with that same UUID, never with the returned potentially foreign identity. Late rejection is consumed. Terminal clients reject future requests without sending. Repeated revoke finds no listener or pending entries, so previously owned cleanup is not repeated via those paths.

Stop-before-start now closes across the current broker contract: the first cleanup stop records canonical UUID cancellation even if reservation has not arrived. The late host reservation is refused; started UUIDs cannot be reused. When the 4096-marker budget is full, all new reservations fail closed without evicting cancellation markers. A late cleanup for old UUID cannot stop a newer UUID. This addresses the specific pre-reservation dispatch ordering finding from the earlier review; it does not claim cancellation of already-completed debuggee side effects.

The prior controller/store review remains applicable because their hashes are unchanged: synchronous detach, captured session/epoch and pause guards, negotiated configurationDone and failing initialized deadline are retained.

## Fixture inspection and remaining evidence

The two added fixtures use actual DapClient with deferred mocked Tauri IPC and fake timers. They inspect never-answering listen/start deadline rejection, late listener disposal, late start same-UUID stop, no start after listener expiry, timer cleanup and terminal request rejection. Existing seven fixtures still cover UUID filtering, send failure, exit, disposal during listen/start, bounded stop and mismatched reply. This review inspected these tests without executing them; do not use this document as a test-pass receipt.

A deadline settles the client-facing operation but cannot cancel an unresolved native IPC Promise itself; the retained continuation intentionally exists until it completes. Bounded broker stop, tombstones and conditional spawn protect current ownership on eventual dispatch. No native adapter/GUI walkthrough, process-tree reaping guarantee, automatic project-switch stop, source-navigation or exception-policy acceptance is added here.
