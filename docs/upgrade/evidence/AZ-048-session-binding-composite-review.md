# AZ048 durable session binding candidate

Domain86191 actual6 tests passed,0failed,141filtered,17.50s build/0.31s tests. Client86919 actual11tests passed3.36s, full TypeScript and scoped ESLint exited0. Host76168 old revision passed1m37s, independently found overview provisioning fixed afterwards; current16688 check pending, prior success not extended to fix. No chatrow/model execution/UI/backup/rollback/nativeinvoke/fulljourney acceptance.

## Preserved review docs/upgrade/evidence/AZ-048-client-composite-review.md

# AZ048 directory client candidate

Host check47282 actual exit0 in6m24s. Domain49746 actual4 passed. Client16951 actual8 tests passed in3.00s and full TypeScript exit0. These results do not cover UI/session integration, native invocation, archive/restore, model, rollback or platform acceptance. Preserved old review scopes and limitations below.

## Preserved review docs/upgrade/evidence/AZ-048-host-composite-review.md

# AZ048 bounded host directory candidate

Preserves all earlier consumer scopes and review history. New scope is only main-window fixed Tauri AppData create/reopen and two additive command registrations; no session/UI/archive, native invocation, model or release acceptance.

Actual domain session49746: offline locked 4 passed, 0 failed, 141 filtered; 21.09s compile, 0.20s tests. Earlier99869 3 passed. Current desktop check47282 pending; no success inferred.

## Preserved review: docs/upgrade/evidence/AZ-048-shared-export-review.md

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


# AZ040 summary/Advanced independent implementation review

2026-10-10. Read-only review of InputContextSummary and the added CodeZ/WorkZ JSX integration. Existing unrelated repository diff was not attributed to this slice. No independent tests/build/models/GUI run.

The shared summary is presentational: props, translated strings and native details only; no IPC, tool execution, permission response, state mutation or scope minting. Unknown/empty strings remain Unknown, optional counts reject negative/unsafe values. Policy explicitly says configured policy does not grant permission. Workspace is explicitly configured context rather than host-verified environment. Queued text explicitly says “Queued locally”/“本地待发”; it is not persisted Run state. The approval display is a typed possibility but neither current caller supplies it, avoiding inference from stale permissionRequest.

CodeZ adds the summary immediately above its existing composer. Authority and workspace are the configured projectDir. Model uses current selected ID or loaded configured default label; the ID is a selection identifier, not a provider-confirmed effective model. Settings-loading failure falls back to Unknown. Busy displays Running, then existing error, then local queuedView presence, otherwise Idle. Busy takes precedence over pending local messages, so this slice is not an exhaustive queue dashboard. Existing Send/Stop and composer fields remain unchanged by the summary. CodeZ optional skills/connectors count is not yet supplied; their existing controls remain visible and functional, so no hidden-option behavior is introduced by this slice.

WorkZ only displays worktree_path when worktree.id matches sessionRef.current; otherwise the workspace fallback is configured free cwd/direct project, or Unknown for unverified isolated/agent/team contexts. Hidden model selector forces model/source Unknown and does not display the remembered scoped ID or global default. Mode labels separate agent/team/plain; policy remains configuration. Summary counts existing selected optional skills/connectors only when generic controls apply. No stale permission card is converted to waiting state, and no local queue is fabricated for WorkZ.

Native Advanced details keeps its children mounted while collapsed. Team/persona selectors retain original values, callbacks and taskBound/busy disabling. Free cwd and isolation remain outside the collapsed controls. The new start-source/base controls are scoped to isolate && !isFree && !taskBound and busy-disabled, feeding already existing startMode/startBase state consumed by task creation. The default remains working; no automatic switching or selection clearing was added. This adds access to existing configurable inputs rather than changing the send algorithm. Keyboard/focus, overlay placement within collapsed details, layout clipping and native desktop interaction still require GUI acceptance.

No blocking source defect was found in this bounded descriptive-summary/Advanced slice. It does not close budget presentation, permission receipt ownership, verified Run/Attempt live state, five-user first-use evaluation or full AZ040 acceptance. Selected model IDs are intentionally descriptive configured identifiers; future friendly-label resolution must preserve Unknown for missing provider definitions.

Reviewed SHA256:
- src/components/chat/InputContextSummary.tsx: 10ab6a5753f3874655cb2c9636c4b146afc0d8b63f68b3e024ab0f282e1c5847
- src/workspaces/codez/AssistantPanel.tsx: 7c82b46a98d816b34f4ab0ffedbff22f48fa6eaee0010cd9bfcaae25d82a7598
- src/workspaces/workz/index.tsx: e11099d392fe3184eb539db9b2c8b612b1b52af18ac12262c68a9c90ba51f468


# AZ040 observed-budget presentation independent review

2026-10-10. Read-only source/fixture review; no tests, Cargo, models or GUI executed by reviewer. Root reports author BudgetObservation seven and TaskDetailsPanel five targeted tests passed; final combined validation remains root-owned.

BudgetObservation renders totals only for an available typed source. Missing/unknown/stale/unavailable/unsupported sources have explicit status text and no numeric totals. Available known zero is preserved as an observation, not substituted for absent values; null policy maxima use Not configured via nullish checks. Known input/output, known active duration and usage count are distinct from unknown token/duration/pricing record counts. The fixtures cover each unavailable source, mixed known/unknown records, real known zero and absent cost observations.

Costs remain separate currency buckets in micro-units; no conversion, invoice claim, subtraction from maxima, remaining balance or token-to-price estimate is introduced. Empty cost map says No known cost observations. Configured maxima and observed exceedances are labelled observational; the component explicitly says enforcement is not connected, known totals may be incomplete and maxima are not an enforced allowance. It does not call the reservation API or authorize execution. Data validity is supplied by the existing strict taskDetails DTO validator; this component is not a replacement trust boundary.

TaskDetailsPanel change is an import plus replacement of its old budget paragraph with BudgetObservation using data.sources.budget. Existing scope/project/session request key masks old data during render before effect; generation/disposed checks suppress late responses, focus coalescing and error clearing remain intact. Therefore no old budget is exposed through the new child on a task/scope switch. Sources-read-separately/non-atomic disclaimer remains displayed. No polling or model continuation was added.

No blocking defect found within this observational presentation slice. Host accounting completeness, prices/invoices, reservation enforcement, team/member coverage and GUI usability remain unverified and are not inferred from these fixtures.

Reviewed SHA256:
- src/workspaces/codez/BudgetObservation.tsx: 567d6defee4616872f05a7995420e52d0cafdddc0b0a8ec24db8ad001878fa7c
- src/workspaces/codez/BudgetObservation.test.tsx: db01fad4a7d937dd0009db77a80a4499c501030b5112616e70c78ea20abcf22f
- src/workspaces/codez/TaskDetailsPanel.tsx: 8b925d75fb02697050ad3bc910999dfe5348cdf88a209bf3ee7b0f1bd9ad1a6d


# AZ039 bounded delivery observations independent implementation review

2026-10-10. Read-only review of four new summary/wrapper files. No tests/typecheck/build/GUI/model/external operations run by reviewer. WorkZ root mounting has not yet been included in this review. Author reports previous five-summary/four-wrapper fixtures passed; subsequent delivery fix requires root final validation rather than reuse of old results.

Found and resolved during review: initial summary ignored run.delivery and labelled all Acceptance as no receipt. Final source now separately displays authoritative Run delivery accepted/rejected/awaiting_review/not_ready, with added accepted/rejected fixture, while stating that a Run decision does not establish acceptance of a specific artifact version. Artifact-version acceptance, apply and publication still explicitly lack receipts. Full execution enums are shown independently; succeeded never implies validation pass or publication.

Artifacts are counted as distinct IDs and retained versions on this returned page only, with more-available marker. Validation kind counts recorded attachments and explicitly does not establish passed validation/acceptance. Missing store, unavailable/unknown/stale source remain labelled rather than fake zero-success; known empty available page can correctly count zero. No payload/tool text is interpreted as a verdict. Buttons only invoke view callbacks; no accept/apply/PR/merge/model action is introduced. Typed props rely on strict existing DTO services for runtime enum/scope validation, not untrusted raw JSON rendered directly.

DeliveryReviewPanel uses opaque useProjectScope handle, scoped readTaskDetails and fresh observeTaskView before bounded listTaskArtifacts(null,200). Separate sources use allSettled: one failure preserves another valid source but marks failing evidence unavailable rather than empty/pass. Independent project/task/environment mismatch marks artifacts stale. It does not claim binding revisions or separately read sources are atomic. No continuous polling or cursor synthesis occurs.

Render key project/session/handle masks old data before effect; generation cleanup discards late replies and suppresses list invocation when observation outlives its generation. Source loading resets previous values; scope missing prevents reads. Fixtures exercise bounded fresh observations, failed list without leaking private error, late prior-task responses, cross-environment stale and no-scope. Run details may be available while artifacts fail, which is intentionally distinct rather than global successful delivery. Manual refresh remains observational and no active chat projection is rewritten.

No blocking defect remains within this bounded summary/wrapper contract. Exact-version acceptance receipts, complete multi-page artifact enumeration, stable cross-source snapshot, old openPr safety, native GUI and full AZ039 acceptance remain outstanding. This candidate may expose existing review/artifact viewers but does not itself implement their authorization or external-delivery semantics.

Reviewed SHA256:
- src/workspaces/codez/DeliverySummary.tsx: 504f8776ef6ccc20c18b7d6dd84c0f263f299452336cafbb236d1c05b6cbb4df
- src/workspaces/codez/DeliverySummary.test.tsx: f06f3adec81df0e7e2eb851b1b0420752365c10084e3252db59ca0b8f3b6776c
- src/workspaces/codez/DeliveryReviewPanel.tsx: 237c6d2c2310fda3379729fc29acbbbf3f401b2294cceb5d37949ab6c8681458
- src/workspaces/codez/DeliveryReviewPanel.test.tsx: cd5d1f9a732a9e10909e1c6c33ceb123d81f4ce8a67d530be59414e3040b87f0


# AZ039 two-workspace wiring independent review

2026-10-10. Read-only inspection of root integration; no tests/typecheck/GUI/model or Git operations executed. The four new component files are covered separately by AZ-039-implementation-review.md, left unchanged.

CodeZ mounts DeliveryReviewPanel only with a current project and session, using those inputs for the scope-aware observer. Open artifacts sets a project/session identity key; TaskArtifactViewer mounts only when that key matches the current pair and is React-keyed by it. Root added key clearing on project reset, newSession and switchSession start, preventing an existing artifact preview surviving asynchronous selection of another session. No review/merge/PR callback is attached to CodeZ; its existing composer/stream handling is otherwise unchanged by this addition.

WorkZ reuses the existing non-team/non-pool, selected-project/canonical-project/current-session gate. It passes canonical authority to DeliveryReviewPanel. Review changes is present only when not busy and worktree.id matches sessionRef.current, and only opens the existing review UI; no merge/discard/PR runs automatically. Open artifacts uses the same canonical-project/session key as the existing viewer. Old preview clearing at openTask async start remains present. Changing the project/session prevents mismatched-key viewer mounting; child scope generation validation remains the actual asynchronous-response isolation. Neither button issues native permission or changes selected project/task.

The WorkZ review button is UI eligibility, not a host authorization or proof the task remains idle until a later review action. Existing AgentTaskReview/openPr limitations recorded under AZ013 remain unchanged. Delivery observations refresh manually and do not constitute an automatically live completion notification. Artifact viewer has explicit version capture/delete controls from AZ038; merely opening it does not mutate. Native GUI layout/focus and combined tests/typecheck are root-owned outstanding checks, not asserted here.

No blocking defect found in these added mounting/key/reset lines within the limited contract.

Reviewed SHA256:
- src/workspaces/codez/AssistantPanel.tsx: ab469eb32e479f537b7819a2e95311821398f091c6243a8393a2dfc755874673
- src/workspaces/workz/index.tsx: 472d401fd2bf8b543162f2ffe30a00b4e7aab145fd94a81a224d9b68a20cb866


# AZ041 scoped editor follow-up

# AZ041 editor preservation candidate independent review

2026-10-10. Independent read-only source review plus four real exported-helper/serializer integration fixtures. No Cargo, model, native filesystem races, Monaco mount or GUI acceptance run. Two defects found during review were corrected before this conclusion; no existing candidate installer evidence was changed.

EditorConflictTracker captures project/scope generation, tab identity, monotonic local revision, request generation, content and dirty state. Close/reopen/project ABA and same-content edits invalidate old operations. Confirmed reload consent belongs to the captured dirty revision; later edits cannot be discarded by that request. Failed/deleted reads consume their operation but preserve buffer/dirty state. Successful save only marks its exact captured current revision clean and invalidates pending reads. These are in-memory settlement guarantees, not disk-write CAS.

The central setTabs wrapper synchronously updates tracker and tabsRef before passing a concrete array to setTabsState. It evaluates its callback itself rather than giving React a side-effecting functional state updater, so StrictMode replay of React updater functions does not repeat tracker effects. handleEditorChange reports every edit before state publication, including same-content edits. The fixed setter also reports clean-to-dirty transitions even with equal text. Project-scope refs are updated during render: this is conservative operation invalidation, but an aborted concurrent render/real StrictMode/Monaco journey has not been exercised; this review does not claim general concurrent-render transactional correctness.

Dirty watcher deletion now retains the tab with visible explanatory error; accepted dirty reload no longer clears dirty before IO. Read/save replies require tracker settlement before applying React changes. Binary/read failures retain drafts. Save-before-close only closes after a successful current save with unchanged tab identity and clean state; bulk close checks current identities/dirty state and generation, preserving failed saves, newer edits and reopened tabs. Readonly/recovery save is refused. Dirty deleted buffers remain in the existing dirty-buffer snapshot; they are not a proven new durable recovery database.

Found and fixed hot-restore risk: an earlier restored draft formerly replaced any current same-path clean tab, including a user-saved or reopened tab during pending restore. mergeRestoredTabs now always preserves current same-path intent and stores the older dirty draft as an independent reserved recovery tab, including identical-content dirty drafts. Recovery tabs have unique IDs, readonly dirty text and original-path provenance. Real workspace serializer retains their open path and dirty text; restoration recognizes them before any disk read. Dirty persisted buffers are also preserved when the actual disk file becomes binary or unreadable. A restored buffer's disk base remains unknown; no three-way merge or checked-write proof is invented.

Index guards reserved paths out of open/read/reload/watch/diff/reveal, file-tree focus, editor mutation, save and Monaco URI management. Recovery rendering uses a dedicated readonly textarea/copy UI, not FileViewer/LSP. Explicit close uses discard-only wording, not a fake save; bulk dirty checks conservatively retain recovery drafts. Helpers themselves perform no native IPC and never infer disk content. Existing navigation snapshot registration remains, now reading synchronous tabsRef; opaque permission/scope APIs are unchanged. The shared existing AZ036 index fingerprint must be refreshed after review, not silently treated as unchanged.

Actual independent verification: editorRecovery.integration.test.ts four tests passed, final session59314 exit0, 14ms tests/4.10s total. They consume the real tracker (completed save and close/reopen), real merge policy, and real editorSnapshotFromTabs/restoreRecoveryTab, proving current clean intent survives, old draft remains, same-content dirty draft is not swallowed, persisted recovery round-trip retains readonly dirty text, and zero invoke calls. Earlier same target session3327 also passed; own-file ESLint83947 exit0. They do not mount the index or assert its wiring through fake copies. Final tracker test file contains twelve cases: three added fixtures were independently read and verify clean-to-dirty same-content invalidates pending read, successful read/save mutually invalidate captured operations, and old failures cannot clear newer dirty state. Root reports final session43548 tracker12 + recovery4 + navigation6 = twenty-two tests passed, exit0/4.36s. These combined results and source lint/typecheck are root-owned evidence, not our reruns.

No remaining blocking defect found within this local-buffer preservation/restore slice. Host ide_write_file still lacks expected disk revision CAS: parallel saves or external/Agent writes may race and an already sent old write cannot be rolled back by frontend stale settlement. All-write-actor concurrency, durable restart OS verification, explicit conflict comparison/merge, complete accessibility/localized recovery UI and real GUI journeys remain outstanding. Candidate protection is suitable for limited implementation consumption; full AZ041 and stable concurrent-editor acceptance are not declared complete.

Reviewed SHA256:
- src/workspaces/codez/editorConflict.ts: 18913faf070f34c48c1ffef4496aed0cba018f1f2b34400a7f0143e6beadf8a0
- src/workspaces/codez/editorConflict.test.ts: 1a186cb366b939c12b7dc9ac7434ffcb1d44515092aa2fcc8a714b104d913695
- src/workspaces/codez/editorRecovery.ts: 25ff29527254b07dd159f2952902557ac56c0d645a43beae8bf3f7cb030d7968
- src/workspaces/codez/editorRecovery.integration.test.ts: b252966ab7da5f4367501cd14e3123be65653709c54ca86d9a30f2b86be92446
- src/workspaces/codez/index.tsx: 4885b81cc033d4a9499f41088255f58524a74aec719fd2f1eea85b7792e96785


# Limited current translation consumer review

# AZ041 current i18n drift and limited consumer review

2026-10-10. Read-only investigation; neither translation file was edited. Existing old review/pin history must remain. This is a current limited-consumer review, not a claim that the full translation diff is semantically unchanged or authored by this task.

Old pins: en bb8eb0541b49ed46198f7e60ffaa313dcb13272d286f9105f02b451e949d5184; zh 04ff174670e9123ca34274447d7b254662a8b1189920f059ac315df5f6772a65. Current files contain LF only, no CR/BOM (en 1204 LF/51249 bytes, zh 1157 LF/51268 bytes). Normalizing current bytes to LF yields the current raw hashes below. Converting uniformly to CRLF yields en a9c0c61543b27e87843edd487e2bc60cd67e727d1f9de51866995603355456c9 and zh 1febc7baa944b336e6a2ed425cd173f463c2090f1030d217e8ce05f77313ad98: neither matches the old pins. Git HEAD hashes also differ from both old and current pins. Therefore simple uniform newline conversion does not explain the drift. Without old exact bytes, mixed historical line endings/formatting/semantic changes cannot be conclusively attributed or ruled out; comparing with HEAD alone is not a baseline comparison.

Current AZ036 consuming scope independently reviewed: app.saveBeforeProjectSwitch/Close and projectSwitchSaveFailed/projectCloseSaveFailed in both languages ask for saving edited buffers and state cancelled navigation on preservation failure. App uses these as explanatory text after actual dirty/failure preflight decisions, not as permission or state transitions; translations do not remove those guards. These are plain strings, not execution logic. AZ041 current ide.unsavedConfirm, unsavedBulkConfirm and externalChangeConflict clearly distinguish save-before-close from discarding local edits to reload disk. Name/count placeholders remain present in both languages. They do not grant file-write atomicity or fabricate a successful save.

Current AZ017 consuming scope independently reviewed: agent.isolateHint explicitly distinguishes Git worktree/branch isolation from separately controlled system-tool permissions in English and Chinese. agent.taskEnvironmentMissing states the historical task lacks a verified isolated binding, keeps history, requires explicit workspace recovery/new task and refuses silent directory movement. isolateOn/Off labels describe configuration only. These messages do not themselves authorize execution, broaden a filesystem sandbox, resolve a missing binding or turn a configured policy into an approval. Actual host/task identity and permission logic remain outside these files.

Limited conclusion: current relevant navigation/editor and task-environment texts support the existing fail-closed contracts and may be independently pinned for those specific consumers after root validation. The rest of both translation tables, full semantic equality to previous pins, origin of drift and GUI localization/overflow are not established by this review. No automatic ledger refresh, source restoration or stable-release claim is made here.

Current raw SHA256:
- src/i18n/en.ts: c718a52cd27c3b1f6e2dc0950fd6c4d3992ab6af9ae8d6b5837592670d69493e
- src/i18n/zh.ts: bbca86c830405bc983d8297bf000d34dfeea9975f85e6693ac22f2be739f8984


# AZ044 current CodeZ index limited independent review

2026-10-10. Reviewer: Codex current_source_review, independent read-only source inspection. No applicable AGENTS.md was found in repository or C:/Projects/AgentZ, C:/Projects, C:/ parents. Read AZ-041-shared-review.md and AZ-044-shared-source-drift.md before inspecting the current navigation/editor consumers. Only this new evidence file was written; no source, ledger or existing pinned evidence was edited. No tests, typecheck, native IPC, GUI or model evaluation was run by this reviewer.

Current raw SHA256 of src/workspaces/codez/index.tsx: 306ad89024ce5071bdc6d2c678f969d3fb0e8e8b40cf645555802d3a8997f84e. Historical reviewed SHA256: 4885b81cc033d4a9499f41088255f58524a74aec719fd2f1eea85b7792e96785. The exact historical reviewed dirty bytes are unavailable. This review establishes current limited behavior, not precise historical delta, semantic equivalence of the whole file, authorship of changes or a newline-only explanation. Git HEAD is not that missing baseline.

