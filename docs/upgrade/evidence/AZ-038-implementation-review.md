# AZ038 consolidated bounded independent review

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


# AZ038 export-only independent review

2026-10-10. Current lib.rs SHA256 be80ac835872c052676752b35c6927265094a415c4921080d90c8511a57ab46d.

The new pub mod artifacts declaration is an opt-in domain export only. No initializer/global/static execution, existing environment/Run/Event/Plan/approval/recovery/details behavior, command registration or startup flow is added/altered in lib.rs. Artifacts namespace creation happens only when its explicit API is called. Existing consumers therefore retain semantic behavior while whole-lib fingerprint changes; parent must refresh prior review pins with history preserved. Subsequent artifacts implementation changes require their own independent review without changing this narrow export conclusion. No Cargo/product/ledger mutation or GUI验收 by this reviewer.


Additional narrow helper extraction: task_view_read.rs read_captured_text is crate-private and sequences existing relative(false) validation -> checked(root,rel) containment -> read_text same-handle checks. Original command retains early input validation, environment lease/fresh observation/expectedbinding and before-after scope checks; replacing its local checked/read sequence with helper is equivalent and adds no renderer authority. Future caller must supply freshly captured root under its own hostscope/lease; helper is not authorization by itself. Actual task_view_read.rs SHA256 8a38d539c9c3e3db9924169e158d348b84cd1fe6c3d03bdbf90f547489e1657a. No extra verification run.


# AZ038 scoped artifact host independent review

2026-10-10. Read-only source/registration review. Parent reports final domain6 fixtures90757 pass and domain strictClippy62880 exit0; reviewer did not rerun Cargo. Host end-to-end/GUI acceptance not inferred from those domain results.

No blocking defect found in the limited inspected adapter. Common authorized wrapper holds environment execution lease, looks up main-window opaque project scope, resolves supported existing task observation, compares full expected current PlanBinding, then executes under with_verified_scope after project identity/observation recheck, and rechecks observation afterward. Caller sends no filesystem root/payload bytes. Current authority is validated even when reading old stored binding metadata; response scope is current while metadata is historical provenance, not permission.

capture obtains bytes through same-handle, hardlink/reparse/size/UTF8-protected read_captured_text using trusted root. It rechecks observation before explicit store initialization/capture. Host constructs artifact identity by relativepath digest, version from advisory next_version and inputmetadata; renderer can supply title/kind but not bytes/digest/version. Competing captures may conflict rather than overwrite; host retry creates a new version, so this is immutable snapshots, not request-id deduplicated capture. Successful Capture returns storedmetadata only after commit.

list/read open existing readonly without artifact initialization; missing store is explicitstore_missing/empty page and missing readNone. Corrupt/incompatible storage errors propagate, not empty success. Old payload read verifies domain hash and strict UTF8 then returns passive content. No HTML execution or source path fallback. Current environment/task scope determines visibility; old different environment is not automatically included. Pagination is live bounded page, not whole-history atomic snapshot.

delete opens existing RW without CREATE, forwards accurate Deleted/AlreadyDeleted/NotFound after domain commit, and propagates DB errors. It removes captured payload/tombstones only, never live source. Missing storeNotFound and repeatAlreadyDeleted are distinguished. Post-operation observation failure can occur after successful mutation and return Err; this cannot roll back a committed capture/delete. UI must reconcile rather than claim rollback or blindly repeat capture. Held cooperative leases/scope lock narrow normal races but do not provide adversarial DB/filesystem atomicity.

mod.rs adds only pub mod task_artifacts; generate_handler adds four new scoped commands. Existing AZ017/AZ035 contracts and initialization are unchanged by registration, but shared hashes require review-history/pin refresh. New commands intentionally expose snapshot capture/delete, not write/process/tool/renderer-root permission. Existing resolver environment registry load may initialize/migrate as previously recorded, so whole host call is not globally readonly merely because artifactread uses readonlystore.

Current host test covers path-derived ID stability/case distinction and explicit delete serde only; it is not native authorized IPC/capture integration. Case-sensitive ID on Windows may create two snapshots for differently spelled path names; payload safety remains native read boundary. CodeChangeSet kind is passive text snapshot, not staged/applicable Git delta. Native scoped read tests/domainfixtures support limited candidate but full oldbinding/capture/delete UI scenarios need evidence. No GUI/fullAZ038 Gate claim.

Fingerprints:
- src-tauri/src/commands/task_artifacts.rs: d81926b188560e2fc1fd52a0014b21e8770ac92270eb377dbf960039a7c29edf
- src-tauri/src/commands/mod.rs: e3bdaa6d8a1f793d6c64c3c9a9326c5000a75f442d1378a062b519165829ad33
- src-tauri/src/lib.rs: 2516371b7cbeff732ca7747e90f00eee7addcc62848a769fbdca139b91dd3c81


# AZ038 controlled native-read/domain integration fixture

