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