## AZ036 navigation snapshot and project identity

The registered navigation getter synchronously reads tabsRef and activeTabPathRef through editorSnapshotFromTabs; registration cleanup removes the getter. Central setTabs evaluates mutations against the live ref, synchronously reports close/open/content changes to EditorConflictTracker, updates tabMutation and tabsRef, then publishes the concrete array to React. Every editor edit reports a revision before state publication, including equal-content edits; clean-to-dirty equal-content changes are also reported by the setter. Navigation preflight can therefore consume current local buffers instead of a debounced snapshot. The App's actual preflight/persistence decisions are outside this file and are not newly re-certified here.

Current render-time scope invalidation and the reset effect both use exact project string identity. The reset effect explicitly compares prev !== projectDir, including spelling changes, rather than retaining rendered tabs after invalidating their operation ref. Scope changes advance editor generation, reset conflict scope, clear live tabs and open request ownership. Async open requests require captured generation and per-path request ownership before publishing; an existing same-path tab prevents a late open from replacing it. This is local asynchronous publication protection, not host filesystem authority or proof of all concurrent React render behavior.

## AZ041 current buffer preservation wiring

Watcher deletion retains dirty tabs with an explanatory error. Dirty reload requires explicit consent, captures the operation, and does not prematurely clear dirty state. Read replies settle tracker ownership before changing buffer text/dirty status; binary/read errors retain the buffer. Watch callbacks filter event project identity, and cleanup clears pending reload timers/operations. Manual reload similarly checks live project, excludes recovery/browser paths, and settles ownership before publishing.

saveFile refuses readonly/recovery tabs, captures current buffer identity and only clears dirty status after a matching successful settlement. A failed or stale save returns false. Save-before-close requires successful current save, matching tab identity and clean current state; bulk close also guards project generation, retains failed/newly dirty/reopened tabs and excludes recovery drafts. This still does not supply expected disk revision CAS: an already sent old write or external/Agent write may race on disk. That previously recorded limitation remains open.

Hot restore captures editor generation and mutation count; a changed project rejects late restoration. If local intent changed, mergeRestoredTabs preserves current same-path tabs and retains older dirty drafts independently. Reserved recovery paths are recognized before native reads, and persisted dirty text survives binary/unreadable disk results. Recovery content is rendered in a readonly textarea with copy action, outside FileViewer/Monaco/LSP; reserved paths are excluded from editor mutation, save, watcher/reload, file tree focus and model URI management. Explicit recovery close states discard-only; bulk close retains drafts. Native durable restart, explicit compare/merge and full localization/accessibility remain unverified.

## AZ044 mount change boundary and conclusion

BottomPanel is mounted while projectDir exists, with open controlling visibility rather than conditional mounting. This current wiring does not remove the navigation getter, operation generation or editor preservation guards inspected above. It is not independent certification of PTY ownership, process cleanup or terminal tests; those belong to AZ044's separate lifecycle review.

No additional blocking defect was found in the bounded AZ036 current snapshot/project-reset and AZ041 local editor settlement/recovery wiring inspected here. Root may refresh the current fingerprint for all affected ledger consumers in one validated operation, preserving previous review history and appending this limited current review. This is conditional on the file retaining the stated SHA256 and on retaining separate existing scopes/reviews; it is not permission to blindly refresh unrelated source fingerprints or claim full AZ036/AZ041 completion, desktop acceptance or stable release readiness.


# AZ044 current App and translation limited independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only independent inspection of current src/App.tsx, src/i18n/en.ts and src/i18n/zh.ts in the existing bounded AZ017/AZ036 navigation and task-environment explanation scopes. Read AZ-044-current-drift.json, previous AZ-041-shared-review.md and relevant AZ036/AZ017 review history. Applicable AGENTS search from this review sequence found none. Only this new evidence file is written; no source, ledger or existing pinned evidence is changed. No tests, typecheck, native IPC, GUI or model execution was run by this reviewer.

Current raw SHA256:
- src/App.tsx: 81d99ad3755dfaff43bd74343ba2bf0cc21bf2e9df5960358745646bba78f471
- src/i18n/en.ts: 95c75f1c919cf60a40217959be3d4710968b3329ddb4aaa6de80d9f578ec893b
- src/i18n/zh.ts: 14fdeba7b7caedb92907893010532009b716a5b94e6b30fa200a83b2027e3e29

Previous pins are App 79fd684ddaad63cb573db8740358aff68a3a08a818f573325a8d41361db2f465, en c718a52cd27c3b1f6e2dc0950fd6c4d3992ab6af9ae8d6b5837592670d69493e and zh bbca86c830405bc983d8297bf000d34dfeea9975f85e6693ac22f2be739f8984. Exact reviewed dirty baseline bytes are unavailable. No historical semantic equivalence, exact delta, authorship or newline-only explanation is claimed. This is a fresh limited current-consumer inspection.

## Current navigation guards

App stores a synchronous live editor getter and passes its stable registration callback into the continuously mounted CodeZ pane. Project switch and close assemble the live editor snapshot rather than relying solely on the debounced editor patch. Missing getter fails preflight; the helper clones snapshots, refuses dirty buffers, checks current route/snapshot across confirmation and successful persistence, and returns blocked on persistence failure. App checks ready before terminal destruction, scope revocation and route mutation. Busy mutex guards ordinary navigation; picker acquires the same mutex before awaiting native scope selection and holds it through adoption/rejected-scope cleanup, with the navigationOwned inner switch leaving release to the picker finally.

App rechecks current project and complete snapshot after terminal destruction and scope revocation before committing the route. These checks reject late edits and route changes; they cannot undo terminal cleanup or a completed revocation if subsequent checks fail. Cleanup failure is caught and prevents route success. openProjectInCodeZ only changes mode if awaited switchProject succeeds, including same-project success. Same normalized project returns early without a destructive switch. Scope acquisition/revocation calls use opaque existing service handles; this wiring does not grant authority from a path or demonstrate native lifecycle acceptance.

Historical workspace restoration attempts host restoreProjectScope for local projects. Failed scope restoration can retain editing restoration but does not construct a verified durable-plan authority in App; authority-gated actions remain responsible for rejecting unavailable scopes. Existing running-count close confirmation queries chat_running_count, cancels when requested, persists, then acknowledges close; that observation is not proof of complete process-tree cancellation or stable atomic Run state. These retained limitations are not newly certified by source inspection.

## Current relevant translation keys

English and Chinese app.saveBeforeProjectSwitch/Close require saving or closing edited files and unsaved drafts. projectSwitchSaveFailed/projectCloseSaveFailed explicitly describe cancelled navigation on workspace-preservation failure. App only displays these after corresponding actual blocked reasons; strings do not bypass guards or turn a failed operation into success.

agent.isolateHint in both tables separates Git worktree/branch editing isolation from independently controlled system-tool permissions. taskEnvironmentMissing explicitly identifies historical tasks without verified isolated binding, preserves history and requires a new task or explicit original-workspace recovery instead of silent directory movement. isolateOn/Off are configuration labels, not permission receipts or execution evidence. Relevant strings are plain data; no native execution or scope authority is created by their wording. The remainder of translation tables and localization layout are outside this review.

## Limited conclusion

No additional blocking defect was identified in the current inspected AZ017/AZ036 navigation, opaque-scope integration and relevant explanation-key consumers. Root may atomically refresh all affected consumers to these exact current hashes, preserving historical reviews and appending this evidence with the separate current index review. Any subsequent source change invalidates this conclusion's fingerprint. Full App equivalence, all Run observation/authorization semantics, actual native permission tests, whole-table translation equality, desktop roundtrip, model, rollback and full milestone acceptance are not established here. Previously documented partial cleanup/revocation effects and backend authority limitations remain open rather than being counted complete.


# AZ038 shared-source independent reviews

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


# AZ040 summary/Advanced independent implementation review

2026-10-10. Read-only review of InputContextSummary and the added CodeZ/WorkZ JSX integration. Existing unrelated repository diff was not attributed to this slice. No independent tests/build/models/GUI run.

The shared summary is presentational: props, translated strings and native details only; no IPC, tool execution, permission response, state mutation or scope minting. Unknown/empty strings remain Unknown, optional counts reject negative/unsafe values. Policy explicitly says configured policy does not grant permission. Workspace is explicitly configured context rather than host-verified environment. Queued text explicitly says “Queued locally”/“本地待发”; it is not persisted Run state. The approval display is a typed possibility but neither current caller supplies it, avoiding inference from stale permissionRequest.

CodeZ adds the summary immediately above its existing composer. Authority and workspace are the configured projectDir. Model uses current selected ID or loaded configured default label; the ID is a selection identifier, not a provider-confirmed effective model. Settings-loading failure falls back to Unknown. Busy displays Running, then existing error, then local queuedView presence, otherwise Idle. Busy takes precedence over pending local messages, so this slice is not an exhaustive queue dashboard. Existing Send/Stop and composer fields remain unchanged by the summary. CodeZ optional skills/connectors count is not yet supplied; their existing controls remain visible and functional, so no hidden-option behavior is introduced by this slice.

WorkZ only displays worktree_path when worktree.id matches sessionRef.current; otherwise the workspace fallback is configured free cwd/direct project, or Unknown for unverified isolated/agent/team contexts. Hidden model selector forces model/source Unknown and does not display the remembered scoped ID or global default. Mode labels separate agent/team/plain; policy remains configuration. Summary counts existing selected optional skills/connectors only when generic controls apply. No stale permission card is converted to waiting state, and no local queue is fabricated for WorkZ.

Native Advanced details keeps its children mounted while collapsed. Team/persona selectors retain original values, callbacks and taskBound/busy disabling. Free cwd and isolation remain outside the collapsed controls. The new start-source/base controls are scoped to isolate && !isFree && !taskBound and busy-disabled, feeding already existing startMode/startBase state consumed by task creation. The default remains working; no automatic switching or selection clearing was added. This adds access to existing configurable inputs rather than changing the send algorithm. Keyboard/focus, overlay placement within collapsed details, layout clipping and native desktop interaction still require GUI acceptance.

No blocking source defect was found in this bounded descriptive-summary/Advanced slice. It does not close budget presentation, permission receipt ownership, verified Run/Attempt live state, five-user first-use evaluation or full AZ040 acceptance. Selected model IDs are intentionally descriptive configured identifiers; future friendly-label resolution must preserve Unknown for missing provider definitions.

Reviewed SHA256:
- src/components/chat/InputContextSummary.tsx: 10ab6a5753f3874655cb2c9636c4b146afc0d8b63f68b3e024ab0f282e1c5847
- src/workspaces/codez/AssistantPanel.tsx: 7c82b46a98d816b34f4ab0ffedbff22f48fa6eaee0010cd9bfcaae25d82a7598
- src/workspaces/workz/index.tsx: e11099d392fe3184eb539db9b2c8b612b1b52af18ac12262c68a9c90ba51f468


# AZ040 observed-budget presentation independent review

2026-10-10. Read-only source/fixture review; no tests, Cargo, models or GUI executed by reviewer. Root reports author BudgetObservation seven and TaskDetailsPanel five targeted tests passed; final combined validation remains root-owned.

BudgetObservation renders totals only for an available typed source. Missing/unknown/stale/unavailable/unsupported sources have explicit status text and no numeric totals. Available known zero is preserved as an observation, not substituted for absent values; null policy maxima use Not configured via nullish checks. Known input/output, known active duration and usage count are distinct from unknown token/duration/pricing record counts. The fixtures cover each unavailable source, mixed known/unknown records, real known zero and absent cost observations.

Costs remain separate currency buckets in micro-units; no conversion, invoice claim, subtraction from maxima, remaining balance or token-to-price estimate is introduced. Empty cost map says No known cost observations. Configured maxima and observed exceedances are labelled observational; the component explicitly says enforcement is not connected, known totals may be incomplete and maxima are not an enforced allowance. It does not call the reservation API or authorize execution. Data validity is supplied by the existing strict taskDetails DTO validator; this component is not a replacement trust boundary.

TaskDetailsPanel change is an import plus replacement of its old budget paragraph with BudgetObservation using data.sources.budget. Existing scope/project/session request key masks old data during render before effect; generation/disposed checks suppress late responses, focus coalescing and error clearing remain intact. Therefore no old budget is exposed through the new child on a task/scope switch. Sources-read-separately/non-atomic disclaimer remains displayed. No polling or model continuation was added.

No blocking defect found within this observational presentation slice. Host accounting completeness, prices/invoices, reservation enforcement, team/member coverage and GUI usability remain unverified and are not inferred from these fixtures.

Reviewed SHA256:
- src/workspaces/codez/BudgetObservation.tsx: 567d6defee4616872f05a7995420e52d0cafdddc0b0a8ec24db8ad001878fa7c
- src/workspaces/codez/BudgetObservation.test.tsx: db01fad4a7d937dd0009db77a80a4499c501030b5112616e70c78ea20abcf22f
- src/workspaces/codez/TaskDetailsPanel.tsx: 8b925d75fb02697050ad3bc910999dfe5348cdf88a209bf3ee7b0f1bd9ad1a6d


# AZ039 bounded delivery observations independent implementation review

2026-10-10. Read-only review of four new summary/wrapper files. No tests/typecheck/build/GUI/model/external operations run by reviewer. WorkZ root mounting has not yet been included in this review. Author reports previous five-summary/four-wrapper fixtures passed; subsequent delivery fix requires root final validation rather than reuse of old results.

Found and resolved during review: initial summary ignored run.delivery and labelled all Acceptance as no receipt. Final source now separately displays authoritative Run delivery accepted/rejected/awaiting_review/not_ready, with added accepted/rejected fixture, while stating that a Run decision does not establish acceptance of a specific artifact version. Artifact-version acceptance, apply and publication still explicitly lack receipts. Full execution enums are shown independently; succeeded never implies validation pass or publication.

Artifacts are counted as distinct IDs and retained versions on this returned page only, with more-available marker. Validation kind counts recorded attachments and explicitly does not establish passed validation/acceptance. Missing store, unavailable/unknown/stale source remain labelled rather than fake zero-success; known empty available page can correctly count zero. No payload/tool text is interpreted as a verdict. Buttons only invoke view callbacks; no accept/apply/PR/merge/model action is introduced. Typed props rely on strict existing DTO services for runtime enum/scope validation, not untrusted raw JSON rendered directly.

DeliveryReviewPanel uses opaque useProjectScope handle, scoped readTaskDetails and fresh observeTaskView before bounded listTaskArtifacts(null,200). Separate sources use allSettled: one failure preserves another valid source but marks failing evidence unavailable rather than empty/pass. Independent project/task/environment mismatch marks artifacts stale. It does not claim binding revisions or separately read sources are atomic. No continuous polling or cursor synthesis occurs.

Render key project/session/handle masks old data before effect; generation cleanup discards late replies and suppresses list invocation when observation outlives its generation. Source loading resets previous values; scope missing prevents reads. Fixtures exercise bounded fresh observations, failed list without leaking private error, late prior-task responses, cross-environment stale and no-scope. Run details may be available while artifacts fail, which is intentionally distinct rather than global successful delivery. Manual refresh remains observational and no active chat projection is rewritten.

No blocking defect remains within this bounded summary/wrapper contract. Exact-version acceptance receipts, complete multi-page artifact enumeration, stable cross-source snapshot, old openPr safety, native GUI and full AZ039 acceptance remain outstanding. This candidate may expose existing review/artifact viewers but does not itself implement their authorization or external-delivery semantics.

Reviewed SHA256:
- src/workspaces/codez/DeliverySummary.tsx: 504f8776ef6ccc20c18b7d6dd84c0f263f299452336cafbb236d1c05b6cbb4df
- src/workspaces/codez/DeliverySummary.test.tsx: f06f3adec81df0e7e2eb851b1b0420752365c10084e3252db59ca0b8f3b6776c
- src/workspaces/codez/DeliveryReviewPanel.tsx: 237c6d2c2310fda3379729fc29acbbbf3f401b2294cceb5d37949ab6c8681458
- src/workspaces/codez/DeliveryReviewPanel.test.tsx: cd5d1f9a732a9e10909e1c6c33ceb123d81f4ce8a67d530be59414e3040b87f0


# AZ039 two-workspace wiring independent review

2026-10-10. Read-only inspection of root integration; no tests/typecheck/GUI/model or Git operations executed. The four new component files are covered separately by AZ-039-implementation-review.md, left unchanged.

CodeZ mounts DeliveryReviewPanel only with a current project and session, using those inputs for the scope-aware observer. Open artifacts sets a project/session identity key; TaskArtifactViewer mounts only when that key matches the current pair and is React-keyed by it. Root added key clearing on project reset, newSession and switchSession start, preventing an existing artifact preview surviving asynchronous selection of another session. No review/merge/PR callback is attached to CodeZ; its existing composer/stream handling is otherwise unchanged by this addition.

WorkZ reuses the existing non-team/non-pool, selected-project/canonical-project/current-session gate. It passes canonical authority to DeliveryReviewPanel. Review changes is present only when not busy and worktree.id matches sessionRef.current, and only opens the existing review UI; no merge/discard/PR runs automatically. Open artifacts uses the same canonical-project/session key as the existing viewer. Old preview clearing at openTask async start remains present. Changing the project/session prevents mismatched-key viewer mounting; child scope generation validation remains the actual asynchronous-response isolation. Neither button issues native permission or changes selected project/task.

The WorkZ review button is UI eligibility, not a host authorization or proof the task remains idle until a later review action. Existing AgentTaskReview/openPr limitations recorded under AZ013 remain unchanged. Delivery observations refresh manually and do not constitute an automatically live completion notification. Artifact viewer has explicit version capture/delete controls from AZ038; merely opening it does not mutate. Native GUI layout/focus and combined tests/typecheck are root-owned outstanding checks, not asserted here.

No blocking defect found in these added mounting/key/reset lines within the limited contract.

Reviewed SHA256:
- src/workspaces/codez/AssistantPanel.tsx: ab469eb32e479f537b7819a2e95311821398f091c6243a8393a2dfc755874673
- src/workspaces/workz/index.tsx: 472d401fd2bf8b543162f2ffe30a00b4e7aab145fd94a81a224d9b68a20cb866


# AZ048 projectless directory independent review

2026-10-10/11. Reviewer: Codex current_source_review. Read-only module/export inspection; no source, ledger or existing pinned evidence edits. Root reports offline locked domain session24352 one actual filesystem fixture passed. Reviewer did not run Cargo, native IPC or GUI. Host integration is absent.

Observed SHA256:
- crates/agentz-task-state/src/projectless.rs: 3df63fb04bdbf8cb797160440815ac0c6bd7ddab05e00cc8cd191353c16f33f2
- crates/agentz-task-state/src/lib.rs: ba73a74324b07960b853780e797af76d34929047c03f9008505385e0fb2acc73

checked_id requires 36 bytes with fixed hyphens and lowercase hexadecimal, preventing path separators/traversal, though it accepts any UUID-shaped bits rather than enforcing a UUID version. create requires an existing ordinary dedicated root, canonicalizes it, uses nonrecursive create_dir so existing task paths are never adopted, and writes a create_new ownership marker with exact task ID plus sync_all. It does not initialize Git or delete anything. Marker creation/write/sync/open validation failures can leave a partial directory/marker; retry create then fails rather than silently adopting it. Errors are not rollback claims. No root initialization, ledger registration or transactional durable task identity is provided.

open verifies current root/task directory metadata, rejects directory symlink and Windows reparse attributes, canonicalizes the child and requires its parent equal root. Marker must be a regular nonsymlink file with length36 and exact ID bytes. A removed/corrupt marker fails instead of reconstructing state. The actual filesystem fixture proves artifact text survives reopen, Git absence, duplicate-create refusal, traversal refusal and corrupt-marker refusal with artifacts retained. It does not exercise reparse/hardlink/races or interrupted creation.

## Required marker check

At this fingerprint marker validation lacks the Windows FILE_ATTRIBUTE_REPARSE_POINT check used by ordinary_directory. Windows non-symlink reparse files may appear regular and pass is_symlink=false. Add matching marker reparse rejection before reading; report sent to root. The current module should not be frozen as enforcing ordinary marker identity until this is addressed and reviewed. No source modification by reviewer.

## Trust and race limits

Caller must supply a dedicated application-owned root and authoritative task ID; returned path/marker is not permission. This public API cannot distinguish a user project root from a trusted root. Ancestor components are not independently checked. Metadata→canonicalization→create/open→read are separate path operations with TOCTOU; marker read is not same-handle/no-follow, length check does not bound a replaced file's allocation, and marker hardlinks are not rejected. Equal marker contents establish current naming/content consistency, not durable filesystem object identity, unforgeable ownership or hostile concurrent-root safety. Cooperative exclusive root access is required until stronger native containment/lease handling is implemented. Returned paths can be replaced after validation, so callers need fresh authorization for later IO. Directory/parent sync and crash durability are not proven by marker sync alone.