2026-10-10. Added Windows-only actual-file fixture to task_artifacts.rs tests, leaving production command behavior unchanged. Final module SHA256 9765a32726901bacec2d88ff4d35165a708c3e94ce98956f8c46e7da9a96e56b. Only module and this dedicated evidence edited; no Cargo/IPC/model/GUI invocation.

Fixture writes actual sample.rs code and report.md text in a unique temporary directory, calls production read_captured_text with its real Windows same-handle/link/UTF8/size guards, then captures those bytes into real SQLite ArtifactStore. It mutates each live file, reopens existing store and verifies old payload plus original bindingmetadata remain exact despite a new binding revision. Wrongtask/environment cannot read/delete another scope; wrongproject rejects. Deletion yields Deleted then AlreadyDeleted, historical readNone, retainedversion next2, while changed live source remains untouched. RAII removes only the unique fixture directory after store closes.

This is a controlled native-read -> real domain storage integration fixture for code/noncode, not a mocked permissions claim and not a Tauri authorized IPC scenario. It does not instantiate opaque scope/window/environment lease authorization; domain supplied binding is intentionally controlled. ChangeSet content is a text snapshot, not Gitapply. Existing production-native read containment fixtures cover separate faults. Root owns unified compilation/run, including current38483 earlier build requiring a rerun for this new fixture. rustfmt on this one module completed exit0; new fixture has not run by author.


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


# AZ038 WorkZ artifact entry independent review

2026-10-10. Read-only review of new TaskArtifactViewer import, artifactViewKey state, openTask clearing and JSX entry. Current WorkZ index.tsx SHA256 7de0137cdb854c77d309e57d68932a4cec50c0d639d7f6c4fd1f3b56f623e1ec. No GUI/test/Cargo or product/ledger edit.

No blocking defect found in this limited mount wiring. Artifact button/child live only for nonteam/nonpool, existing planProject whose selected project matches current projectDir and existing sessionRef. Key binds canonical authority project and session; mismatch unmounts old viewer instead of displaying another task's artifacts. Button merely selects view key, never changes project/session or submits a turn. openTask clears both fileViewKey and artifactViewKey immediately before async new-task fetch; same current liveforeground shortcut returns before clear and preserves its active view/stream. Viewer scopehook adds opaquehandle identity checks internally; mount does not treat canonicalpath as native permission.

Shared pin refresh for AZ017/AZ035 should retain oldreview history and this current hash. This review concerns only WorkZ entry/mount, not the author's ongoing artifact UI operation-lock/reconcile fixes; those must independently pass before candidate freeze. Case/domain/read/delete semantics and native authorization remain their own reviews. GUI pane layout, close behavior and full task artifact acceptance have not run by this reviewer.


# AZ038 terminal-page contract independent review

2026-10-10. Read-only final source review. No Cargo, Vitest, typecheck or GUI executed by this reviewer. Earlier domain/client reviews remain unchanged historical evidence.

The producer and consumer now agree: next_cursor is the last entry actually returned by this page, or null when this page is empty. It is not the supplied after cursor. ArtifactStore::list_versions retains exclusive lexicographic after filtering, ordered (artifact_id, version), limit+1 has_more detection, scoped hash validation and bounded page limits. At artifacts.rs:710 the former fallback to after is removed. An empty terminal page after another action deleted the remaining version is consequently valid and cannot fabricate continuation.

The client keeps previous=capturedAfter for strict ordering of returned entries, but compares next_cursor against this page's independently computed last entry. Empty versions therefore require null next_cursor; has_more still requires a full requested page, preventing an empty continuing page. Nonempty pages must return their exact final cursor. This change neither grants scope access nor weakens current binding/historical metadata validation.

The new real SQLite fixture at artifacts.rs:249 captures a version, obtains its cursor, tombstones it, then requests after that cursor and asserts empty versions/null cursor/has_more=false. The client fixture accepts the same terminal shape after a non-null input cursor and rejects a response echoing that input as a fabricated continuation. Pagination remains a sequence of individual transactions, not a stable multi-page snapshot: concurrent capture/delete can affect subsequent pages. Manual refresh is the explicit reconciliation mechanism; no claim of an immutable entire listing is made.

No blocking contract mismatch remains in this bounded change. Root reports final frontend thirteen tests passed (28610); domain seven fixtures are running (84762), TypeScript is pending (7312). These are attributed root results, not independent reruns, and pending checks are not marked passed. No desktop/GUI or full milestone acceptance follows from this review.

Final SHA256:
- crates/agentz-task-state/src/artifacts.rs: 794d2e055ae53378fa7b0d3575d24c45f6e0fb2fe17eeb8f4afbb3df009b69eb
- src/services/tauri/taskArtifacts.ts: 1320d9063caf20ce0a4cb5a982b26e4ceaac70e74a8e6feb15844ff52fb2ea7d
- src/services/tauri/taskArtifacts.test.ts: 053f45e8252fb9f0c036d3948249ad67b64c020bc6b053222331cb3bf716dc2a
