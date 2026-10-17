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