lib.rs currently contains a trailing pub mod projectless export with no added global initializer or call at that export. Exporting this module alone does not create directories or change startup permission. Exact historical reviewed dirty bytes are missing; this inspection cannot prove the entire lib diff is only that export or all previous functions unchanged. Seven upstream consumers may receive a limited export/current-fingerprint refresh preserving prior histories, conditioned on root independently validating current source and retaining its earlier scopes; this review is not whole-lib historical equivalence. New module itself remains subject to the marker finding above. Full AZ048 task acceptance, projectless host persistence/routing, native authorization and GUI remain open.


# AZ048 marker reparse follow-up independent review

2026-10-11. Reviewer: Codex current_source_review. Read-only inspection of the corrected module and current lib fingerprint. Historical AZ-048-directory-independent-review.md preserves the pre-fix finding. No source, ledger or pinned evidence edited; no Cargo/native/GUI execution by reviewer. Root's final targeted Cargo53801 remains pending at request, so the earlier one-test pass does not validate these exact final bytes.

Current raw SHA256:
- crates/agentz-task-state/src/projectless.rs: 6059245cc488d14c77807d4bba89937a9057c1b5181e765fc566e30dd9a69682
- crates/agentz-task-state/src/lib.rs: ba73a74324b07960b853780e797af76d34929047c03f9008505385e0fb2acc73

The reported marker deficiency is closed in inspected source. Following the regular-file/nonsymlink/exact-length metadata check, the Windows-specific block imports MetadataExt and rejects marker.file_attributes() & 0x400 != 0 before read_to_string. This mirrors explicit directory reparse refusal and includes non-symlink reparse files. Formatting does not itself prove behavior; this conclusion is from the actual conditional and ordering. The existing filesystem fixture does not create a Windows reparse marker, so native reparse-case rejection is source-inspected rather than dynamically demonstrated.

The limited create/reopen primitive is reviewable after owner obtains exact-source targeted verification: it uses a dedicated caller-owned root and UUID-shaped path, refuses existing creation, preserves artifacts/no Git initialization, verifies current marker contents and avoids implicit deletion/recovery. lib export hash is unchanged from the prior review, and its limited no-initialization export conclusion remains applicable. Upstream fingerprint refresh must preserve prior scopes/history; entire historical lib byte equivalence is not established.

Trust constraints remain mandatory: caller selects genuinely application-owned root/authoritative ID and excludes untrusted concurrent mutation. Ancestor root reparse/symlink replacement, metadata/canonicalization/read/create TOCTOU, hardlink marker substitution and replacement-file read allocation are not resolved. No same-handle identity, unforgeable ownership, durable filesystem object identity, directory crash durability or transactional task-registration guarantee exists. Partial creation can retain orphan directories/markers and later create fails rather than adopting them. Host is not connected; this adds no IPC authority, execution permission, task routing, GUI behavior or complete AZ048 acceptance. No remaining blocker was found for this explicitly trusted-root, cooperative-use domain slice; hostile-root safety and native integration remain separate unfinished work.


## Preserved review: docs/upgrade/evidence/AZ-044-shared-review.md

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


# AZ040 summary/Advanced independent implementation review

2026-10-10. Read-only review of InputContextSummary and the added CodeZ/WorkZ JSX integration. Existing unrelated repository diff was not attributed to this slice. No independent tests/build/models/GUI run.

The shared summary is presentational: props, translated strings and native details only; no IPC, tool execution, permission response, state mutation or scope minting. Unknown/empty strings remain Unknown, optional counts reject negative/unsafe values. Policy explicitly says configured policy does not grant permission. Workspace is explicitly configured context rather than host-verified environment. Queued text explicitly says “Queued locally”/“本地待发”; it is not persisted Run state. The approval display is a typed possibility but neither current caller supplies it, avoiding inference from stale permissionRequest.

CodeZ adds the summary immediately above its existing composer. Authority and workspace are the configured projectDir. Model uses current selected ID or loaded configured default label; the ID is a selection identifier, not a provider-confirmed effective model. Settings-loading failure falls back to Unknown. Busy displays Running, then existing error, then local queuedView presence, otherwise Idle. Busy takes precedence over pending local messages, so this slice is not an exhaustive queue dashboard. Existing Send/Stop and composer fields remain unchanged by the summary. CodeZ optional skills/connectors count is not yet supplied; their existing controls remain visible and functional, so no hidden-option behavior is introduced by this slice.

WorkZ only displays worktree_path when worktree.id matches sessionRef.current; otherwise the workspace fallback is configured free cwd/direct project, or Unknown for unverified isolated/agent/team contexts. Hidden model selector forces model/source Unknown and does not display the remembered scoped ID or global default. Mode labels separate agent/team/plain; policy remains configuration. Summary counts existing selected optional skills/connectors only when generic controls apply. No stale permission card is converted to waiting state, and no local queue is fabricated for WorkZ.

Native Advanced details keeps its children mounted while collapsed. Team/persona selectors retain original values, callbacks and taskBound/busy disabling. Free cwd and isolation remain outside the collapsed controls. The new start-source/base controls are scoped to isolate && !isFree && !taskBound and busy-disabled, feeding already existing startMode/startBase state consumed by task creation. The default remains working; no automatic switching or selection clearing was added. This adds access to existing configurable inputs rather than changing the send algorithm. Keyboard/focus, overlay placement within collapsed details, layout clipping and native desktop interaction still require GUI acceptance.

No blocking source defect was found in this bounded descriptive-summary/Advanced slice. It does not close budget presentation, permission receipt ownership, verified Run/Attempt live state, five-user first-use evaluation or full AZ040 acceptance. Selected model IDs are intentionally descriptive configured identifiers; future friendly-label resolution must preserve Unknown for missing provider definitions.

Reviewed SHA256:
- src/components/chat/InputContextSummary.tsx: 10ab6a5753f3874655cb2c9636c4b146afc0d8b63f68b3e024ab0f282e1c5847
- src/workspaces/codez/AssistantPanel.tsx: 7c82b46a98d816b34f4ab0ffedbff22f48fa6eaee0010cd9bfcaae25d82a7598
- src/workspaces/workz/index.tsx: e11099d392fe3184eb539db9b2c8b612b1b52af18ac12262c68a9c90ba51f468


# AZ040 observed-budget presentation independent review

2026-10-10. Read-only source/fixture review; no tests, Cargo, models or GUI executed by reviewer. Root reports author BudgetObservation seven and TaskDetailsPanel five targeted tests passed; final combined validation remains root-owned.

BudgetObservation renders totals only for an available typed source. Missing/unknown/stale/unavailable/unsupported sources have explicit status text and no numeric totals. Available known zero is preserved as an observation, not substituted for absent values; null policy maxima use Not configured via nullish checks. Known input/output, known active duration and usage count are distinct from unknown token/duration/pricing record counts. The fixtures cover each unavailable source, mixed known/unknown records, real known zero and absent cost observations.

Costs remain separate currency buckets in micro-units; no conversion, invoice claim, subtraction from maxima, remaining balance or token-to-price estimate is introduced. Empty cost map says No known cost observations. Configured maxima and observed exceedances are labelled observational; the component explicitly says enforcement is not connected, known totals may be incomplete and maxima are not an enforced allowance. It does not call the reservation API or authorize execution. Data validity is supplied by the existing strict taskDetails DTO validator; this component is not a replacement trust boundary.

TaskDetailsPanel change is an import plus replacement of its old budget paragraph with BudgetObservation using data.sources.budget. Existing scope/project/session request key masks old data during render before effect; generation/disposed checks suppress late responses, focus coalescing and error clearing remain intact. Therefore no old budget is exposed through the new child on a task/scope switch. Sources-read-separately/non-atomic disclaimer remains displayed. No polling or model continuation was added.

No blocking defect found within this observational presentation slice. Host accounting completeness, prices/invoices, reservation enforcement, team/member coverage and GUI usability remain unverified and are not inferred from these fixtures.

Reviewed SHA256:
- src/workspaces/codez/BudgetObservation.tsx: 567d6defee4616872f05a7995420e52d0cafdddc0b0a8ec24db8ad001878fa7c
- src/workspaces/codez/BudgetObservation.test.tsx: db01fad4a7d937dd0009db77a80a4499c501030b5112616e70c78ea20abcf22f
- src/workspaces/codez/TaskDetailsPanel.tsx: 8b925d75fb02697050ad3bc910999dfe5348cdf88a209bf3ee7b0f1bd9ad1a6d


# AZ039 bounded delivery observations independent implementation review

2026-10-10. Read-only review of four new summary/wrapper files. No tests/typecheck/build/GUI/model/external operations run by reviewer. WorkZ root mounting has not yet been included in this review. Author reports previous five-summary/four-wrapper fixtures passed; subsequent delivery fix requires root final validation rather than reuse of old results.

Found and resolved during review: initial summary ignored run.delivery and labelled all Acceptance as no receipt. Final source now separately displays authoritative Run delivery accepted/rejected/awaiting_review/not_ready, with added accepted/rejected fixture, while stating that a Run decision does not establish acceptance of a specific artifact version. Artifact-version acceptance, apply and publication still explicitly lack receipts. Full execution enums are shown independently; succeeded never implies validation pass or publication.

Artifacts are counted as distinct IDs and retained versions on this returned page only, with more-available marker. Validation kind counts recorded attachments and explicitly does not establish passed validation/acceptance. Missing store, unavailable/unknown/stale source remain labelled rather than fake zero-success; known empty available page can correctly count zero. No payload/tool text is interpreted as a verdict. Buttons only invoke view callbacks; no accept/apply/PR/merge/model action is introduced. Typed props rely on strict existing DTO services for runtime enum/scope validation, not untrusted raw JSON rendered directly.

DeliveryReviewPanel uses opaque useProjectScope handle, scoped readTaskDetails and fresh observeTaskView before bounded listTaskArtifacts(null,200). Separate sources use allSettled: one failure preserves another valid source but marks failing evidence unavailable rather than empty/pass. Independent project/task/environment mismatch marks artifacts stale. It does not claim binding revisions or separately read sources are atomic. No continuous polling or cursor synthesis occurs.

Render key project/session/handle masks old data before effect; generation cleanup discards late replies and suppresses list invocation when observation outlives its generation. Source loading resets previous values; scope missing prevents reads. Fixtures exercise bounded fresh observations, failed list without leaking private error, late prior-task responses, cross-environment stale and no-scope. Run details may be available while artifacts fail, which is intentionally distinct rather than global successful delivery. Manual refresh remains observational and no active chat projection is rewritten.

No blocking defect remains within this bounded summary/wrapper contract. Exact-version acceptance receipts, complete multi-page artifact enumeration, stable cross-source snapshot, old openPr safety, native GUI and full AZ039 acceptance remain outstanding. This candidate may expose existing review/artifact viewers but does not itself implement their authorization or external-delivery semantics.

Reviewed SHA256:
- src/workspaces/codez/DeliverySummary.tsx: 504f8776ef6ccc20c18b7d6dd84c0f263f299452336cafbb236d1c05b6cbb4df
- src/workspaces/codez/DeliverySummary.test.tsx: f06f3adec81df0e7e2eb851b1b0420752365c10084e3252db59ca0b8f3b6776c
- src/workspaces/codez/DeliveryReviewPanel.tsx: 237c6d2c2310fda3379729fc29acbbbf3f401b2294cceb5d37949ab6c8681458
- src/workspaces/codez/DeliveryReviewPanel.test.tsx: cd5d1f9a732a9e10909e1c6c33ceb123d81f4ce8a67d530be59414e3040b87f0


# AZ039 two-workspace wiring independent review

2026-10-10. Read-only inspection of root integration; no tests/typecheck/GUI/model or Git operations executed. The four new component files are covered separately by AZ-039-implementation-review.md, left unchanged.

CodeZ mounts DeliveryReviewPanel only with a current project and session, using those inputs for the scope-aware observer. Open artifacts sets a project/session identity key; TaskArtifactViewer mounts only when that key matches the current pair and is React-keyed by it. Root added key clearing on project reset, newSession and switchSession start, preventing an existing artifact preview surviving asynchronous selection of another session. No review/merge/PR callback is attached to CodeZ; its existing composer/stream handling is otherwise unchanged by this addition.

WorkZ reuses the existing non-team/non-pool, selected-project/canonical-project/current-session gate. It passes canonical authority to DeliveryReviewPanel. Review changes is present only when not busy and worktree.id matches sessionRef.current, and only opens the existing review UI; no merge/discard/PR runs automatically. Open artifacts uses the same canonical-project/session key as the existing viewer. Old preview clearing at openTask async start remains present. Changing the project/session prevents mismatched-key viewer mounting; child scope generation validation remains the actual asynchronous-response isolation. Neither button issues native permission or changes selected project/task.

The WorkZ review button is UI eligibility, not a host authorization or proof the task remains idle until a later review action. Existing AgentTaskReview/openPr limitations recorded under AZ013 remain unchanged. Delivery observations refresh manually and do not constitute an automatically live completion notification. Artifact viewer has explicit version capture/delete controls from AZ038; merely opening it does not mutate. Native GUI layout/focus and combined tests/typecheck are root-owned outstanding checks, not asserted here.

No blocking defect found in these added mounting/key/reset lines within the limited contract.

Reviewed SHA256:
- src/workspaces/codez/AssistantPanel.tsx: ab469eb32e479f537b7819a2e95311821398f091c6243a8393a2dfc755874673
- src/workspaces/workz/index.tsx: 472d401fd2bf8b543162f2ffe30a00b4e7aab145fd94a81a224d9b68a20cb866


# AZ041 scoped editor follow-up

# AZ041 editor preservation candidate independent review

2026-10-10. Independent read-only source review plus four real exported-helper/serializer integration fixtures. No Cargo, model, native filesystem races, Monaco mount or GUI acceptance run. Two defects found during review were corrected before this conclusion; no existing candidate installer evidence was changed.

EditorConflictTracker captures project/scope generation, tab identity, monotonic local revision, request generation, content and dirty state. Close/reopen/project ABA and same-content edits invalidate old operations. Confirmed reload consent belongs to the captured dirty revision; later edits cannot be discarded by that request. Failed/deleted reads consume their operation but preserve buffer/dirty state. Successful save only marks its exact captured current revision clean and invalidates pending reads. These are in-memory settlement guarantees, not disk-write CAS.

The central setTabs wrapper synchronously updates tracker and tabsRef before passing a concrete array to setTabsState. It evaluates its callback itself rather than giving React a side-effecting functional state updater, so StrictMode replay of React updater functions does not repeat tracker effects. handleEditorChange reports every edit before state publication, including same-content edits. The fixed setter also reports clean-to-dirty transitions even with equal text. Project-scope refs are updated during render: this is conservative operation invalidation, but an aborted concurrent render/real StrictMode/Monaco journey has not been exercised; this review does not claim general concurrent-render transactional correctness.

Dirty watcher deletion now retains the tab with visible explanatory error; accepted dirty reload no longer clears dirty before IO. Read/save replies require tracker settlement before applying React changes. Binary/read failures retain drafts. Save-before-close only closes after a successful current save with unchanged tab identity and clean state; bulk close checks current identities/dirty state and generation, preserving failed saves, newer edits and reopened tabs. Readonly/recovery save is refused. Dirty deleted buffers remain in the existing dirty-buffer snapshot; they are not a proven new durable recovery database.

Found and fixed hot-restore risk: an earlier restored draft formerly replaced any current same-path clean tab, including a user-saved or reopened tab during pending restore. mergeRestoredTabs now always preserves current same-path intent and stores the older dirty draft as an independent reserved recovery tab, including identical-content dirty drafts. Recovery tabs have unique IDs, readonly dirty text and original-path provenance. Real workspace serializer retains their open path and dirty text; restoration recognizes them before any disk read. Dirty persisted buffers are also preserved when the actual disk file becomes binary or unreadable. A restored buffer's disk base remains unknown; no three-way merge or checked-write proof is invented.

Index guards reserved paths out of open/read/reload/watch/diff/reveal, file-tree focus, editor mutation, save and Monaco URI management. Recovery rendering uses a dedicated readonly textarea/copy UI, not FileViewer/LSP. Explicit close uses discard-only wording, not a fake save; bulk dirty checks conservatively retain recovery drafts. Helpers themselves perform no native IPC and never infer disk content. Existing navigation snapshot registration remains, now reading synchronous tabsRef; opaque permission/scope APIs are unchanged. The shared existing AZ036 index fingerprint must be refreshed after review, not silently treated as unchanged.

Actual independent verification: editorRecovery.integration.test.ts four tests passed, final session59314 exit0, 14ms tests/4.10s total. They consume the real tracker (completed save and close/reopen), real merge policy, and real editorSnapshotFromTabs/restoreRecoveryTab, proving current clean intent survives, old draft remains, same-content dirty draft is not swallowed, persisted recovery round-trip retains readonly dirty text, and zero invoke calls. Earlier same target session3327 also passed; own-file ESLint83947 exit0. They do not mount the index or assert its wiring through fake copies. Final tracker test file contains twelve cases: three added fixtures were independently read and verify clean-to-dirty same-content invalidates pending read, successful read/save mutually invalidate captured operations, and old failures cannot clear newer dirty state. Root reports final session43548 tracker12 + recovery4 + navigation6 = twenty-two tests passed, exit0/4.36s. These combined results and source lint/typecheck are root-owned evidence, not our reruns.

No remaining blocking defect found within this local-buffer preservation/restore slice. Host ide_write_file still lacks expected disk revision CAS: parallel saves or external/Agent writes may race and an already sent old write cannot be rolled back by frontend stale settlement. All-write-actor concurrency, durable restart OS verification, explicit conflict comparison/merge, complete accessibility/localized recovery UI and real GUI journeys remain outstanding. Candidate protection is suitable for limited implementation consumption; full AZ041 and stable concurrent-editor acceptance are not declared complete.

Reviewed SHA256:
- src/workspaces/codez/editorConflict.ts: 18913faf070f34c48c1ffef4496aed0cba018f1f2b34400a7f0143e6beadf8a0
- src/workspaces/codez/editorConflict.test.ts: 1a186cb366b939c12b7dc9ac7434ffcb1d44515092aa2fcc8a714b104d913695
- src/workspaces/codez/editorRecovery.ts: 25ff29527254b07dd159f2952902557ac56c0d645a43beae8bf3f7cb030d7968
- src/workspaces/codez/editorRecovery.integration.test.ts: b252966ab7da5f4367501cd14e3123be65653709c54ca86d9a30f2b86be92446
- src/workspaces/codez/index.tsx: 4885b81cc033d4a9499f41088255f58524a74aec719fd2f1eea85b7792e96785


# Limited current translation consumer review

# AZ041 current i18n drift and limited consumer review

2026-10-10. Read-only investigation; neither translation file was edited. Existing old review/pin history must remain. This is a current limited-consumer review, not a claim that the full translation diff is semantically unchanged or authored by this task.

Old pins: en bb8eb0541b49ed46198f7e60ffaa313dcb13272d286f9105f02b451e949d5184; zh 04ff174670e9123ca34274447d7b254662a8b1189920f059ac315df5f6772a65. Current files contain LF only, no CR/BOM (en 1204 LF/51249 bytes, zh 1157 LF/51268 bytes). Normalizing current bytes to LF yields the current raw hashes below. Converting uniformly to CRLF yields en a9c0c61543b27e87843edd487e2bc60cd67e727d1f9de51866995603355456c9 and zh 1febc7baa944b336e6a2ed425cd173f463c2090f1030d217e8ce05f77313ad98: neither matches the old pins. Git HEAD hashes also differ from both old and current pins. Therefore simple uniform newline conversion does not explain the drift. Without old exact bytes, mixed historical line endings/formatting/semantic changes cannot be conclusively attributed or ruled out; comparing with HEAD alone is not a baseline comparison.

Current AZ036 consuming scope independently reviewed: app.saveBeforeProjectSwitch/Close and projectSwitchSaveFailed/projectCloseSaveFailed in both languages ask for saving edited buffers and state cancelled navigation on preservation failure. App uses these as explanatory text after actual dirty/failure preflight decisions, not as permission or state transitions; translations do not remove those guards. These are plain strings, not execution logic. AZ041 current ide.unsavedConfirm, unsavedBulkConfirm and externalChangeConflict clearly distinguish save-before-close from discarding local edits to reload disk. Name/count placeholders remain present in both languages. They do not grant file-write atomicity or fabricate a successful save.

Current AZ017 consuming scope independently reviewed: agent.isolateHint explicitly distinguishes Git worktree/branch isolation from separately controlled system-tool permissions in English and Chinese. agent.taskEnvironmentMissing states the historical task lacks a verified isolated binding, keeps history, requires explicit workspace recovery/new task and refuses silent directory movement. isolateOn/Off labels describe configuration only. These messages do not themselves authorize execution, broaden a filesystem sandbox, resolve a missing binding or turn a configured policy into an approval. Actual host/task identity and permission logic remain outside these files.

