# AZ038 artifact service/UI independent review

2026-10-10. Read-only candidate review; no tests/Cargo/API/GUI run. Findings reported to root/author; final fix review pending.

## Must fix UI operation boundaries

action guards only React visible.busy. setState does not synchronously lock same-render event handlers, so two immediate callbacks can concurrently create multiple versions or delete/reload, with same generation and competing response writes. Add synchronous in-flight ownership/operation identity and safe cleanup across generation changes, plus immediate-doubleinvoke deferred fixture. Disable alone is not a native permission guarantee.

catch shows “refresh before retrying” but immediately reenables capture/delete and retains oldlisting. A server commit followed by postverify/transport/clientvalidation error is unknown result, not rollback; direct recapture makes a new version. Require reconciliation state that blocks mutation until explicit successful fresh observe+list; do not automatically retry capture. A refresh must obtain new task binding, not simply reuse stale view. Add committed-but-response-error fixture proving no direct replay and manual refresh unlock only after success.

## Positive service boundaries

Service strict ownshape/enums/safe integers/size/digest schema distinguishes current response scope from historical metadata binding_version while retaining sameproject/task/environment. read verifies requested ID/version, UTF8 raw bytes, null rejection rules and SHA256 via WebCrypto before publication. capture verifies returned kind/title/path and host path-derived ID, and requires current metadata binding. Cursor/order/duplicate checks reject substituted pages; pageabsence/storemissing remain explicit. Only opaque scope/session/expectedbinding and relativepath inputs invoke host, not renderer payload/root. ID clientASCII restriction fits current host path.sha256 IDs; broader domain unicode IDs would need intentional future service contract.

Viewer requestKey project/session/handle masks oldstate before effect; generation cleanup rejects other-task responses. Text/pre presentation does not execute webpage HTML/scripts and ChangeSet label limitation is explicit. Pendingdelete confirmation names storedversion/livefile distinction. Current error wording does not fake success, but cannot substitute an enforced reconciliation barrier. UI scopehook remains hostscope authority observer, not permission issuer.

This review does not cover WorkZ mount integration, native scoped authorization fixtures, full GUI or milestone acceptance. Final fingerprints (pre-fix) retained for history:
- src/services/tauri/taskArtifacts.ts: 8d616f3fe5879a8a9a1ce8cfc1a7253cc7687d42d420770173f81fd2fce8cc53
- src/workspaces/codez/TaskArtifactViewer.tsx: 0cc129a6254ed98af4bf67cf4991f228a57ab7da9f989de8aef19cbf623f07e5

## Final source follow-up

Both reported UI defects are closed in the reviewed source. action now acquires a synchronous inFlight epoch before its first await; same-tick capture/delete callbacks cannot issue a second mutation. Scope/session effect resets ownership and advances generation/epoch, while an old finally only releases its own epoch. Response publication still checks generation. The six component fixtures include deferred immediate-double-capture and immediate-double-delete, plus late content after task switch.

Mutation failure now marks both a synchronous reconciliation ref and rendered reconciliation key. Capture/delete entry guards and their buttons remain blocked until explicit Refresh versions successfully obtains a fresh observation AND lists that scope. Failed fresh observation leaves the barrier in place. Neither read nor pagination clears it; successful refresh does not replay the failed mutation. This is conservative even when the failure happened before commit, and does not claim rollback or exactly-once transport. The deferred unknown-commit fixture verifies retained old callbacks cannot recapture and an unsuccessful refresh does not unlock. Task switch clears this per-view barrier; native store/authorization remains the independent enforcement boundary.

Service tests now use fixed independently specified SHA256 vectors, not a mocked digest implementation or node:crypto dependency; production continues real global WebCrypto validation. Root is running the unified twelve fixtures/typecheck; this reviewer has not run them and does not record their pending result as passed. No additional blocking source defect was found within these two fixes. Full desktop GUI, cross-platform and complete Git/artifact delivery remain unverified.

Final SHA256:
- src/services/tauri/taskArtifacts.ts: 8d616f3fe5879a8a9a1ce8cfc1a7253cc7687d42d420770173f81fd2fce8cc53
- src/services/tauri/taskArtifacts.test.ts: 4100725f5b813d6cf4fa2f3617fd0598f2a38e8879227835cdfdd7850f565572
- src/workspaces/codez/TaskArtifactViewer.tsx: aae9cbe061a5b0bb9945fff0ca5b451ea2720c9f9c911579d8d3965b432e9044
- src/workspaces/codez/TaskArtifactViewer.test.tsx: 52455ec7c7e6867e9566b64dbf92e63b24b4647c7acc9a3f98ad3833ad23967d
