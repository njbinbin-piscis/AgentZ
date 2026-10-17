# AZ038 artifact domain independent review

2026-10-10. Read-only review of initial frozen artifacts.rs SHA256 73e6f54782e58ab74e5c324d027aadfed38dd489872cef32284e9756b115dcf5. No Cargo/product/ledger edits or GUI execution by reviewer.

## Required fix before accurate mutation success

capture INSERT and delete UPDATE check changedrows==1, then commit without verifying post-trigger storage. SQLite AFTER INSERT can DELETE newly inserted row while changes reports1; capture returns Created for missing artifact. AFTER UPDATE can reset deleted/payload to prior content while UPDATE reports1; delete returns Deleted for still-active bytes. Existing ABORT fixture proves rollback but not this false-success path. Reported to root and author. Require same-transaction postwrite actual identity/payload/tombstone verification (and aggregate budget), or explicitly reject artifact-table triggers. Add actual AFTERtrigger fixtures; do not equate affectedrowcount with immutable persisted result.

## Reviewed positive boundaries

Tombstone retains metadata/key/version and empty BLOB, prevents same version reuse after delete, distinguishes AlreadyDeleted/NotFound, and next_version includes deleted identities. Historical snapshot metadata remains original binding version; same-task/environment read can retrieve it without current execution permission. Physical live source is never opened/deleted by domain store. Scope1000row budget includes tombstones, intentionally not released version identities; payload32MiB aggregate and perrow1MiB bounds validate before allocations. read validates actual payload digest and metadata identity.

Namespace strict table type, names/columns/types/NOTNULL/PK and singletonversion validation rejects views/partial/future metadata. Existing readonly missing returns absent without init/migration; explicit initialize alone creates namespace. Immediate writes serialize concurrent mutable state. Paginated lexicographic artifact/version reads validate entire bounded scope hashes before returning page, so a corrupt unselected row causes error not a misleading clean page. Cursor is advisory within suppliedscope and is not an authority token or frozen cross-page snapshot; concurrent deletions/captures can alter subsequent pages. No total historical snapshot consistency claimed.

Standalone binding validation is syntax/canonicalproject only: trusted host fresh task/environment/scope authorization is a mandatory caller contract. No Run ownership/grantedcapability should be inferred from validhash. Tombstone metadata bytes remain original even payload is cleared; this is provenance, not current payload size. Five fixtures cover useful core behavior but have not run by reviewer. Actual native file capture, host API/code+noncodeintegration, GUI, SQLite online backup/WAL/recovery and semantic ChangeSet application remain unverified. Domain result is not a full AZ038 Gate pass.


During review author added explicit namespace trigger rejection at layout entry. Live SHA observed b435eb73fe1bdbc1763e340cd951aae7ee4b8ee9cd795a7d3f00e046cfaeb052. This addresses the reported mutation false-success mechanism by preventing trigger execution before any operation. Final freeze and meaningful trigger fixtures still require owner confirmation and root-run verification; reviewer will append final proof rather than overwriting historical finding.


## Final frozen trigger repair review

Final artifacts.rs SHA256 93f0429e93b0e6f4af631772182781c4b0af271ff64a5315ea0727ac2b473e0d. layout rejects every trigger bound to artifact_schema or artifact_versions before open/init/read/mutations proceed. SQLite Immediate transaction prevents a concurrent schema write from inserting a trigger between this validation and mutation. New fixture installs actual AFTER INSERT erasure and AFTER UPDATE resurrection triggers, verifies capture/delete reject with DBbytes unchanged, validates readonly/init rejection, removes trigger and confirms originalpayload intact. This covers false-success behavior rather than only ABORT failure. Six fixtures await root-run verification; no outcome claimed here. Parent reports previous pre-fix5 passed, which is not final6 proof.

Tombstone immutableidentity issue and triggerfalse-success findings are closed by source inspection. No blocking defect remains in the limited domain slice reviewed, with hostauthority/nativecapture/GUI/backup restrictions above still applicable.