Limited conclusion: current relevant navigation/editor and task-environment texts support the existing fail-closed contracts and may be independently pinned for those specific consumers after root validation. The rest of both translation tables, full semantic equality to previous pins, origin of drift and GUI localization/overflow are not established by this review. No automatic ledger refresh, source restoration or stable-release claim is made here.

Current raw SHA256:
- src/i18n/en.ts: c718a52cd27c3b1f6e2dc0950fd6c4d3992ab6af9ae8d6b5837592670d69493e
- src/i18n/zh.ts: bbca86c830405bc983d8297bf000d34dfeea9975f85e6693ac22f2be739f8984


# AZ044 current CodeZ index limited independent review

2026-10-10. Reviewer: Codex current_source_review, independent read-only source inspection. No applicable AGENTS.md was found in repository or C:/Projects/AgentZ, C:/Projects, C:/ parents. Read AZ-041-shared-review.md and AZ-044-shared-source-drift.md before inspecting the current navigation/editor consumers. Only this new evidence file was written; no source, ledger or existing pinned evidence was edited. No tests, typecheck, native IPC, GUI or model evaluation was run by this reviewer.

Current raw SHA256 of src/workspaces/codez/index.tsx: 306ad89024ce5071bdc6d2c678f969d3fb0e8e8b40cf645555802d3a8997f84e. Historical reviewed SHA256: 4885b81cc033d4a9499f41088255f58524a74aec719fd2f1eea85b7792e96785. The exact historical reviewed dirty bytes are unavailable. This review establishes current limited behavior, not precise historical delta, semantic equivalence of the whole file, authorship of changes or a newline-only explanation. Git HEAD is not that missing baseline.

## AZ036 navigation snapshot and project identity

The registered navigation getter synchronously reads tabsRef and activeTabPathRef through editorSnapshotFromTabs; registration cleanup removes the getter. Central setTabs evaluates mutations against the live ref, synchronously reports close/open/content changes to EditorConflictTracker, updates tabMutation and tabsRef, then publishes the concrete array to React. Every editor edit reports a revision before state publication, including equal-content edits; clean-to-dirty equal-content changes are also reported by the setter. Navigation preflight can therefore consume current local buffers instead of a debounced snapshot. The App's actual preflight/persistence decisions are outside this file and are not newly re-certified here.

Current render-time scope invalidation and the reset effect both use exact project string identity. The reset effect explicitly compares prev !== projectDir, including spelling changes, rather than retaining rendered tabs after invalidating their operation ref. Scope changes advance editor generation, reset conflict scope, clear live tabs and open request ownership. Async open requests require captured generation and per-path request ownership before publishing; an existing same-path tab prevents a late open from replacing it. This is local asynchronous publication protection, not host filesystem authority or proof of all concurrent React render behavior.

## AZ041 current buffer preservation wiring

Watcher deletion retains dirty tabs with an explanatory error. Dirty reload requires explicit consent, captures the operation, and does not prematurely clear dirty state. Read replies settle tracker ownership before changing buffer text/dirty status; binary/read errors retain the buffer. Watch callbacks filter event project identity, and cleanup clears pending reload timers/operations. Manual reload similarly checks live project, excludes recovery/browser paths, and settles ownership before publishing.

saveFile refuses readonly/recovery tabs, captures current buffer identity and only clears dirty status after a matching successful settlement. A failed or stale save returns false. Save-before-close requires successful current save, matching tab identity and clean current state; bulk close also guards project generation, retains failed/newly dirty/reopened tabs and excludes recovery drafts. This still does not supply expected disk revision CAS: an already sent old write or external/Agent write may race on disk. That previously recorded limitation remains open.

Hot restore captures editor generation and mutation count; a changed project rejects late restoration. If local intent changed, mergeRestoredTabs preserves current same-path tabs and retains older dirty drafts independently. Reserved recovery paths are recognized before native reads, and persisted dirty text survives binary/unreadable disk results. Recovery content is rendered in a readonly textarea with copy action, outside FileViewer/Monaco/LSP; reserved paths are excluded from editor mutation, save, watcher/reload, file tree focus and model URI management. Explicit recovery close states discard-only; bulk close retains drafts. Native durable restart, explicit compare/merge and full localization/accessibility remain unverified.

## AZ044 mount change boundary and conclusion

BottomPanel is mounted while projectDir exists, with open controlling visibility rather than conditional mounting. This current wiring does not remove the navigation getter, operation generation or editor preservation guards inspected above. It is not independent certification of PTY ownership, process cleanup or terminal tests; those belong to AZ044's separate lifecycle review.

No additional blocking defect was found in the bounded AZ036 current snapshot/project-reset and AZ041 local editor settlement/recovery wiring inspected here. Root may refresh the current fingerprint for all affected ledger consumers in one validated operation, preserving previous review history and appending this limited current review. This is conditional on the file retaining the stated SHA256 and on retaining separate existing scopes/reviews; it is not permission to blindly refresh unrelated source fingerprints or claim full AZ036/AZ041 completion, desktop acceptance or stable release readiness.


# AZ044 current App and translation limited independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only independent inspection of current src/App.tsx, src/i18n/en.ts and src/i18n/zh.ts in the existing bounded AZ017/AZ036 navigation and task-environment explanation scopes. Read AZ-044-current-drift.json, previous AZ-041-shared-review.md and relevant AZ036/AZ017 review history. Applicable AGENTS search from this review sequence found none. Only this new evidence file is written; no source, ledger or existing pinned evidence is changed. No tests, typecheck, native IPC, GUI or model execution was run by this reviewer.

Current raw SHA256:
- src/App.tsx: 81d99ad3755dfaff43bd74343ba2bf0cc21bf2e9df5960358745646bba78f471
- src/i18n/en.ts: 95c75f1c919cf60a40217959be3d4710968b3329ddb4aaa6de80d9f578ec893b
- src/i18n/zh.ts: 14fdeba7b7caedb92907893010532009b716a5b94e6b30fa200a83b2027e3e29

Previous pins are App 79fd684ddaad63cb573db8740358aff68a3a08a818f573325a8d41361db2f465, en c718a52cd27c3b1f6e2dc0950fd6c4d3992ab6af9ae8d6b5837592670d69493e and zh bbca86c830405bc983d8297bf000d34dfeea9975f85e6693ac22f2be739f8984. Exact reviewed dirty baseline bytes are unavailable. No historical semantic equivalence, exact delta, authorship or newline-only explanation is claimed. This is a fresh limited current-consumer inspection.

## Current navigation guards

App stores a synchronous live editor getter and passes its stable registration callback into the continuously mounted CodeZ pane. Project switch and close assemble the live editor snapshot rather than relying solely on the debounced editor patch. Missing getter fails preflight; the helper clones snapshots, refuses dirty buffers, checks current route/snapshot across confirmation and successful persistence, and returns blocked on persistence failure. App checks ready before terminal destruction, scope revocation and route mutation. Busy mutex guards ordinary navigation; picker acquires the same mutex before awaiting native scope selection and holds it through adoption/rejected-scope cleanup, with the navigationOwned inner switch leaving release to the picker finally.

App rechecks current project and complete snapshot after terminal destruction and scope revocation before committing the route. These checks reject late edits and route changes; they cannot undo terminal cleanup or a completed revocation if subsequent checks fail. Cleanup failure is caught and prevents route success. openProjectInCodeZ only changes mode if awaited switchProject succeeds, including same-project success. Same normalized project returns early without a destructive switch. Scope acquisition/revocation calls use opaque existing service handles; this wiring does not grant authority from a path or demonstrate native lifecycle acceptance.

Historical workspace restoration attempts host restoreProjectScope for local projects. Failed scope restoration can retain editing restoration but does not construct a verified durable-plan authority in App; authority-gated actions remain responsible for rejecting unavailable scopes. Existing running-count close confirmation queries chat_running_count, cancels when requested, persists, then acknowledges close; that observation is not proof of complete process-tree cancellation or stable atomic Run state. These retained limitations are not newly certified by source inspection.

## Current relevant translation keys

English and Chinese app.saveBeforeProjectSwitch/Close require saving or closing edited files and unsaved drafts. projectSwitchSaveFailed/projectCloseSaveFailed explicitly describe cancelled navigation on workspace-preservation failure. App only displays these after corresponding actual blocked reasons; strings do not bypass guards or turn a failed operation into success.

agent.isolateHint in both tables separates Git worktree/branch editing isolation from independently controlled system-tool permissions. taskEnvironmentMissing explicitly identifies historical tasks without verified isolated binding, preserves history and requires a new task or explicit original-workspace recovery instead of silent directory movement. isolateOn/Off are configuration labels, not permission receipts or execution evidence. Relevant strings are plain data; no native execution or scope authority is created by their wording. The remainder of translation tables and localization layout are outside this review.

## Limited conclusion

No additional blocking defect was identified in the current inspected AZ017/AZ036 navigation, opaque-scope integration and relevant explanation-key consumers. Root may atomically refresh all affected consumers to these exact current hashes, preserving historical reviews and appending this evidence with the separate current index review. Any subsequent source change invalidates this conclusion's fingerprint. Full App equivalence, all Run observation/authorization semantics, actual native permission tests, whole-table translation equality, desktop roundtrip, model, rollback and full milestone acceptance are not established here. Previously documented partial cleanup/revocation effects and backend authority limitations remain open rather than being counted complete.


## Preserved review: docs/upgrade/evidence/AZ-048-appdata-independent-review.md

# AZ048 application-data domain independent review

2026-10-11. Reviewer: Codex current_source_review. Read-only current projectless.rs inspection; no code/ledger/pinned evidence edits and no Cargo/native/GUI execution. Current module raw SHA256 ddbf3bde9db44e44285fecb679a90e43e55100983a2eee4529cd4bc0e84ae5a8. Root reports previous three-test revision session99869 passed; final directed session49746 was running at request. No final result is inferred from previous tests or current fixture presence.

AppDataStore selects a fixed projectless/workspaces root and projectless/catalog.sqlite beneath caller-supplied app_data. initialize requires existing ordinary app_data, provisions only the two dedicated children via create_dir (no recursive user-tree initialization), then checks database file before Catalog initialization. Existing child paths must be ordinary directories. Existing database must be regular, nonsymlink and on Windows not a reparse point. open_existing checks existing app_data/base/root and database, does not provision missing roots and opens catalog without SQLite CREATE. Task creation/reopen delegate to the previously reviewed persistent Catalog, not marker-only adoption.

The new actual filesystem fixture verifies fixed layout, artifact preservation across dropped/reopened store, Git absence, refusal to recreate a missing moved-away catalog on reopen and refusal of a directory at the DB location without deleting retained reports. It does not dynamically test symlink/reparse/hardlink replacement or interrupted provisioning. Explicit initialize can provision missing subdirectories/new catalog even when older orphan files remain; it does not reconstruct previous registrations or recover historical identity. Host must deliberately distinguish first initialization from recovery.

reopen_task now holds a SQLite transaction across validate_catalog, registered-version lookup and existing directory/marker validation, then commits. This closes the earlier split-autocommit catalog observation gap: schema/root identity and registration are read under one SQLite snapshot/locking context. It does not freeze the filesystem or lock later uses of the returned path. create_task remains one transaction from schema validation through directory creation/INSERT/commit with retained orphan files on failure.

The corruption fixture installs a real AFTER INSERT registration-erasure trigger and checks create_task rejects before any task directory appears, plus open_existing rejection. It then tests unsupported identity schema and altered registration table layout, again verifying pre-create refusal and open rejection. These are meaningful actual SQLite fault fixtures, not mirrored validators. No same-handle filesystem race, concurrent schema-lock, commit-failure or full DB integrity test is provided.

Database path symlink/reparse rejection is specific to AppDataStore's wrapper; public Catalog API still accepts an arbitrary trusted database path. app_data authenticity is caller responsibility: API does not derive the OS app-data directory, set ACLs, authorize a renderer, reject hardlinks or verify ancestor components. SQLite sidecar file identity, file metadata-to-open races, root swap/create/read TOCTOU, marker hardlinks/unbounded substituted read, directory durability and fs+DB atomicity remain unfinished. Provisioning failures may retain partially created roots/catalog files; errors are not rollback evidence. Strict DDL/schema validation is supported-layout refusal rather than a promise of hostile SQLite/filesystem safety.

No additional blocking source issue found within the bounded host-selected trusted-appdata/cooperative-root domain API, contingent on owner's exact final verification. This is not yet Tauri host/UI/session integration, projectless task routing, native scope authorization or full AZ048 acceptance. Existing historical findings/fixes and these limits must remain attached to any candidate fingerprint refresh.


## Preserved review: docs/upgrade/evidence/AZ-048-host-independent-review.md

# AZ048 fixed-appdata host adapter independent review

2026-10-11. Reviewer: Codex current_source_review. Read-only review of the two new projectless commands and invoke registrations, with supporting current domain fingerprint. No code/ledger/pinned evidence edits, Cargo or native invoke/GUI execution. Root reports domain session49746 four tests passed; host check47282 was running at request and is not counted passed.

Current raw SHA256:
- src-tauri/src/commands/project_scope.rs: 32a6e004386212e354f5c5115b28243d6676435314bc7b6c4af1c6809b8f673b
- src-tauri/src/lib.rs: 4243052cf2eef1bd3ee40f00f5f5e13636d78c320312f80e0c5a31ff2ed7b9fd
- crates/agentz-task-state/src/projectless.rs: ddbf3bde9db44e44285fecb679a90e43e55100983a2eee4529cd4bc0e84ae5a8

Both commands call require_main before filesystem work. App data derives from Tauri app.path().app_data_dir, not any renderer path/config parameter. create accepts no directory argument, generates the UUID natively and performs provisioning plus catalog registration on spawn_blocking. reopen accepts only directory_id, enters existing fixed AppDataStore and requires catalog registration plus UUID/path/marker validation; it does not implicitly initialize missing roots or create missing tasks. Join and domain errors propagate.

After blocking work succeeds, existing mint records a main/MAIN_GENERATION VerifiedProjectScope with TrustedGuiOpen provenance and returns its opaque handle/project path. The renderer cannot select an arbitrary directory through these commands. Mint failure after creation returns an explicit retained directory ID/error and does not delete registered artifacts. This is directory opening authority, not a chat session binding, Run permission, execution grant or receipt of task acceptance. Later actions still require their own trusted task/environment observation. Scope mint occurs after path validation and remains subject to filesystem replacement TOCTOU; domain trusted appdata/cooperative root assumptions are unchanged.

lib invoke list adds projectless_directory_create/reopen beside existing project-scope commands. These registrations do not invoke the functions at startup or add implicit filesystem initialization. Existing scope lookup/with_verified_scope and picker/restore code inspected retains its distinct native-source/opaque-handle contract. Exact historical dirty source bytes are unavailable; this limited inspection is not proof that every other byte/function in either shared file is equivalent to previous reviewed versions. Upstream fingerprint refresh must preserve their prior bounded histories and attach this current adapter review, not silently treat old hashes as valid.

Material limitations: create initializes when catalog is missing, so host/UI must distinguish explicit first creation from recovery and retain identifiers on uncertain transport/mint failures; blindly retrying creates another directory. No session-to-directory mapping, list/recovery UI, archival/deletion, quotas, idempotent request key or task lifecycle cleanup is supplied. Concurrent first initialization/create may return SQLite locking/schema errors and can retain partial roots/orphans; no exactly-once or rollback guarantee. Main-window label check is not proof of a human gesture or full renderer compromise resistance. Parent ancestor paths, hardlinks, SQLite sidecars and filesystem metadata-to-use races retain prior domain trust limits. create_dir_all of Tauri-selected appdata adds provisioning but is not hostile ancestor defense.

No additional blocking source issue found in the limited fixed-appdata/main-window/catalog-only directory-open adapter. It may be recorded after exact host compile verification with the current domain evidence. Domain filesystem tests do not instantiate Tauri AppHandle/window/state or prove actual command authorization, scope mint and native invoke behavior. Full host/UI/session integration and AZ048 acceptance remain unfinished.


## Preserved review docs/upgrade/evidence/AZ-048-client-independent-review.md

# AZ048 projectless client independent review

2026-10-11. Reviewer: Codex current_source_review. Read-only inspection of current projectScope service and its three additional projectless fixtures, plus existing cache consumers. No source/ledger/pinned evidence edit and no tests/typecheck/native invoke/GUI executed by reviewer. Root reports host check47282 exit0 in6m24s. Client Vitest/full TypeScript16951 was running at request and is not asserted passed.

Raw SHA256:
- src/services/tauri/projectScope.ts: cf160f63ff90344a41363c234e869d8f19d388ffbc6a7221d04d2413d310128f
- src/services/tauri/projectScope.test.ts: 62eb3447bea034109299209a7c59f7f0b79e465b982c1346903acbbff1f308b5

reopen validates the entire lowercase hyphenated UUID-shaped string before invoke, supplies only directoryId, and compares returned directory_id to requested identity before remember can cache authority. Invalid/mismatched IDs reject before scope publication. create invokes the fixed command without renderer path/config/window/generation arguments. Both propagate native/validation failures without fallback, retry or replacement creation. UUID syntax does not prove registered existence, task ownership or UUID version; native catalog/authority remains required.

rememberDirectory validates object/identity/scope presence then delegates scope validation to existing remember. The returned wrapper and nested cached scope are frozen; identical path/handle reuses stable existing scope object and replacement publishes observable new scope. Existing opaque cache, subscriptions, restore source validation and forget-before-native revoke remain present. No handle persistence, prompting or execution grant is added. Validation checks required fields/types rather than forbidding every unknown key or JS prototype/getter; expected input is ordinary Tauri deserialized DTO, not arbitrary executable objects.

Three new fixtures invoke real exported service with native invoke mocked: creation and ID-only reopening cache the returned scope; invalid IDs/mismatch/invalid scope produce no cache publication; missing-directory rejection issues one reopen and no create. Existing fixtures still exercise immutable snapshot identity, observable replacement/revocation, known restore sources and authorization-error propagation. Tests do not run real Tauri command authorization, filesystem catalog, scope mint or UI/session lifecycle. Synthetic paths/handles are controlled inputs rather than authority proof.

Remaining integration limits: no directory_id-to-chat-session persistence or restore policy, UI invocation, transport-uncertain create reconciliation, stale concurrent acquire/revoke generation, host scope reclamation or complete projectless journey. A failed create response after native success can retain an unreturned registered directory; blindly retrying is not deduplicated. Cache validation does not cryptographically bind path to ID; it relies on trusted native result. Earlier global scope limits and current host/domain filesystem trust constraints remain.

No additional blocking source issue found in this limited projectless client/opaque cache integration. Exact-source owner checks are required before fingerprint registration. Shared consumer refresh must preserve prior scopes/review history; missing historical dirty bytes prevents asserting full old-source equivalence or authorship. Full AZ048 native/UI/session/rollback acceptance remains unfinished.


## Preserved review docs/upgrade/evidence/AZ-048-host-composite-review.md

# AZ048 bounded host directory candidate

Preserves all earlier consumer scopes and review history. New scope is only main-window fixed Tauri AppData create/reopen and two additive command registrations; no session/UI/archive, native invocation, model or release acceptance.

Actual domain session49746: offline locked 4 passed, 0 failed, 141 filtered; 21.09s compile, 0.20s tests. Earlier99869 3 passed. Current desktop check47282 pending; no success inferred.

## Preserved review: docs/upgrade/evidence/AZ-048-shared-export-review.md

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


# AZ040 summary/Advanced independent implementation review

2026-10-10. Read-only review of InputContextSummary and the added CodeZ/WorkZ JSX integration. Existing unrelated repository diff was not attributed to this slice. No independent tests/build/models/GUI run.

The shared summary is presentational: props, translated strings and native details only; no IPC, tool execution, permission response, state mutation or scope minting. Unknown/empty strings remain Unknown, optional counts reject negative/unsafe values. Policy explicitly says configured policy does not grant permission. Workspace is explicitly configured context rather than host-verified environment. Queued text explicitly says “Queued locally”/“本地待发”; it is not persisted Run state. The approval display is a typed possibility but neither current caller supplies it, avoiding inference from stale permissionRequest.

CodeZ adds the summary immediately above its existing composer. Authority and workspace are the configured projectDir. Model uses current selected ID or loaded configured default label; the ID is a selection identifier, not a provider-confirmed effective model. Settings-loading failure falls back to Unknown. Busy displays Running, then existing error, then local queuedView presence, otherwise Idle. Busy takes precedence over pending local messages, so this slice is not an exhaustive queue dashboard. Existing Send/Stop and composer fields remain unchanged by the summary. CodeZ optional skills/connectors count is not yet supplied; their existing controls remain visible and functional, so no hidden-option behavior is introduced by this slice.

