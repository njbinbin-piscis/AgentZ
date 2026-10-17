# AZ-025 independent external action domain review

2026-10-09; read-only source/contract review. No Cargo, provider, IPC, desktop or external side effect executed by this reviewer. Source SHA-256 713f68946790c6f8653c8f4359a5f44e46d05391daf21a9989f9c546a037400b.

## Limited conclusion

No blocking correctness defect identified in the reviewed standalone domain slice. This is a local single-permit intent ledger, not authorization for real send/deploy nor distributed exactly-once execution.

Existing-only open uses READ_WRITE/READ_ONLY without CREATE and validates Run schema plus independent namespace in a transaction. Missing external namespace is query-missing, never automatically repaired; explicit initialization uses Immediate and rejects partial layout/version metadata. Column name/type/NOTNULL/composite PK and exactly singleton1/version1 are validated. Layout is not a cryptographic tamper proof; arbitrary privileged DB edits remain outside this contract.

prepare validates trusted project/task/origin and all task history before insertion. Duplicate immutable scope returns Existing without a permit; conflicting digests/provider/kind reject. Immediate start validates stored record, exact revision/Prepared state, same environment, authoritative captured origin and current Running attempt. CAS/save and commit precede issuance of private-field non-Clone/non-serde StartPermit. Cross-connection writers serialize; restart/duplicate cannot recreate a permit from Started/Unknown/terminal records. A preparation from an earlier attempt does not authorize it to start after retry.

complete/mark_unknown consume permit; a failed SQL transaction leaves Started unresolved and no replacement permit. Reconcile takes trusted evidence only, never returns a dispatch capability. Ordinary transport errors cannot produce NoEffect implicitly; Started/Unknown remain blocked until explicit evidence, terminal records have a valid digest. Record deserialization bounds/identity/revision/timestamps/receipt-state plus authoritative origin are checked before transitions. Save requires exactly one changed row; overflow/digest/time errors return before commit. Read-only missing/future/corrupt cases do not repair.

Eight meaningful fixtures are present, including real two-connection contention and SQL ABORT rollback, historical-attempt rejection, immutable conflict, lost-result controlled service/reopen and readonly bytes. They have not been run by this reviewer. Parent owns actual compilation/testing and module export.

## Export-only and acceptance limits

Adding pub mod external_action to lib.rs exposes an independent opt-in module; reviewed source has no global initialization, existing RunStore transitions/event/approval hooks changes or host registration. Therefore export-only does not alter AZ017/018/019/020 execution semantics. It does change their whole-lib review fingerprint: parent should independently refresh affected manifests rather than pretend the old hash is current.

StartPermit proves a local start transaction, not caller honesty or provider idempotency. The controlled service fixture is an in-process counter and cannot attest HTTP delivery/reconciliation. Callers must retain task/environment authority and ensure the outgoing request matches the immutable target/payload digests; no host adapter currently enforces that. Reconciliation evidence is caller-trusted, not verified by this crate. Lifecycle event append, startup owner proof, renderer query authority, provider integration and GUI remain deferred. No automatic retry/reset API exists.

Useful additional boundary fixtures before broader host consumption: revision i64::MAX start rollback; empty/schema0/missing-PK namespace fixtures with bytes unchanged; terminal Run with late trusted reconciliation; different project permit rejection; duplicate completion/reconciliation conflict. Existing source guards these paths by inspection, but missing fixtures are not claimed passed. No requirement to perform a real external action for this limited domain candidate review.


## Additional schema/identity/adapter review

Ordinary same-name views report PK0 in PRAGMA table_info and fail expected PK1/2; replacing composite PRIMARY KEY with plain UNIQUE also reports PK0 and is rejected. SQLite enforces an actual composite PK for the accepted normal table. Current layout counts sqlite_master names but does not explicitly require type=table; recommend explicit type check plus fixture for a clearer fail-closed contract rather than presenting column inspection as verification of every storage property. Constraint SQL/collation are not fully fingerprinted; this is not tamper proof.

RunStore validate_budget_origin validates Binding syntax and authoritative task/environment/run/historical Attempt ownership, not current binding_version/capability_version: those versions are not persisted in Run. start adds current Attempt Running CAS and same environment. Thus trusted host must freshly validate environment/capability versions and permission before dispatch; ledger does not itself grant OS/provider authority. A same-task same-environment fresh Run may adopt a previously Prepared action under the current contract, without resetting a Started action.

StartPermit has private fields and no read-only accessor. That preserves non-forgeability but prevents a future standalone adapter from deriving target/payload/provider/identity directly from the permit. Future integration should provide a minimal immutable accessor or enforce retained scope digests before dispatch, not serialize/reconstruct the permit. This is an adapter readiness gap, not a duplicate-start bug in the current core.

AZ026 is titled fixed kernel contract version integration, depending on AZ022/AZ023; it is not the diagnostic-export task. The smallest next independent preparation is a versioned overlay/source manifest and compatibility record against the immutable upstream archive, actual apply/reverse chain and frozen source/vendor targets, followed by root-only integration checks. It must not call current partial-overlay patches a full vendor reconstruction, rely on sibling uncommitted overrides, or perform public release without authorization. Diagnostic export can separately reuse RunHistory/TaskDetails readonly scope and bounded structured recovery records, avoiding raw prompt/log/credentials; it is outside AZ026 scope and should not be silently added.
