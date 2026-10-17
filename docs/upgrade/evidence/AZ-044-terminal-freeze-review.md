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