WorkZ only displays worktree_path when worktree.id matches sessionRef.current; otherwise the workspace fallback is configured free cwd/direct project, or Unknown for unverified isolated/agent/team contexts. Hidden model selector forces model/source Unknown and does not display the remembered scoped ID or global default. Mode labels separate agent/team/plain; policy remains configuration. Summary counts existing selected optional skills/connectors only when generic controls apply. No stale permission card is converted to waiting state, and no local queue is fabricated for WorkZ.

Native Advanced details keeps its children mounted while collapsed. Team/persona selectors retain original values, callbacks and taskBound/busy disabling. Free cwd and isolation remain outside the collapsed controls. The new start-source/base controls are scoped to isolate && !isFree && !taskBound and busy-disabled, feeding already existing startMode/startBase state consumed by task creation. The default remains working; no automatic switching or selection clearing was added. This adds access to existing configurable inputs rather than changing the send algorithm. Keyboard/focus, overlay placement within collapsed details, layout clipping and native desktop interaction still require GUI acceptance.

No blocking source defect was found in this bounded descriptive-summary/Advanced slice. It does not close budget presentation, permission receipt ownership, verified Run/Attempt live state, five-user first-use evaluation or full AZ040 acceptance. Selected model IDs are intentionally descriptive configured identifiers; future friendly-label resolution must preserve Unknown for missing provider definitions.

Reviewed SHA256:
- src/components/chat/InputContextSummary.tsx: 10ab6a5753f3874655cb2c9636c4b146afc0d8b63f68b3e024ab0f282e1c5847
- src/workspaces/codez/AssistantPanel.tsx: 7c82b46a98d816b34f4ab0ffedbff22f48fa6eaee0010cd9bfcaae25d82a7598
- src/workspaces/workz/index.tsx: e11099d392fe3184eb539db9b2c8b612b1b52af18ac12262c68a9c90ba51f468


# AZ040 observed-budget presentation independent review

2026-10-10. Read-only source/fixture review; no tests, Cargo, models or GUI executed by reviewer. Root reports author BudgetObservation seven and TaskDetailsPanel five targeted tests passed; final combined validation remains root-owned.

BudgetObservation renders totals only for an available typed source. Missing/unknown/stale/unavailable/unsupported sources have explicit status text and no numeric totals. Available known zero is preserved as an observation, not substituted for absent values; null policy maxima use Not configured via nullish checks. Known input/output, known active duration and usage count are distinct from unknown token/duration/pricing record counts. The fixtures cover each unavailable source, mixed known/unknown records, real known zero and absent cost observations.

Costs remain separate currency buckets in micro-units; no conversion, invoice claim, subtraction from maxima, remaining balance or token-to-price estimate is introduced. Empty cost map says No known cost observations. Configured maxima and observed exceedances are labelled observational; the component explicitly says enforcement is not connected, known totals may be incomplete and maxima are not an enforced allowance. It does not call the reservation API or authorize execution. Data validity is supplied by the existing strict taskDetails DTO validator; this component is not a replacement trust boundary.

TaskDetailsPanel change is an import plus replacement of its old budget paragraph with BudgetObservation using data.sources.budget. Existing scope/project/session request key masks old data during render before effect; generation/disposed checks suppress late responses, focus coalescing and error clearing remain intact. Therefore no old budget is exposed through the new child on a task/scope switch. Sources-read-separately/non-atomic disclaimer remains displayed. No polling or model continuation was added.

No blocking defect found within this observational presentation slice. Host accounting completeness, prices/invoices, reservation enforcement, team/member coverage and GUI usability remain unverified and are not inferred from these fixtures.

Reviewed SHA256:
- src/workspaces/codez/BudgetObservation.tsx: 567d6defee4616872f05a7995420e52d0cafdddc0b0a8ec24db8ad001878fa7c
- src/workspaces/codez/BudgetObservation.test.tsx: db01fad4a7d937dd0009db77a80a4499c501030b5112616e70c78ea20abcf22f
- src/workspaces/codez/TaskDetailsPanel.tsx: 8b925d75fb02697050ad3bc910999dfe5348cdf88a209bf3ee7b0f1bd9ad1a6d


# AZ039 bounded delivery observations independent implementation review

2026-10-10. Read-only review of four new summary/wrapper files. No tests/typecheck/build/GUI/model/external operations run by reviewer. WorkZ root mounting has not yet been included in this review. Author reports previous five-summary/four-wrapper fixtures passed; subsequent delivery fix requires root final validation rather than reuse of old results.

Found and resolved during review: initial summary ignored run.delivery and labelled all Acceptance as no receipt. Final source now separately displays authoritative Run delivery accepted/rejected/awaiting_review/not_ready, with added accepted/rejected fixture, while stating that a Run decision does not establish acceptance of a specific artifact version. Artifact-version acceptance, apply and publication still explicitly lack receipts. Full execution enums are shown independently; succeeded never implies validation pass or publication.

Artifacts are counted as distinct IDs and retained versions on this returned page only, with more-available marker. Validation kind counts recorded attachments and explicitly does not establish passed validation/acceptance. Missing store, unavailable/unknown/stale source remain labelled rather than fake zero-success; known empty available page can correctly count zero. No payload/tool text is interpreted as a verdict. Buttons only invoke view callbacks; no accept/apply/PR/merge/model action is introduced. Typed props rely on strict existing DTO services for runtime enum/scope validation, not untrusted raw JSON rendered directly.

DeliveryReviewPanel uses opaque useProjectScope handle, scoped readTaskDetails and fresh observeTaskView before bounded listTaskArtifacts(null,200). Separate sources use allSettled: one failure preserves another valid source but marks failing evidence unavailable rather than empty/pass. Independent project/task/environment mismatch marks artifacts stale. It does not claim binding revisions or separately read sources are atomic. No continuous polling or cursor synthesis occurs.

Render key project/session/handle masks old data before effect; generation cleanup discards late replies and suppresses list invocation when observation outlives its generation. Source loading resets previous values; scope missing prevents reads. Fixtures exercise bounded fresh observations, failed list without leaking private error, late prior-task responses, cross-environment stale and no-scope. Run details may be available while artifacts fail, which is intentionally distinct rather than global successful delivery. Manual refresh remains observational and no active chat projection is rewritten.

No blocking defect remains within this bounded summary/wrapper contract. Exact-version acceptance receipts, complete multi-page artifact enumeration, stable cross-source snapshot, old openPr safety, native GUI and full AZ039 acceptance remain outstanding. This candidate may expose existing review/artifact viewers but does not itself implement their authorization or external-delivery semantics.

Reviewed SHA256:
- src/workspaces/codez/DeliverySummary.tsx: 504f8776ef6ccc20c18b7d6dd84c0f263f299452336cafbb236d1c05b6cbb4df
- src/workspaces/codez/DeliverySummary.test.tsx: f06f3adec81df0e7e2eb851b1b0420752365c10084e3252db59ca0b8f3b6776c
- src/workspaces/codez/DeliveryReviewPanel.tsx: 237c6d2c2310fda3379729fc29acbbbf3f401b2294cceb5d37949ab6c8681458
- src/workspaces/codez/DeliveryReviewPanel.test.tsx: cd5d1f9a732a9e10909e1c6c33ceb123d81f4ce8a67d530be59414e3040b87f0


# AZ039 two-workspace wiring independent review

2026-10-10. Read-only inspection of root integration; no tests/typecheck/GUI/model or Git operations executed. The four new component files are covered separately by AZ-039-implementation-review.md, left unchanged.

CodeZ mounts DeliveryReviewPanel only with a current project and session, using those inputs for the scope-aware observer. Open artifacts sets a project/session identity key; TaskArtifactViewer mounts only when that key matches the current pair and is React-keyed by it. Root added key clearing on project reset, newSession and switchSession start, preventing an existing artifact preview surviving asynchronous selection of another session. No review/merge/PR callback is attached to CodeZ; its existing composer/stream handling is otherwise unchanged by this addition.

WorkZ reuses the existing non-team/non-pool, selected-project/canonical-project/current-session gate. It passes canonical authority to DeliveryReviewPanel. Review changes is present only when not busy and worktree.id matches sessionRef.current, and only opens the existing review UI; no merge/discard/PR runs automatically. Open artifacts uses the same canonical-project/session key as the existing viewer. Old preview clearing at openTask async start remains present. Changing the project/session prevents mismatched-key viewer mounting; child scope generation validation remains the actual asynchronous-response isolation. Neither button issues native permission or changes selected project/task.

The WorkZ review button is UI eligibility, not a host authorization or proof the task remains idle until a later review action. Existing AgentTaskReview/openPr limitations recorded under AZ013 remain unchanged. Delivery observations refresh manually and do not constitute an automatically live completion notification. Artifact viewer has explicit version capture/delete controls from AZ038; merely opening it does not mutate. Native GUI layout/focus and combined tests/typecheck are root-owned outstanding checks, not asserted here.

No blocking defect found in these added mounting/key/reset lines within the limited contract.

Reviewed SHA256:
- src/workspaces/codez/AssistantPanel.tsx: ab469eb32e479f537b7819a2e95311821398f091c6243a8393a2dfc755874673
- src/workspaces/workz/index.tsx: 472d401fd2bf8b543162f2ffe30a00b4e7aab145fd94a81a224d9b68a20cb866


# AZ041 scoped editor follow-up

# AZ041 editor preservation candidate independent review

2026-10-10. Independent read-only source review plus four real exported-helper/serializer integration fixtures. No Cargo, model, native filesystem races, Monaco mount or GUI acceptance run. Two defects found during review were corrected before this conclusion; no existing candidate installer evidence was changed.

EditorConflictTracker captures project/scope generation, tab identity, monotonic local revision, request generation, content and dirty state. Close/reopen/project ABA and same-content edits invalidate old operations. Confirmed reload consent belongs to the captured dirty revision; later edits cannot be discarded by that request. Failed/deleted reads consume their operation but preserve buffer/dirty state. Successful save only marks its exact captured current revision clean and invalidates pending reads. These are in-memory settlement guarantees, not disk-write CAS.

The central setTabs wrapper synchronously updates tracker and tabsRef before passing a concrete array to setTabsState. It evaluates its callback itself rather than giving React a side-effecting functional state updater, so StrictMode replay of React updater functions does not repeat tracker effects. handleEditorChange reports every edit before state publication, including same-content edits. The fixed setter also reports clean-to-dirty transitions even with equal text. Project-scope refs are updated during render: this is conservative operation invalidation, but an aborted concurrent render/real StrictMode/Monaco journey has not been exercised; this review does not claim general concurrent-render transactional correctness.

Dirty watcher deletion now retains the tab with visible explanatory error; accepted dirty reload no longer clears dirty before IO. Read/save replies require tracker settlement before applying React changes. Binary/read failures retain drafts. Save-before-close only closes after a successful current save with unchanged tab identity and clean state; bulk close checks current identities/dirty state and generation, preserving failed saves, newer edits and reopened tabs. Readonly/recovery save is refused. Dirty deleted buffers remain in the existing dirty-buffer snapshot; they are not a proven new durable recovery database.

Found and fixed hot-restore risk: an earlier restored draft formerly replaced any current same-path clean tab, including a user-saved or reopened tab during pending restore. mergeRestoredTabs now always preserves current same-path intent and stores the older dirty draft as an independent reserved recovery tab, including identical-content dirty drafts. Recovery tabs have unique IDs, readonly dirty text and original-path provenance. Real workspace serializer retains their open path and dirty text; restoration recognizes them before any disk read. Dirty persisted buffers are also preserved when the actual disk file becomes binary or unreadable. A restored buffer's disk base remains unknown; no three-way merge or checked-write proof is invented.

Index guards reserved paths out of open/read/reload/watch/diff/reveal, file-tree focus, editor mutation, save and Monaco URI management. Recovery rendering uses a dedicated readonly textarea/copy UI, not FileViewer/LSP. Explicit close uses discard-only wording, not a fake save; bulk dirty checks conservatively retain recovery drafts. Helpers themselves perform no native IPC and never infer disk content. Existing navigation snapshot registration remains, now reading synchronous tabsRef; opaque permission/scope APIs are unchanged. The shared existing AZ036 index fingerprint must be refreshed after review, not silently treated as unchanged.

Actual independent verification: editorRecovery.integration.test.ts four tests passed, final session59314 exit0, 14ms tests/4.10s total. They consume the real tracker (completed save and close/reopen), real merge policy, and real editorSnapshotFromTabs/restoreRecoveryTab, proving current clean intent survives, old draft remains, same-content dirty draft is not swallowed, persisted recovery round-trip retains readonly dirty text, and zero invoke calls. Earlier same target session3327 also passed; own-file ESLint83947 exit0. They do not mount the index or assert its wiring through fake copies. Final tracker test file contains twelve cases: three added fixtures were independently read and verify clean-to-dirty same-content invalidates pending read, successful read/save mutually invalidate captured operations, and old failures cannot clear newer dirty state. Root reports final session43548 tracker12 + recovery4 + navigation6 = twenty-two tests passed, exit0/4.36s. These combined results and source lint/typecheck are root-owned evidence, not our reruns.

No remaining blocking defect found within this local-buffer preservation/restore slice. Host ide_write_file still lacks expected disk revision CAS: parallel saves or external/Agent writes may race and an already sent old write cannot be rolled back by frontend stale settlement. All-write-actor concurrency, durable restart OS verification, explicit conflict comparison/merge, complete accessibility/localized recovery UI and real GUI journeys remain outstanding. Candidate protection is suitable for limited implementation consumption; full AZ041 and stable concurrent-editor acceptance are not declared complete.

Reviewed SHA256:
- src/workspaces/codez/editorConflict.ts: 18913faf070f34c48c1ffef4496aed0cba018f1f2b34400a7f0143e6beadf8a0
- src/workspaces/codez/editorConflict.test.ts: 1a186cb366b939c12b7dc9ac7434ffcb1d44515092aa2fcc8a714b104d913695
- src/workspaces/codez/editorRecovery.ts: 25ff29527254b07dd159f2952902557ac56c0d645a43beae8bf3f7cb030d7968
- src/workspaces/codez/editorRecovery.integration.test.ts: b252966ab7da5f4367501cd14e3123be65653709c54ca86d9a30f2b86be92446
- src/workspaces/codez/index.tsx: 4885b81cc033d4a9499f41088255f58524a74aec719fd2f1eea85b7792e96785


# Limited current translation consumer review

# AZ041 current i18n drift and limited consumer review

2026-10-10. Read-only investigation; neither translation file was edited. Existing old review/pin history must remain. This is a current limited-consumer review, not a claim that the full translation diff is semantically unchanged or authored by this task.

Old pins: en bb8eb0541b49ed46198f7e60ffaa313dcb13272d286f9105f02b451e949d5184; zh 04ff174670e9123ca34274447d7b254662a8b1189920f059ac315df5f6772a65. Current files contain LF only, no CR/BOM (en 1204 LF/51249 bytes, zh 1157 LF/51268 bytes). Normalizing current bytes to LF yields the current raw hashes below. Converting uniformly to CRLF yields en a9c0c61543b27e87843edd487e2bc60cd67e727d1f9de51866995603355456c9 and zh 1febc7baa944b336e6a2ed425cd173f463c2090f1030d217e8ce05f77313ad98: neither matches the old pins. Git HEAD hashes also differ from both old and current pins. Therefore simple uniform newline conversion does not explain the drift. Without old exact bytes, mixed historical line endings/formatting/semantic changes cannot be conclusively attributed or ruled out; comparing with HEAD alone is not a baseline comparison.

Current AZ036 consuming scope independently reviewed: app.saveBeforeProjectSwitch/Close and projectSwitchSaveFailed/projectCloseSaveFailed in both languages ask for saving edited buffers and state cancelled navigation on preservation failure. App uses these as explanatory text after actual dirty/failure preflight decisions, not as permission or state transitions; translations do not remove those guards. These are plain strings, not execution logic. AZ041 current ide.unsavedConfirm, unsavedBulkConfirm and externalChangeConflict clearly distinguish save-before-close from discarding local edits to reload disk. Name/count placeholders remain present in both languages. They do not grant file-write atomicity or fabricate a successful save.

Current AZ017 consuming scope independently reviewed: agent.isolateHint explicitly distinguishes Git worktree/branch isolation from separately controlled system-tool permissions in English and Chinese. agent.taskEnvironmentMissing states the historical task lacks a verified isolated binding, keeps history, requires explicit workspace recovery/new task and refuses silent directory movement. isolateOn/Off labels describe configuration only. These messages do not themselves authorize execution, broaden a filesystem sandbox, resolve a missing binding or turn a configured policy into an approval. Actual host/task identity and permission logic remain outside these files.

Limited conclusion: current relevant navigation/editor and task-environment texts support the existing fail-closed contracts and may be independently pinned for those specific consumers after root validation. The rest of both translation tables, full semantic equality to previous pins, origin of drift and GUI localization/overflow are not established by this review. No automatic ledger refresh, source restoration or stable-release claim is made here.

Current raw SHA256:
- src/i18n/en.ts: c718a52cd27c3b1f6e2dc0950fd6c4d3992ab6af9ae8d6b5837592670d69493e
- src/i18n/zh.ts: bbca86c830405bc983d8297bf000d34dfeea9975f85e6693ac22f2be739f8984


# AZ044 current CodeZ index limited independent review

2026-10-10. Reviewer: Codex current_source_review, independent read-only source inspection. No applicable AGENTS.md was found in repository or C:/Projects/AgentZ, C:/Projects, C:/ parents. Read AZ-041-shared-review.md and AZ-044-shared-source-drift.md before inspecting the current navigation/editor consumers. Only this new evidence file was written; no source, ledger or existing pinned evidence was edited. No tests, typecheck, native IPC, GUI or model evaluation was run by this reviewer.

Current raw SHA256 of src/workspaces/codez/index.tsx: 306ad89024ce5071bdc6d2c678f969d3fb0e8e8b40cf645555802d3a8997f84e. Historical reviewed SHA256: 4885b81cc033d4a9499f41088255f58524a74aec719fd2f1eea85b7792e96785. The exact historical reviewed dirty bytes are unavailable. This review establishes current limited behavior, not precise historical delta, semantic equivalence of the whole file, authorship of changes or a newline-only explanation. Git HEAD is not that missing baseline.

## AZ036 navigation snapshot and project identity

The registered navigation getter synchronously reads tabsRef and activeTabPathRef through editorSnapshotFromTabs; registration cleanup removes the getter. Central setTabs evaluates mutations against the live ref, synchronously reports close/open/content changes to EditorConflictTracker, updates tabMutation and tabsRef, then publishes the concrete array to React. Every editor edit reports a revision before state publication, including equal-content edits; clean-to-dirty equal-content changes are also reported by the setter. Navigation preflight can therefore consume current local buffers instead of a debounced snapshot. The App's actual preflight/persistence decisions are outside this file and are not newly re-certified here.

Current render-time scope invalidation and the reset effect both use exact project string identity. The reset effect explicitly compares prev !== projectDir, including spelling changes, rather than retaining rendered tabs after invalidating their operation ref. Scope changes advance editor generation, reset conflict scope, clear live tabs and open request ownership. Async open requests require captured generation and per-path request ownership before publishing; an existing same-path tab prevents a late open from replacing it. This is local asynchronous publication protection, not host filesystem authority or proof of all concurrent React render behavior.

## AZ041 current buffer preservation wiring

Watcher deletion retains dirty tabs with an explanatory error. Dirty reload requires explicit consent, captures the operation, and does not prematurely clear dirty state. Read replies settle tracker ownership before changing buffer text/dirty status; binary/read errors retain the buffer. Watch callbacks filter event project identity, and cleanup clears pending reload timers/operations. Manual reload similarly checks live project, excludes recovery/browser paths, and settles ownership before publishing.

saveFile refuses readonly/recovery tabs, captures current buffer identity and only clears dirty status after a matching successful settlement. A failed or stale save returns false. Save-before-close requires successful current save, matching tab identity and clean current state; bulk close also guards project generation, retains failed/newly dirty/reopened tabs and excludes recovery drafts. This still does not supply expected disk revision CAS: an already sent old write or external/Agent write may race on disk. That previously recorded limitation remains open.

Hot restore captures editor generation and mutation count; a changed project rejects late restoration. If local intent changed, mergeRestoredTabs preserves current same-path tabs and retains older dirty drafts independently. Reserved recovery paths are recognized before native reads, and persisted dirty text survives binary/unreadable disk results. Recovery content is rendered in a readonly textarea with copy action, outside FileViewer/Monaco/LSP; reserved paths are excluded from editor mutation, save, watcher/reload, file tree focus and model URI management. Explicit recovery close states discard-only; bulk close retains drafts. Native durable restart, explicit compare/merge and full localization/accessibility remain unverified.

## AZ044 mount change boundary and conclusion

