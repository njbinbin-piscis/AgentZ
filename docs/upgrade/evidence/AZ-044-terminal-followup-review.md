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
