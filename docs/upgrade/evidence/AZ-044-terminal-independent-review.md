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