BottomPanel is mounted while projectDir exists, with open controlling visibility rather than conditional mounting. This current wiring does not remove the navigation getter, operation generation or editor preservation guards inspected above. It is not independent certification of PTY ownership, process cleanup or terminal tests; those belong to AZ044's separate lifecycle review.

No additional blocking defect was found in the bounded AZ036 current snapshot/project-reset and AZ041 local editor settlement/recovery wiring inspected here. Root may refresh the current fingerprint for all affected ledger consumers in one validated operation, preserving previous review history and appending this limited current review. This is conditional on the file retaining the stated SHA256 and on retaining separate existing scopes/reviews; it is not permission to blindly refresh unrelated source fingerprints or claim full AZ036/AZ041 completion, desktop acceptance or stable release readiness.


# AZ044 current App and translation limited independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only independent inspection of current src/App.tsx, src/i18n/en.ts and src/i18n/zh.ts in the existing bounded AZ017/AZ036 navigation and task-environment explanation scopes. Read AZ-044-current-drift.json, previous AZ-041-shared-review.md and relevant AZ036/AZ017 review history. Applicable AGENTS search from this review sequence found none. Only this new evidence file is written; no source, ledger or existing pinned evidence is changed. No tests, typecheck, native IPC, GUI or model execution was run by this reviewer.

Current raw SHA256:
- src/App.tsx: 81d99ad3755dfaff43bd74343ba2bf0cc21bf2e9df5960358745646bba78f471
- src/i18n/en.ts: 95c75f1c919cf60a40217959be3d4710968b3329ddb4aaa6de80d9f578ec893b
- src/i18n/zh.ts: 14fdeba7b7caedb92907893010532009b716a5b94e6b30fa200a83b2027e3e29

Previous pins are App 79fd684ddaad63cb573db8740358aff68a3a08a818f573325a8d41361db2f465, en c718a52cd27c3b1f6e2dc0950fd6c4d3992ab6af9ae8d6b5837592670d69493e and zh bbca86c830405bc983d8297bf000d34dfeea9975f85e6693ac22f2be739f8984. Exact reviewed dirty baseline bytes are unavailable. No historical semantic equivalence, exact delta, authorship or newline-only explanation is claimed. This is a fresh limited current-consumer inspection.

## Current navigation guards

App stores a synchronous live editor getter and passes its stable registration callback into the continuously mounted CodeZ pane. Project switch and close assemble the live editor snapshot rather than relying solely on the debounced editor patch. Missing getter fails preflight; the helper clones snapshots, refuses dirty buffers, checks current route/snapshot across confirmation and successful persistence, and returns blocked on persistence failure. App checks ready before terminal destruction, scope revocation and route mutation. Busy mutex guards ordinary navigation; picker acquires the same mutex before awaiting native scope selection and holds it through adoption/rejected-scope cleanup, with the navigationOwned inner switch leaving release to the picker finally.

App rechecks current project and complete snapshot after terminal destruction and scope revocation before committing the route. These checks reject late edits and route changes; they cannot undo terminal cleanup or a completed revocation if subsequent checks fail. Cleanup failure is caught and prevents route success. openProjectInCodeZ only changes mode if awaited switchProject succeeds, including same-project success. Same normalized project returns early without a destructive switch. Scope acquisition/revocation calls use opaque existing service handles; this wiring does not grant authority from a path or demonstrate native lifecycle acceptance.

Historical workspace restoration attempts host restoreProjectScope for local projects. Failed scope restoration can retain editing restoration but does not construct a verified durable-plan authority in App; authority-gated actions remain responsible for rejecting unavailable scopes. Existing running-count close confirmation queries chat_running_count, cancels when requested, persists, then acknowledges close; that observation is not proof of complete process-tree cancellation or stable atomic Run state. These retained limitations are not newly certified by source inspection.

## Current relevant translation keys

English and Chinese app.saveBeforeProjectSwitch/Close require saving or closing edited files and unsaved drafts. projectSwitchSaveFailed/projectCloseSaveFailed explicitly describe cancelled navigation on workspace-preservation failure. App only displays these after corresponding actual blocked reasons; strings do not bypass guards or turn a failed operation into success.

agent.isolateHint in both tables separates Git worktree/branch editing isolation from independently controlled system-tool permissions. taskEnvironmentMissing explicitly identifies historical tasks without verified isolated binding, preserves history and requires a new task or explicit original-workspace recovery instead of silent directory movement. isolateOn/Off are configuration labels, not permission receipts or execution evidence. Relevant strings are plain data; no native execution or scope authority is created by their wording. The remainder of translation tables and localization layout are outside this review.

## Limited conclusion

No additional blocking defect was identified in the current inspected AZ017/AZ036 navigation, opaque-scope integration and relevant explanation-key consumers. Root may atomically refresh all affected consumers to these exact current hashes, preserving historical reviews and appending this evidence with the separate current index review. Any subsequent source change invalidates this conclusion's fingerprint. Full App equivalence, all Run observation/authorization semantics, actual native permission tests, whole-table translation equality, desktop roundtrip, model, rollback and full milestone acceptance are not established here. Previously documented partial cleanup/revocation effects and backend authority limitations remain open rather than being counted complete.


# AZ038 shared-source independent reviews

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


# AZ040 summary/Advanced independent implementation review

2026-10-10. Read-only review of InputContextSummary and the added CodeZ/WorkZ JSX integration. Existing unrelated repository diff was not attributed to this slice. No independent tests/build/models/GUI run.

The shared summary is presentational: props, translated strings and native details only; no IPC, tool execution, permission response, state mutation or scope minting. Unknown/empty strings remain Unknown, optional counts reject negative/unsafe values. Policy explicitly says configured policy does not grant permission. Workspace is explicitly configured context rather than host-verified environment. Queued text explicitly says “Queued locally”/“本地待发”; it is not persisted Run state. The approval display is a typed possibility but neither current caller supplies it, avoiding inference from stale permissionRequest.

CodeZ adds the summary immediately above its existing composer. Authority and workspace are the configured projectDir. Model uses current selected ID or loaded configured default label; the ID is a selection identifier, not a provider-confirmed effective model. Settings-loading failure falls back to Unknown. Busy displays Running, then existing error, then local queuedView presence, otherwise Idle. Busy takes precedence over pending local messages, so this slice is not an exhaustive queue dashboard. Existing Send/Stop and composer fields remain unchanged by the summary. CodeZ optional skills/connectors count is not yet supplied; their existing controls remain visible and functional, so no hidden-option behavior is introduced by this slice.

WorkZ only displays worktree_path when worktree.id matches sessionRef.current; otherwise the workspace fallback is configured free cwd/direct project, or Unknown for unverified isolated/agent/team contexts. Hidden model selector forces model/source Unknown and does not display the remembered scoped ID or global default. Mode labels separate agent/team/plain; policy remains configuration. Summary counts existing selected optional skills/connectors only when generic controls apply. No stale permission card is converted to waiting state, and no local queue is fabricated for WorkZ.

Native Advanced details keeps its children mounted while collapsed. Team/persona selectors retain original values, callbacks and taskBound/busy disabling. Free cwd and isolation remain outside the collapsed controls. The new start-source/base controls are scoped to isolate && !isFree && !taskBound and busy-disabled, feeding already existing startMode/startBase state consumed by task creation. The default remains working; no automatic switching or selection clearing was added. This adds access to existing configurable inputs rather than changing the send algorithm. Keyboard/focus, overlay placement within collapsed details, layout clipping and native desktop interaction still require GUI acceptance.

No blocking source defect was found in this bounded descriptive-summary/Advanced slice. It does not close budget presentation, permission receipt ownership, verified Run/Attempt live state, five-user first-use evaluation or full AZ040 acceptance. Selected model IDs are intentionally descriptive configured identifiers; future friendly-label resolution must preserve Unknown for missing provider definitions.

Reviewed SHA256:
- src/components/chat/InputContextSummary.tsx: 10ab6a5753f3874655cb2c9636c4b146afc0d8b63f68b3e024ab0f282e1c5847
- src/workspaces/codez/AssistantPanel.tsx: 7c82b46a98d816b34f4ab0ffedbff22f48fa6eaee0010cd9bfcaae25d82a7598
- src/workspaces/workz/index.tsx: e11099d392fe3184eb539db9b2c8b612b1b52af18ac12262c68a9c90ba51f468


# AZ040 observed-budget presentation independent review

2026-10-10. Read-only source/fixture review; no tests, Cargo, models or GUI executed by reviewer. Root reports author BudgetObservation seven and TaskDetailsPanel five targeted tests passed; final combined validation remains root-owned.

BudgetObservation renders totals only for an available typed source. Missing/unknown/stale/unavailable/unsupported sources have explicit status text and no numeric totals. Available known zero is preserved as an observation, not substituted for absent values; null policy maxima use Not configured via nullish checks. Known input/output, known active duration and usage count are distinct from unknown token/duration/pricing record counts. The fixtures cover each unavailable source, mixed known/unknown records, real known zero and absent cost observations.

Costs remain separate currency buckets in micro-units; no conversion, invoice claim, subtraction from maxima, remaining balance or token-to-price estimate is introduced. Empty cost map says No known cost observations. Configured maxima and observed exceedances are labelled observational; the component explicitly says enforcement is not connected, known totals may be incomplete and maxima are not an enforced allowance. It does not call the reservation API or authorize execution. Data validity is supplied by the existing strict taskDetails DTO validator; this component is not a replacement trust boundary.

TaskDetailsPanel change is an import plus replacement of its old budget paragraph with BudgetObservation using data.sources.budget. Existing scope/project/session request key masks old data during render before effect; generation/disposed checks suppress late responses, focus coalescing and error clearing remain intact. Therefore no old budget is exposed through the new child on a task/scope switch. Sources-read-separately/non-atomic disclaimer remains displayed. No polling or model continuation was added.

No blocking defect found within this observational presentation slice. Host accounting completeness, prices/invoices, reservation enforcement, team/member coverage and GUI usability remain unverified and are not inferred from these fixtures.

Reviewed SHA256:
- src/workspaces/codez/BudgetObservation.tsx: 567d6defee4616872f05a7995420e52d0cafdddc0b0a8ec24db8ad001878fa7c
- src/workspaces/codez/BudgetObservation.test.tsx: db01fad4a7d937dd0009db77a80a4499c501030b5112616e70c78ea20abcf22f
- src/workspaces/codez/TaskDetailsPanel.tsx: 8b925d75fb02697050ad3bc910999dfe5348cdf88a209bf3ee7b0f1bd9ad1a6d


# AZ039 bounded delivery observations independent implementation review

2026-10-10. Read-only review of four new summary/wrapper files. No tests/typecheck/build/GUI/model/external operations run by reviewer. WorkZ root mounting has not yet been included in this review. Author reports previous five-summary/four-wrapper fixtures passed; subsequent delivery fix requires root final validation rather than reuse of old results.

Found and resolved during review: initial summary ignored run.delivery and labelled all Acceptance as no receipt. Final source now separately displays authoritative Run delivery accepted/rejected/awaiting_review/not_ready, with added accepted/rejected fixture, while stating that a Run decision does not establish acceptance of a specific artifact version. Artifact-version acceptance, apply and publication still explicitly lack receipts. Full execution enums are shown independently; succeeded never implies validation pass or publication.

Artifacts are counted as distinct IDs and retained versions on this returned page only, with more-available marker. Validation kind counts recorded attachments and explicitly does not establish passed validation/acceptance. Missing store, unavailable/unknown/stale source remain labelled rather than fake zero-success; known empty available page can correctly count zero. No payload/tool text is interpreted as a verdict. Buttons only invoke view callbacks; no accept/apply/PR/merge/model action is introduced. Typed props rely on strict existing DTO services for runtime enum/scope validation, not untrusted raw JSON rendered directly.

DeliveryReviewPanel uses opaque useProjectScope handle, scoped readTaskDetails and fresh observeTaskView before bounded listTaskArtifacts(null,200). Separate sources use allSettled: one failure preserves another valid source but marks failing evidence unavailable rather than empty/pass. Independent project/task/environment mismatch marks artifacts stale. It does not claim binding revisions or separately read sources are atomic. No continuous polling or cursor synthesis occurs.

Render key project/session/handle masks old data before effect; generation cleanup discards late replies and suppresses list invocation when observation outlives its generation. Source loading resets previous values; scope missing prevents reads. Fixtures exercise bounded fresh observations, failed list without leaking private error, late prior-task responses, cross-environment stale and no-scope. Run details may be available while artifacts fail, which is intentionally distinct rather than global successful delivery. Manual refresh remains observational and no active chat projection is rewritten.

No blocking defect remains within this bounded summary/wrapper contract. Exact-version acceptance receipts, complete multi-page artifact enumeration, stable cross-source snapshot, old openPr safety, native GUI and full AZ039 acceptance remain outstanding. This candidate may expose existing review/artifact viewers but does not itself implement their authorization or external-delivery semantics.

Reviewed SHA256:
- src/workspaces/codez/DeliverySummary.tsx: 504f8776ef6ccc20c18b7d6dd84c0f263f299452336cafbb236d1c05b6cbb4df
- src/workspaces/codez/DeliverySummary.test.tsx: f06f3adec81df0e7e2eb851b1b0420752365c10084e3252db59ca0b8f3b6776c
- src/workspaces/codez/DeliveryReviewPanel.tsx: 237c6d2c2310fda3379729fc29acbbbf3f401b2294cceb5d37949ab6c8681458
- src/workspaces/codez/DeliveryReviewPanel.test.tsx: cd5d1f9a732a9e10909e1c6c33ceb123d81f4ce8a67d530be59414e3040b87f0


# AZ039 two-workspace wiring independent review

2026-10-10. Read-only inspection of root integration; no tests/typecheck/GUI/model or Git operations executed. The four new component files are covered separately by AZ-039-implementation-review.md, left unchanged.

CodeZ mounts DeliveryReviewPanel only with a current project and session, using those inputs for the scope-aware observer. Open artifacts sets a project/session identity key; TaskArtifactViewer mounts only when that key matches the current pair and is React-keyed by it. Root added key clearing on project reset, newSession and switchSession start, preventing an existing artifact preview surviving asynchronous selection of another session. No review/merge/PR callback is attached to CodeZ; its existing composer/stream handling is otherwise unchanged by this addition.

WorkZ reuses the existing non-team/non-pool, selected-project/canonical-project/current-session gate. It passes canonical authority to DeliveryReviewPanel. Review changes is present only when not busy and worktree.id matches sessionRef.current, and only opens the existing review UI; no merge/discard/PR runs automatically. Open artifacts uses the same canonical-project/session key as the existing viewer. Old preview clearing at openTask async start remains present. Changing the project/session prevents mismatched-key viewer mounting; child scope generation validation remains the actual asynchronous-response isolation. Neither button issues native permission or changes selected project/task.

The WorkZ review button is UI eligibility, not a host authorization or proof the task remains idle until a later review action. Existing AgentTaskReview/openPr limitations recorded under AZ013 remain unchanged. Delivery observations refresh manually and do not constitute an automatically live completion notification. Artifact viewer has explicit version capture/delete controls from AZ038; merely opening it does not mutate. Native GUI layout/focus and combined tests/typecheck are root-owned outstanding checks, not asserted here.

No blocking defect found in these added mounting/key/reset lines within the limited contract.

Reviewed SHA256:
- src/workspaces/codez/AssistantPanel.tsx: ab469eb32e479f537b7819a2e95311821398f091c6243a8393a2dfc755874673
- src/workspaces/workz/index.tsx: 472d401fd2bf8b543162f2ffe30a00b4e7aab145fd94a81a224d9b68a20cb866


# AZ048 projectless directory independent review

2026-10-10/11. Reviewer: Codex current_source_review. Read-only module/export inspection; no source, ledger or existing pinned evidence edits. Root reports offline locked domain session24352 one actual filesystem fixture passed. Reviewer did not run Cargo, native IPC or GUI. Host integration is absent.

Observed SHA256:
- crates/agentz-task-state/src/projectless.rs: 3df63fb04bdbf8cb797160440815ac0c6bd7ddab05e00cc8cd191353c16f33f2
- crates/agentz-task-state/src/lib.rs: ba73a74324b07960b853780e797af76d34929047c03f9008505385e0fb2acc73

checked_id requires 36 bytes with fixed hyphens and lowercase hexadecimal, preventing path separators/traversal, though it accepts any UUID-shaped bits rather than enforcing a UUID version. create requires an existing ordinary dedicated root, canonicalizes it, uses nonrecursive create_dir so existing task paths are never adopted, and writes a create_new ownership marker with exact task ID plus sync_all. It does not initialize Git or delete anything. Marker creation/write/sync/open validation failures can leave a partial directory/marker; retry create then fails rather than silently adopting it. Errors are not rollback claims. No root initialization, ledger registration or transactional durable task identity is provided.

open verifies current root/task directory metadata, rejects directory symlink and Windows reparse attributes, canonicalizes the child and requires its parent equal root. Marker must be a regular nonsymlink file with length36 and exact ID bytes. A removed/corrupt marker fails instead of reconstructing state. The actual filesystem fixture proves artifact text survives reopen, Git absence, duplicate-create refusal, traversal refusal and corrupt-marker refusal with artifacts retained. It does not exercise reparse/hardlink/races or interrupted creation.

## Required marker check

At this fingerprint marker validation lacks the Windows FILE_ATTRIBUTE_REPARSE_POINT check used by ordinary_directory. Windows non-symlink reparse files may appear regular and pass is_symlink=false. Add matching marker reparse rejection before reading; report sent to root. The current module should not be frozen as enforcing ordinary marker identity until this is addressed and reviewed. No source modification by reviewer.

## Trust and race limits

Caller must supply a dedicated application-owned root and authoritative task ID; returned path/marker is not permission. This public API cannot distinguish a user project root from a trusted root. Ancestor components are not independently checked. Metadata→canonicalization→create/open→read are separate path operations with TOCTOU; marker read is not same-handle/no-follow, length check does not bound a replaced file's allocation, and marker hardlinks are not rejected. Equal marker contents establish current naming/content consistency, not durable filesystem object identity, unforgeable ownership or hostile concurrent-root safety. Cooperative exclusive root access is required until stronger native containment/lease handling is implemented. Returned paths can be replaced after validation, so callers need fresh authorization for later IO. Directory/parent sync and crash durability are not proven by marker sync alone.

lib.rs currently contains a trailing pub mod projectless export with no added global initializer or call at that export. Exporting this module alone does not create directories or change startup permission. Exact historical reviewed dirty bytes are missing; this inspection cannot prove the entire lib diff is only that export or all previous functions unchanged. Seven upstream consumers may receive a limited export/current-fingerprint refresh preserving prior histories, conditioned on root independently validating current source and retaining its earlier scopes; this review is not whole-lib historical equivalence. New module itself remains subject to the marker finding above. Full AZ048 task acceptance, projectless host persistence/routing, native authorization and GUI remain open.


# AZ048 marker reparse follow-up independent review

2026-10-11. Reviewer: Codex current_source_review. Read-only inspection of the corrected module and current lib fingerprint. Historical AZ-048-directory-independent-review.md preserves the pre-fix finding. No source, ledger or pinned evidence edited; no Cargo/native/GUI execution by reviewer. Root's final targeted Cargo53801 remains pending at request, so the earlier one-test pass does not validate these exact final bytes.

Current raw SHA256:
- crates/agentz-task-state/src/projectless.rs: 6059245cc488d14c77807d4bba89937a9057c1b5181e765fc566e30dd9a69682
- crates/agentz-task-state/src/lib.rs: ba73a74324b07960b853780e797af76d34929047c03f9008505385e0fb2acc73

The reported marker deficiency is closed in inspected source. Following the regular-file/nonsymlink/exact-length metadata check, the Windows-specific block imports MetadataExt and rejects marker.file_attributes() & 0x400 != 0 before read_to_string. This mirrors explicit directory reparse refusal and includes non-symlink reparse files. Formatting does not itself prove behavior; this conclusion is from the actual conditional and ordering. The existing filesystem fixture does not create a Windows reparse marker, so native reparse-case rejection is source-inspected rather than dynamically demonstrated.

The limited create/reopen primitive is reviewable after owner obtains exact-source targeted verification: it uses a dedicated caller-owned root and UUID-shaped path, refuses existing creation, preserves artifacts/no Git initialization, verifies current marker contents and avoids implicit deletion/recovery. lib export hash is unchanged from the prior review, and its limited no-initialization export conclusion remains applicable. Upstream fingerprint refresh must preserve prior scopes/history; entire historical lib byte equivalence is not established.

Trust constraints remain mandatory: caller selects genuinely application-owned root/authoritative ID and excludes untrusted concurrent mutation. Ancestor root reparse/symlink replacement, metadata/canonicalization/read/create TOCTOU, hardlink marker substitution and replacement-file read allocation are not resolved. No same-handle identity, unforgeable ownership, durable filesystem object identity, directory crash durability or transactional task-registration guarantee exists. Partial creation can retain orphan directories/markers and later create fails rather than adopting them. Host is not connected; this adds no IPC authority, execution permission, task routing, GUI behavior or complete AZ048 acceptance. No remaining blocker was found for this explicitly trusted-root, cooperative-use domain slice; hostile-root safety and native integration remain separate unfinished work.


## Preserved review: docs/upgrade/evidence/AZ-044-shared-review.md

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


# AZ040 summary/Advanced independent implementation review

2026-10-10. Read-only review of InputContextSummary and the added CodeZ/WorkZ JSX integration. Existing unrelated repository diff was not attributed to this slice. No independent tests/build/models/GUI run.

The shared summary is presentational: props, translated strings and native details only; no IPC, tool execution, permission response, state mutation or scope minting. Unknown/empty strings remain Unknown, optional counts reject negative/unsafe values. Policy explicitly says configured policy does not grant permission. Workspace is explicitly configured context rather than host-verified environment. Queued text explicitly says “Queued locally”/“本地待发”; it is not persisted Run state. The approval display is a typed possibility but neither current caller supplies it, avoiding inference from stale permissionRequest.

CodeZ adds the summary immediately above its existing composer. Authority and workspace are the configured projectDir. Model uses current selected ID or loaded configured default label; the ID is a selection identifier, not a provider-confirmed effective model. Settings-loading failure falls back to Unknown. Busy displays Running, then existing error, then local queuedView presence, otherwise Idle. Busy takes precedence over pending local messages, so this slice is not an exhaustive queue dashboard. Existing Send/Stop and composer fields remain unchanged by the summary. CodeZ optional skills/connectors count is not yet supplied; their existing controls remain visible and functional, so no hidden-option behavior is introduced by this slice.

WorkZ only displays worktree_path when worktree.id matches sessionRef.current; otherwise the workspace fallback is configured free cwd/direct project, or Unknown for unverified isolated/agent/team contexts. Hidden model selector forces model/source Unknown and does not display the remembered scoped ID or global default. Mode labels separate agent/team/plain; policy remains configuration. Summary counts existing selected optional skills/connectors only when generic controls apply. No stale permission card is converted to waiting state, and no local queue is fabricated for WorkZ.

Native Advanced details keeps its children mounted while collapsed. Team/persona selectors retain original values, callbacks and taskBound/busy disabling. Free cwd and isolation remain outside the collapsed controls. The new start-source/base controls are scoped to isolate && !isFree && !taskBound and busy-disabled, feeding already existing startMode/startBase state consumed by task creation. The default remains working; no automatic switching or selection clearing was added. This adds access to existing configurable inputs rather than changing the send algorithm. Keyboard/focus, overlay placement within collapsed details, layout clipping and native desktop interaction still require GUI acceptance.

No blocking source defect was found in this bounded descriptive-summary/Advanced slice. It does not close budget presentation, permission receipt ownership, verified Run/Attempt live state, five-user first-use evaluation or full AZ040 acceptance. Selected model IDs are intentionally descriptive configured identifiers; future friendly-label resolution must preserve Unknown for missing provider definitions.

Reviewed SHA256:
- src/components/chat/InputContextSummary.tsx: 10ab6a5753f3874655cb2c9636c4b146afc0d8b63f68b3e024ab0f282e1c5847
- src/workspaces/codez/AssistantPanel.tsx: 7c82b46a98d816b34f4ab0ffedbff22f48fa6eaee0010cd9bfcaae25d82a7598
- src/workspaces/workz/index.tsx: e11099d392fe3184eb539db9b2c8b612b1b52af18ac12262c68a9c90ba51f468


# AZ040 observed-budget presentation independent review

2026-10-10. Read-only source/fixture review; no tests, Cargo, models or GUI executed by reviewer. Root reports author BudgetObservation seven and TaskDetailsPanel five targeted tests passed; final combined validation remains root-owned.

BudgetObservation renders totals only for an available typed source. Missing/unknown/stale/unavailable/unsupported sources have explicit status text and no numeric totals. Available known zero is preserved as an observation, not substituted for absent values; null policy maxima use Not configured via nullish checks. Known input/output, known active duration and usage count are distinct from unknown token/duration/pricing record counts. The fixtures cover each unavailable source, mixed known/unknown records, real known zero and absent cost observations.

Costs remain separate currency buckets in micro-units; no conversion, invoice claim, subtraction from maxima, remaining balance or token-to-price estimate is introduced. Empty cost map says No known cost observations. Configured maxima and observed exceedances are labelled observational; the component explicitly says enforcement is not connected, known totals may be incomplete and maxima are not an enforced allowance. It does not call the reservation API or authorize execution. Data validity is supplied by the existing strict taskDetails DTO validator; this component is not a replacement trust boundary.

TaskDetailsPanel change is an import plus replacement of its old budget paragraph with BudgetObservation using data.sources.budget. Existing scope/project/session request key masks old data during render before effect; generation/disposed checks suppress late responses, focus coalescing and error clearing remain intact. Therefore no old budget is exposed through the new child on a task/scope switch. Sources-read-separately/non-atomic disclaimer remains displayed. No polling or model continuation was added.

No blocking defect found within this observational presentation slice. Host accounting completeness, prices/invoices, reservation enforcement, team/member coverage and GUI usability remain unverified and are not inferred from these fixtures.

Reviewed SHA256:
- src/workspaces/codez/BudgetObservation.tsx: 567d6defee4616872f05a7995420e52d0cafdddc0b0a8ec24db8ad001878fa7c
- src/workspaces/codez/BudgetObservation.test.tsx: db01fad4a7d937dd0009db77a80a4499c501030b5112616e70c78ea20abcf22f
- src/workspaces/codez/TaskDetailsPanel.tsx: 8b925d75fb02697050ad3bc910999dfe5348cdf88a209bf3ee7b0f1bd9ad1a6d


# AZ039 bounded delivery observations independent implementation review

2026-10-10. Read-only review of four new summary/wrapper files. No tests/typecheck/build/GUI/model/external operations run by reviewer. WorkZ root mounting has not yet been included in this review. Author reports previous five-summary/four-wrapper fixtures passed; subsequent delivery fix requires root final validation rather than reuse of old results.

Found and resolved during review: initial summary ignored run.delivery and labelled all Acceptance as no receipt. Final source now separately displays authoritative Run delivery accepted/rejected/awaiting_review/not_ready, with added accepted/rejected fixture, while stating that a Run decision does not establish acceptance of a specific artifact version. Artifact-version acceptance, apply and publication still explicitly lack receipts. Full execution enums are shown independently; succeeded never implies validation pass or publication.

Artifacts are counted as distinct IDs and retained versions on this returned page only, with more-available marker. Validation kind counts recorded attachments and explicitly does not establish passed validation/acceptance. Missing store, unavailable/unknown/stale source remain labelled rather than fake zero-success; known empty available page can correctly count zero. No payload/tool text is interpreted as a verdict. Buttons only invoke view callbacks; no accept/apply/PR/merge/model action is introduced. Typed props rely on strict existing DTO services for runtime enum/scope validation, not untrusted raw JSON rendered directly.

DeliveryReviewPanel uses opaque useProjectScope handle, scoped readTaskDetails and fresh observeTaskView before bounded listTaskArtifacts(null,200). Separate sources use allSettled: one failure preserves another valid source but marks failing evidence unavailable rather than empty/pass. Independent project/task/environment mismatch marks artifacts stale. It does not claim binding revisions or separately read sources are atomic. No continuous polling or cursor synthesis occurs.

Render key project/session/handle masks old data before effect; generation cleanup discards late replies and suppresses list invocation when observation outlives its generation. Source loading resets previous values; scope missing prevents reads. Fixtures exercise bounded fresh observations, failed list without leaking private error, late prior-task responses, cross-environment stale and no-scope. Run details may be available while artifacts fail, which is intentionally distinct rather than global successful delivery. Manual refresh remains observational and no active chat projection is rewritten.

No blocking defect remains within this bounded summary/wrapper contract. Exact-version acceptance receipts, complete multi-page artifact enumeration, stable cross-source snapshot, old openPr safety, native GUI and full AZ039 acceptance remain outstanding. This candidate may expose existing review/artifact viewers but does not itself implement their authorization or external-delivery semantics.

Reviewed SHA256:
- src/workspaces/codez/DeliverySummary.tsx: 504f8776ef6ccc20c18b7d6dd84c0f263f299452336cafbb236d1c05b6cbb4df
- src/workspaces/codez/DeliverySummary.test.tsx: f06f3adec81df0e7e2eb851b1b0420752365c10084e3252db59ca0b8f3b6776c
- src/workspaces/codez/DeliveryReviewPanel.tsx: 237c6d2c2310fda3379729fc29acbbbf3f401b2294cceb5d37949ab6c8681458
- src/workspaces/codez/DeliveryReviewPanel.test.tsx: cd5d1f9a732a9e10909e1c6c33ceb123d81f4ce8a67d530be59414e3040b87f0


# AZ039 two-workspace wiring independent review

2026-10-10. Read-only inspection of root integration; no tests/typecheck/GUI/model or Git operations executed. The four new component files are covered separately by AZ-039-implementation-review.md, left unchanged.

CodeZ mounts DeliveryReviewPanel only with a current project and session, using those inputs for the scope-aware observer. Open artifacts sets a project/session identity key; TaskArtifactViewer mounts only when that key matches the current pair and is React-keyed by it. Root added key clearing on project reset, newSession and switchSession start, preventing an existing artifact preview surviving asynchronous selection of another session. No review/merge/PR callback is attached to CodeZ; its existing composer/stream handling is otherwise unchanged by this addition.

WorkZ reuses the existing non-team/non-pool, selected-project/canonical-project/current-session gate. It passes canonical authority to DeliveryReviewPanel. Review changes is present only when not busy and worktree.id matches sessionRef.current, and only opens the existing review UI; no merge/discard/PR runs automatically. Open artifacts uses the same canonical-project/session key as the existing viewer. Old preview clearing at openTask async start remains present. Changing the project/session prevents mismatched-key viewer mounting; child scope generation validation remains the actual asynchronous-response isolation. Neither button issues native permission or changes selected project/task.

The WorkZ review button is UI eligibility, not a host authorization or proof the task remains idle until a later review action. Existing AgentTaskReview/openPr limitations recorded under AZ013 remain unchanged. Delivery observations refresh manually and do not constitute an automatically live completion notification. Artifact viewer has explicit version capture/delete controls from AZ038; merely opening it does not mutate. Native GUI layout/focus and combined tests/typecheck are root-owned outstanding checks, not asserted here.

No blocking defect found in these added mounting/key/reset lines within the limited contract.

Reviewed SHA256:
- src/workspaces/codez/AssistantPanel.tsx: ab469eb32e479f537b7819a2e95311821398f091c6243a8393a2dfc755874673
- src/workspaces/workz/index.tsx: 472d401fd2bf8b543162f2ffe30a00b4e7aab145fd94a81a224d9b68a20cb866


# AZ041 scoped editor follow-up

# AZ041 editor preservation candidate independent review

2026-10-10. Independent read-only source review plus four real exported-helper/serializer integration fixtures. No Cargo, model, native filesystem races, Monaco mount or GUI acceptance run. Two defects found during review were corrected before this conclusion; no existing candidate installer evidence was changed.

EditorConflictTracker captures project/scope generation, tab identity, monotonic local revision, request generation, content and dirty state. Close/reopen/project ABA and same-content edits invalidate old operations. Confirmed reload consent belongs to the captured dirty revision; later edits cannot be discarded by that request. Failed/deleted reads consume their operation but preserve buffer/dirty state. Successful save only marks its exact captured current revision clean and invalidates pending reads. These are in-memory settlement guarantees, not disk-write CAS.

The central setTabs wrapper synchronously updates tracker and tabsRef before passing a concrete array to setTabsState. It evaluates its callback itself rather than giving React a side-effecting functional state updater, so StrictMode replay of React updater functions does not repeat tracker effects. handleEditorChange reports every edit before state publication, including same-content edits. The fixed setter also reports clean-to-dirty transitions even with equal text. Project-scope refs are updated during render: this is conservative operation invalidation, but an aborted concurrent render/real StrictMode/Monaco journey has not been exercised; this review does not claim general concurrent-render transactional correctness.

Dirty watcher deletion now retains the tab with visible explanatory error; accepted dirty reload no longer clears dirty before IO. Read/save replies require tracker settlement before applying React changes. Binary/read failures retain drafts. Save-before-close only closes after a successful current save with unchanged tab identity and clean state; bulk close checks current identities/dirty state and generation, preserving failed saves, newer edits and reopened tabs. Readonly/recovery save is refused. Dirty deleted buffers remain in the existing dirty-buffer snapshot; they are not a proven new durable recovery database.

Found and fixed hot-restore risk: an earlier restored draft formerly replaced any current same-path clean tab, including a user-saved or reopened tab during pending restore. mergeRestoredTabs now always preserves current same-path intent and stores the older dirty draft as an independent reserved recovery tab, including identical-content dirty drafts. Recovery tabs have unique IDs, readonly dirty text and original-path provenance. Real workspace serializer retains their open path and dirty text; restoration recognizes them before any disk read. Dirty persisted buffers are also preserved when the actual disk file becomes binary or unreadable. A restored buffer's disk base remains unknown; no three-way merge or checked-write proof is invented.

Index guards reserved paths out of open/read/reload/watch/diff/reveal, file-tree focus, editor mutation, save and Monaco URI management. Recovery rendering uses a dedicated readonly textarea/copy UI, not FileViewer/LSP. Explicit close uses discard-only wording, not a fake save; bulk dirty checks conservatively retain recovery drafts. Helpers themselves perform no native IPC and never infer disk content. Existing navigation snapshot registration remains, now reading synchronous tabsRef; opaque permission/scope APIs are unchanged. The shared existing AZ036 index fingerprint must be refreshed after review, not silently treated as unchanged.

Actual independent verification: editorRecovery.integration.test.ts four tests passed, final session59314 exit0, 14ms tests/4.10s total. They consume the real tracker (completed save and close/reopen), real merge policy, and real editorSnapshotFromTabs/restoreRecoveryTab, proving current clean intent survives, old draft remains, same-content dirty draft is not swallowed, persisted recovery round-trip retains readonly dirty text, and zero invoke calls. Earlier same target session3327 also passed; own-file ESLint83947 exit0. They do not mount the index or assert its wiring through fake copies. Final tracker test file contains twelve cases: three added fixtures were independently read and verify clean-to-dirty same-content invalidates pending read, successful read/save mutually invalidate captured operations, and old failures cannot clear newer dirty state. Root reports final session43548 tracker12 + recovery4 + navigation6 = twenty-two tests passed, exit0/4.36s. These combined results and source lint/typecheck are root-owned evidence, not our reruns.

No remaining blocking defect found within this local-buffer preservation/restore slice. Host ide_write_file still lacks expected disk revision CAS: parallel saves or external/Agent writes may race and an already sent old write cannot be rolled back by frontend stale settlement. All-write-actor concurrency, durable restart OS verification, explicit conflict comparison/merge, complete accessibility/localized recovery UI and real GUI journeys remain outstanding. Candidate protection is suitable for limited implementation consumption; full AZ041 and stable concurrent-editor acceptance are not declared complete.

Reviewed SHA256:
- src/workspaces/codez/editorConflict.ts: 18913faf070f34c48c1ffef4496aed0cba018f1f2b34400a7f0143e6beadf8a0
- src/workspaces/codez/editorConflict.test.ts: 1a186cb366b939c12b7dc9ac7434ffcb1d44515092aa2fcc8a714b104d913695
- src/workspaces/codez/editorRecovery.ts: 25ff29527254b07dd159f2952902557ac56c0d645a43beae8bf3f7cb030d7968
- src/workspaces/codez/editorRecovery.integration.test.ts: b252966ab7da5f4367501cd14e3123be65653709c54ca86d9a30f2b86be92446
- src/workspaces/codez/index.tsx: 4885b81cc033d4a9499f41088255f58524a74aec719fd2f1eea85b7792e96785


# Limited current translation consumer review

# AZ041 current i18n drift and limited consumer review

2026-10-10. Read-only investigation; neither translation file was edited. Existing old review/pin history must remain. This is a current limited-consumer review, not a claim that the full translation diff is semantically unchanged or authored by this task.

Old pins: en bb8eb0541b49ed46198f7e60ffaa313dcb13272d286f9105f02b451e949d5184; zh 04ff174670e9123ca34274447d7b254662a8b1189920f059ac315df5f6772a65. Current files contain LF only, no CR/BOM (en 1204 LF/51249 bytes, zh 1157 LF/51268 bytes). Normalizing current bytes to LF yields the current raw hashes below. Converting uniformly to CRLF yields en a9c0c61543b27e87843edd487e2bc60cd67e727d1f9de51866995603355456c9 and zh 1febc7baa944b336e6a2ed425cd173f463c2090f1030d217e8ce05f77313ad98: neither matches the old pins. Git HEAD hashes also differ from both old and current pins. Therefore simple uniform newline conversion does not explain the drift. Without old exact bytes, mixed historical line endings/formatting/semantic changes cannot be conclusively attributed or ruled out; comparing with HEAD alone is not a baseline comparison.

Current AZ036 consuming scope independently reviewed: app.saveBeforeProjectSwitch/Close and projectSwitchSaveFailed/projectCloseSaveFailed in both languages ask for saving edited buffers and state cancelled navigation on preservation failure. App uses these as explanatory text after actual dirty/failure preflight decisions, not as permission or state transitions; translations do not remove those guards. These are plain strings, not execution logic. AZ041 current ide.unsavedConfirm, unsavedBulkConfirm and externalChangeConflict clearly distinguish save-before-close from discarding local edits to reload disk. Name/count placeholders remain present in both languages. They do not grant file-write atomicity or fabricate a successful save.

Current AZ017 consuming scope independently reviewed: agent.isolateHint explicitly distinguishes Git worktree/branch isolation from separately controlled system-tool permissions in English and Chinese. agent.taskEnvironmentMissing states the historical task lacks a verified isolated binding, keeps history, requires explicit workspace recovery/new task and refuses silent directory movement. isolateOn/Off labels describe configuration only. These messages do not themselves authorize execution, broaden a filesystem sandbox, resolve a missing binding or turn a configured policy into an approval. Actual host/task identity and permission logic remain outside these files.

Limited conclusion: current relevant navigation/editor and task-environment texts support the existing fail-closed contracts and may be independently pinned for those specific consumers after root validation. The rest of both translation tables, full semantic equality to previous pins, origin of drift and GUI localization/overflow are not established by this review. No automatic ledger refresh, source restoration or stable-release claim is made here.

Current raw SHA256:
- src/i18n/en.ts: c718a52cd27c3b1f6e2dc0950fd6c4d3992ab6af9ae8d6b5837592670d69493e
- src/i18n/zh.ts: bbca86c830405bc983d8297bf000d34dfeea9975f85e6693ac22f2be739f8984


# AZ044 current CodeZ index limited independent review

2026-10-10. Reviewer: Codex current_source_review, independent read-only source inspection. No applicable AGENTS.md was found in repository or C:/Projects/AgentZ, C:/Projects, C:/ parents. Read AZ-041-shared-review.md and AZ-044-shared-source-drift.md before inspecting the current navigation/editor consumers. Only this new evidence file was written; no source, ledger or existing pinned evidence was edited. No tests, typecheck, native IPC, GUI or model evaluation was run by this reviewer.

Current raw SHA256 of src/workspaces/codez/index.tsx: 306ad89024ce5071bdc6d2c678f969d3fb0e8e8b40cf645555802d3a8997f84e. Historical reviewed SHA256: 4885b81cc033d4a9499f41088255f58524a74aec719fd2f1eea85b7792e96785. The exact historical reviewed dirty bytes are unavailable. This review establishes current limited behavior, not precise historical delta, semantic equivalence of the whole file, authorship of changes or a newline-only explanation. Git HEAD is not that missing baseline.

## AZ036 navigation snapshot and project identity

The registered navigation getter synchronously reads tabsRef and activeTabPathRef through editorSnapshotFromTabs; registration cleanup removes the getter. Central setTabs evaluates mutations against the live ref, synchronously reports close/open/content changes to EditorConflictTracker, updates tabMutation and tabsRef, then publishes the concrete array to React. Every editor edit reports a revision before state publication, including equal-content edits; clean-to-dirty equal-content changes are also reported by the setter. Navigation preflight can therefore consume current local buffers instead of a debounced snapshot. The App's actual preflight/persistence decisions are outside this file and are not newly re-certified here.

Current render-time scope invalidation and the reset effect both use exact project string identity. The reset effect explicitly compares prev !== projectDir, including spelling changes, rather than retaining rendered tabs after invalidating their operation ref. Scope changes advance editor generation, reset conflict scope, clear live tabs and open request ownership. Async open requests require captured generation and per-path request ownership before publishing; an existing same-path tab prevents a late open from replacing it. This is local asynchronous publication protection, not host filesystem authority or proof of all concurrent React render behavior.

## AZ041 current buffer preservation wiring

Watcher deletion retains dirty tabs with an explanatory error. Dirty reload requires explicit consent, captures the operation, and does not prematurely clear dirty state. Read replies settle tracker ownership before changing buffer text/dirty status; binary/read errors retain the buffer. Watch callbacks filter event project identity, and cleanup clears pending reload timers/operations. Manual reload similarly checks live project, excludes recovery/browser paths, and settles ownership before publishing.

saveFile refuses readonly/recovery tabs, captures current buffer identity and only clears dirty status after a matching successful settlement. A failed or stale save returns false. Save-before-close requires successful current save, matching tab identity and clean current state; bulk close also guards project generation, retains failed/newly dirty/reopened tabs and excludes recovery drafts. This still does not supply expected disk revision CAS: an already sent old write or external/Agent write may race on disk. That previously recorded limitation remains open.

Hot restore captures editor generation and mutation count; a changed project rejects late restoration. If local intent changed, mergeRestoredTabs preserves current same-path tabs and retains older dirty drafts independently. Reserved recovery paths are recognized before native reads, and persisted dirty text survives binary/unreadable disk results. Recovery content is rendered in a readonly textarea with copy action, outside FileViewer/Monaco/LSP; reserved paths are excluded from editor mutation, save, watcher/reload, file tree focus and model URI management. Explicit recovery close states discard-only; bulk close retains drafts. Native durable restart, explicit compare/merge and full localization/accessibility remain unverified.

## AZ044 mount change boundary and conclusion

BottomPanel is mounted while projectDir exists, with open controlling visibility rather than conditional mounting. This current wiring does not remove the navigation getter, operation generation or editor preservation guards inspected above. It is not independent certification of PTY ownership, process cleanup or terminal tests; those belong to AZ044's separate lifecycle review.

No additional blocking defect was found in the bounded AZ036 current snapshot/project-reset and AZ041 local editor settlement/recovery wiring inspected here. Root may refresh the current fingerprint for all affected ledger consumers in one validated operation, preserving previous review history and appending this limited current review. This is conditional on the file retaining the stated SHA256 and on retaining separate existing scopes/reviews; it is not permission to blindly refresh unrelated source fingerprints or claim full AZ036/AZ041 completion, desktop acceptance or stable release readiness.


# AZ044 current App and translation limited independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only independent inspection of current src/App.tsx, src/i18n/en.ts and src/i18n/zh.ts in the existing bounded AZ017/AZ036 navigation and task-environment explanation scopes. Read AZ-044-current-drift.json, previous AZ-041-shared-review.md and relevant AZ036/AZ017 review history. Applicable AGENTS search from this review sequence found none. Only this new evidence file is written; no source, ledger or existing pinned evidence is changed. No tests, typecheck, native IPC, GUI or model execution was run by this reviewer.

Current raw SHA256:
- src/App.tsx: 81d99ad3755dfaff43bd74343ba2bf0cc21bf2e9df5960358745646bba78f471
- src/i18n/en.ts: 95c75f1c919cf60a40217959be3d4710968b3329ddb4aaa6de80d9f578ec893b
- src/i18n/zh.ts: 14fdeba7b7caedb92907893010532009b716a5b94e6b30fa200a83b2027e3e29

Previous pins are App 79fd684ddaad63cb573db8740358aff68a3a08a818f573325a8d41361db2f465, en c718a52cd27c3b1f6e2dc0950fd6c4d3992ab6af9ae8d6b5837592670d69493e and zh bbca86c830405bc983d8297bf000d34dfeea9975f85e6693ac22f2be739f8984. Exact reviewed dirty baseline bytes are unavailable. No historical semantic equivalence, exact delta, authorship or newline-only explanation is claimed. This is a fresh limited current-consumer inspection.

## Current navigation guards

App stores a synchronous live editor getter and passes its stable registration callback into the continuously mounted CodeZ pane. Project switch and close assemble the live editor snapshot rather than relying solely on the debounced editor patch. Missing getter fails preflight; the helper clones snapshots, refuses dirty buffers, checks current route/snapshot across confirmation and successful persistence, and returns blocked on persistence failure. App checks ready before terminal destruction, scope revocation and route mutation. Busy mutex guards ordinary navigation; picker acquires the same mutex before awaiting native scope selection and holds it through adoption/rejected-scope cleanup, with the navigationOwned inner switch leaving release to the picker finally.

App rechecks current project and complete snapshot after terminal destruction and scope revocation before committing the route. These checks reject late edits and route changes; they cannot undo terminal cleanup or a completed revocation if subsequent checks fail. Cleanup failure is caught and prevents route success. openProjectInCodeZ only changes mode if awaited switchProject succeeds, including same-project success. Same normalized project returns early without a destructive switch. Scope acquisition/revocation calls use opaque existing service handles; this wiring does not grant authority from a path or demonstrate native lifecycle acceptance.

Historical workspace restoration attempts host restoreProjectScope for local projects. Failed scope restoration can retain editing restoration but does not construct a verified durable-plan authority in App; authority-gated actions remain responsible for rejecting unavailable scopes. Existing running-count close confirmation queries chat_running_count, cancels when requested, persists, then acknowledges close; that observation is not proof of complete process-tree cancellation or stable atomic Run state. These retained limitations are not newly certified by source inspection.

## Current relevant translation keys

English and Chinese app.saveBeforeProjectSwitch/Close require saving or closing edited files and unsaved drafts. projectSwitchSaveFailed/projectCloseSaveFailed explicitly describe cancelled navigation on workspace-preservation failure. App only displays these after corresponding actual blocked reasons; strings do not bypass guards or turn a failed operation into success.

agent.isolateHint in both tables separates Git worktree/branch editing isolation from independently controlled system-tool permissions. taskEnvironmentMissing explicitly identifies historical tasks without verified isolated binding, preserves history and requires a new task or explicit original-workspace recovery instead of silent directory movement. isolateOn/Off are configuration labels, not permission receipts or execution evidence. Relevant strings are plain data; no native execution or scope authority is created by their wording. The remainder of translation tables and localization layout are outside this review.

## Limited conclusion

No additional blocking defect was identified in the current inspected AZ017/AZ036 navigation, opaque-scope integration and relevant explanation-key consumers. Root may atomically refresh all affected consumers to these exact current hashes, preserving historical reviews and appending this evidence with the separate current index review. Any subsequent source change invalidates this conclusion's fingerprint. Full App equivalence, all Run observation/authorization semantics, actual native permission tests, whole-table translation equality, desktop roundtrip, model, rollback and full milestone acceptance are not established here. Previously documented partial cleanup/revocation effects and backend authority limitations remain open rather than being counted complete.


## Preserved review: docs/upgrade/evidence/AZ-048-appdata-independent-review.md

# AZ048 application-data domain independent review

2026-10-11. Reviewer: Codex current_source_review. Read-only current projectless.rs inspection; no code/ledger/pinned evidence edits and no Cargo/native/GUI execution. Current module raw SHA256 ddbf3bde9db44e44285fecb679a90e43e55100983a2eee4529cd4bc0e84ae5a8. Root reports previous three-test revision session99869 passed; final directed session49746 was running at request. No final result is inferred from previous tests or current fixture presence.

AppDataStore selects a fixed projectless/workspaces root and projectless/catalog.sqlite beneath caller-supplied app_data. initialize requires existing ordinary app_data, provisions only the two dedicated children via create_dir (no recursive user-tree initialization), then checks database file before Catalog initialization. Existing child paths must be ordinary directories. Existing database must be regular, nonsymlink and on Windows not a reparse point. open_existing checks existing app_data/base/root and database, does not provision missing roots and opens catalog without SQLite CREATE. Task creation/reopen delegate to the previously reviewed persistent Catalog, not marker-only adoption.

The new actual filesystem fixture verifies fixed layout, artifact preservation across dropped/reopened store, Git absence, refusal to recreate a missing moved-away catalog on reopen and refusal of a directory at the DB location without deleting retained reports. It does not dynamically test symlink/reparse/hardlink replacement or interrupted provisioning. Explicit initialize can provision missing subdirectories/new catalog even when older orphan files remain; it does not reconstruct previous registrations or recover historical identity. Host must deliberately distinguish first initialization from recovery.

reopen_task now holds a SQLite transaction across validate_catalog, registered-version lookup and existing directory/marker validation, then commits. This closes the earlier split-autocommit catalog observation gap: schema/root identity and registration are read under one SQLite snapshot/locking context. It does not freeze the filesystem or lock later uses of the returned path. create_task remains one transaction from schema validation through directory creation/INSERT/commit with retained orphan files on failure.

The corruption fixture installs a real AFTER INSERT registration-erasure trigger and checks create_task rejects before any task directory appears, plus open_existing rejection. It then tests unsupported identity schema and altered registration table layout, again verifying pre-create refusal and open rejection. These are meaningful actual SQLite fault fixtures, not mirrored validators. No same-handle filesystem race, concurrent schema-lock, commit-failure or full DB integrity test is provided.

Database path symlink/reparse rejection is specific to AppDataStore's wrapper; public Catalog API still accepts an arbitrary trusted database path. app_data authenticity is caller responsibility: API does not derive the OS app-data directory, set ACLs, authorize a renderer, reject hardlinks or verify ancestor components. SQLite sidecar file identity, file metadata-to-open races, root swap/create/read TOCTOU, marker hardlinks/unbounded substituted read, directory durability and fs+DB atomicity remain unfinished. Provisioning failures may retain partially created roots/catalog files; errors are not rollback evidence. Strict DDL/schema validation is supported-layout refusal rather than a promise of hostile SQLite/filesystem safety.

No additional blocking source issue found within the bounded host-selected trusted-appdata/cooperative-root domain API, contingent on owner's exact final verification. This is not yet Tauri host/UI/session integration, projectless task routing, native scope authorization or full AZ048 acceptance. Existing historical findings/fixes and these limits must remain attached to any candidate fingerprint refresh.


## Preserved review: docs/upgrade/evidence/AZ-048-host-independent-review.md

# AZ048 fixed-appdata host adapter independent review

2026-10-11. Reviewer: Codex current_source_review. Read-only review of the two new projectless commands and invoke registrations, with supporting current domain fingerprint. No code/ledger/pinned evidence edits, Cargo or native invoke/GUI execution. Root reports domain session49746 four tests passed; host check47282 was running at request and is not counted passed.

Current raw SHA256:
- src-tauri/src/commands/project_scope.rs: 32a6e004386212e354f5c5115b28243d6676435314bc7b6c4af1c6809b8f673b
- src-tauri/src/lib.rs: 4243052cf2eef1bd3ee40f00f5f5e13636d78c320312f80e0c5a31ff2ed7b9fd
- crates/agentz-task-state/src/projectless.rs: ddbf3bde9db44e44285fecb679a90e43e55100983a2eee4529cd4bc0e84ae5a8

Both commands call require_main before filesystem work. App data derives from Tauri app.path().app_data_dir, not any renderer path/config parameter. create accepts no directory argument, generates the UUID natively and performs provisioning plus catalog registration on spawn_blocking. reopen accepts only directory_id, enters existing fixed AppDataStore and requires catalog registration plus UUID/path/marker validation; it does not implicitly initialize missing roots or create missing tasks. Join and domain errors propagate.

After blocking work succeeds, existing mint records a main/MAIN_GENERATION VerifiedProjectScope with TrustedGuiOpen provenance and returns its opaque handle/project path. The renderer cannot select an arbitrary directory through these commands. Mint failure after creation returns an explicit retained directory ID/error and does not delete registered artifacts. This is directory opening authority, not a chat session binding, Run permission, execution grant or receipt of task acceptance. Later actions still require their own trusted task/environment observation. Scope mint occurs after path validation and remains subject to filesystem replacement TOCTOU; domain trusted appdata/cooperative root assumptions are unchanged.

lib invoke list adds projectless_directory_create/reopen beside existing project-scope commands. These registrations do not invoke the functions at startup or add implicit filesystem initialization. Existing scope lookup/with_verified_scope and picker/restore code inspected retains its distinct native-source/opaque-handle contract. Exact historical dirty source bytes are unavailable; this limited inspection is not proof that every other byte/function in either shared file is equivalent to previous reviewed versions. Upstream fingerprint refresh must preserve their prior bounded histories and attach this current adapter review, not silently treat old hashes as valid.

Material limitations: create initializes when catalog is missing, so host/UI must distinguish explicit first creation from recovery and retain identifiers on uncertain transport/mint failures; blindly retrying creates another directory. No session-to-directory mapping, list/recovery UI, archival/deletion, quotas, idempotent request key or task lifecycle cleanup is supplied. Concurrent first initialization/create may return SQLite locking/schema errors and can retain partial roots/orphans; no exactly-once or rollback guarantee. Main-window label check is not proof of a human gesture or full renderer compromise resistance. Parent ancestor paths, hardlinks, SQLite sidecars and filesystem metadata-to-use races retain prior domain trust limits. create_dir_all of Tauri-selected appdata adds provisioning but is not hostile ancestor defense.

No additional blocking source issue found in the limited fixed-appdata/main-window/catalog-only directory-open adapter. It may be recorded after exact host compile verification with the current domain evidence. Domain filesystem tests do not instantiate Tauri AppHandle/window/state or prove actual command authorization, scope mint and native invoke behavior. Full host/UI/session integration and AZ048 acceptance remain unfinished.


## Preserved review docs/upgrade/evidence/AZ-048-session-binding-independent-review.md

# AZ048 persistent session-binding independent review

2026-10-11. Reviewer: Codex current_source_review. Read-only current domain and new host session command/registration review. No source/ledger/pinned evidence edit or Cargo/native invoke/GUI execution by reviewer. Root reports domain session86191 six tests passed; host check76168 was pending at request and is not asserted passed.

Observed raw SHA256:
- crates/agentz-task-state/src/projectless.rs: bf97f7d9fd7fdf2225530d42b760b771dcca11be0dca1959906d5c9b5b734280
- src-tauri/src/commands/project_scope.rs: 3122bfa6ea1709223fb730f2fab265bf97d5a9de81e03229b10f0dd74cedff5e
- src-tauri/src/lib.rs: f815518c4cc82c8ff91ee8d3fc0834b7112d6d8205467c21dfef747203543cc3

Domain bind_session validates canonical UUID-shaped session/directory IDs, canonicalizes a caller-trusted ordinary session project and requires a registered existing managed directory. Separate SQLite binding storage has exact known table definitions, singleton schema/root identity and global trigger refusal. Primary key(project_root,session_id) plus UNIQUE directory_id prevents redirecting the same session or sharing a directory with another registered session. Matching existing binding is idempotent; differing directory rejects. Native creation generates independent session UUID and directory UUID, accepts no existing client session to adopt, creates/registers directory then binds before returning minted scope.

Catalog reserve_binding_store commits an initialization watermark before provisioning the separate binding DB. An existing watermark requires expected layout/schema and existing database file; missing binding DB is reported rather than recreated empty. Crash between watermark commit and binding DB creation leaves explicit incomplete initialization requiring recovery. This is deliberately fail-closed availability, not cross-database atomicity. Existing binding schema/root validation and insert happen under one transaction; registered directory verification uses a separate catalog transaction. No filesystem/both-DB atomic snapshot or crash-consistent backup guarantee.

Domain reopen_session opens existing binding database without CREATE, validates binding snapshot, looks up exact canonical project/session and reopens the registered directory; unknown identity/missing storage never calls create. Both new commands require main window, use fixed Tauri appdata and host-selected WorkZ free database root, use spawn_blocking for domain work and mint existing opaque GUI directory scope only after binding succeeds. Reopen derives directory_id from validated managed path. Lib adds two registrations without startup invocation. This is identity allocation/binding, not creation of a chat task row, Run, execution permission or a completed user session journey.

## Host reopen provisioning finding

Current projectless_session_reopen obtains project via workz_overview(app). That existing overview calls free_dir, which uses create_dir_all for the fixed free-session project; it also evaluates/provisions default scratch paths. Consequently a missing session-project root can be recreated before ordinary_directory validation in reopen_session. Managed task directories/binding catalogs still fail closed, but the whole host reopen is not non-provisioning. Finding sent to root: use the existing pure free_sessions_path with host config root and require existing root for reopen, or explicitly retain/document this mutation rather than claiming no creation on reopen. This is a concrete remaining scope discrepancy at the observed host hash.

Six domain tests cover real persistent bindings/drop reopen, matching repeat binding, cross-project/retarget/shared-directory refusal, moved/missing files, actual trigger refusal and missing binding DB refusal including bind-with-watermark not recreating it. They do not instantiate AppHandle/main window/scope mint or chat task records. Unknown-commit create/bind/mint failure can leave retained registered directories/identities; retry of host create generates new IDs and is not deduplicated. Main-window access is not evidence of human gesture. Trusted config/appdata, filesystem ancestor/TOCTOU/hardlink/sidecar constraints and finite-storage/cleanup limits remain.

The domain insert-only trusted-root binding slice is source-reviewable with these restrictions. Host no-provision reopen claim remains blocked by overview side effects until fixed or deliberately narrowed. Shared consumers must preserve prior bounded review histories; missing historic dirty bytes prevents whole-file equivalence claims. No full AZ048/UI/session/chat-run/backup/rollback/native invoke acceptance is established.


## Preserved review docs/upgrade/evidence/AZ-048-session-binding-followup-review.md

# AZ048 non-provisioning session reopen and client follow-up

2026-10-11. Reviewer: Codex current_source_review. Read-only current host/domain/client inspection. No source/ledger/pinned evidence edits or Cargo/native/GUI checks by reviewer. Initial provisioning finding stays in AZ-048-session-binding-independent-review.md. Root reports client eleven controlled tests passed; TypeScript/lint86919 pending. Host76168 passed only the preceding revision and does not verify the current host fix; rerun required.

Current five-source raw SHA256:
- crates/agentz-task-state/src/projectless.rs: bf97f7d9fd7fdf2225530d42b760b771dcca11be0dca1959906d5c9b5b734280
- src-tauri/src/commands/project_scope.rs: c472df3bc613aff46eaefe3758b35381b4a1b2f02d0aea9238c9649f5218b4af
- src-tauri/src/lib.rs: f815518c4cc82c8ff91ee8d3fc0834b7112d6d8205467c21dfef747203543cc3
- src/services/tauri/projectScope.ts: 155e1954ac4a7b8314c6c0505fbc0f4b65a5473b95b6b41d6263170dd518f2ff
- src/services/tauri/projectScope.test.ts: 3cc6ed94a99f6b02d931c7e1122af89bb7adcc07c9c13be6aa867da3b49be3f9

Both session commands now use pure free_sessions_path(resolve_global_config_dir(&app)) rather than workz_overview. resolve_global_config_dir only returns host environment AGENTZ_CONFIG_DIR or Tauri appdata; free_sessions_path only joins fixed free-session child. Neither function creates directories. Only create provisions appdata/free root inside spawn_blocking, then registers directory/binding before mint. Reopen calls existing AppDataStore/reopen_session and never creates the free root, managed directory or missing catalog. Therefore the reported host overview provisioning side effect is closed by source inspection. A host environment override remains possible for the session database root, distinct from renderer-supplied path and fixed managed workspace appdata; no claim that this project root is immune to trusted host configuration changes.

The client new immutable ProjectlessSessionDTO distinguishes session_id, session_project_dir and nested managed directory/opaque scope. create invokes session-create with no renderer identity/path; reopen validates lowercase UUID before invoke and checks exact response session ID before rememberDirectory/cache publication. Nonblank host session_project_dir and nested directory UUID/scope fields are validated. No implicit retry, cwd fallback or replacement create on reopen error. It relies on trusted native response for project root/directory/session association; client checks are field validity, not independent canonical root or persisted-binding verification. Same session/directory UUID equality is not explicitly forbidden by client; host independent UUID allocation is the actual source of distinct identities.

Three new fixtures exercise real exported client service with mocked invoke: host-assigned identities/cache, malformed or mismatched fields with no publication, and one-call missing binding failure with no retry. Test titled mismatched root covers blank root rather than a substituted valid nonblank root; it must not be represented as cross-root identity validation. Existing cache/source/revoke checks remain. Eleven tests are controlled client evidence, not real IPC/domain/UI/task-row execution.

No additional blocking issue found within this limited non-provisioning reopen/client DTO slice, contingent on final exact-host/client verification. Root may refresh shared consumers preserving histories and previous scope limits. Managed directory/session binding still does not create or enforce a chat row, Run/task environment, selected GUI task, backup receipt or execution grant. Unknown-commit/mint failures retain directories and can lose returned IDs; host create retries are not deduplicated. Dual-DB atomicity, host configuration changes, raw-vs-canonical returned session project spelling, filesystem TOCTOU/hardlinks/sidecars, archival/quota/recovery and native invoke/UI acceptance remain unfinished.
