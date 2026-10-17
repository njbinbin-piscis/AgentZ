# Consolidated independent AZ036 bounded candidate reviews

Scope: navigation preflight and observation/Windows read-only task-file viewer only. Formal acceptance/Gates and full CodeZ action handoff remain outstanding. The following are preserved independent review texts; actual verification is in AZ-036-execution.md. No new semantic review is fabricated by this consolidation.


---
Source review: AZ-036-navigation-review.md

# AZ036 navigation result guard independent review

2026-10-09. Read applicable repository instructions search (no AGENTS.md found in repo or existing C:/, C:/Projects parent locations), AZ-036-routing-design.md and current App.tsx switchProject/openProjectInCodeZ. No production/ledger modification, build/test or GUI execution by this reviewer.

Current App.tsx SHA-256 e0b915a42ab3ba7896db4b5f0796e229577ec1388d03f6ee8f73fd82f3e12efd.

The change is present: openProjectInCodeZ now returns immediately when awaited switchProject(dir) is falsy, before setMode(codez). switchProject returns false on navigation-busy and rejected terminal confirmation, true on same-project and completed switching, and undefined after its caught exception. Thus busy/cancel/error does not change workspace mode; successful and same-project navigation retains previous CodeZ mode behavior. Await still serializes mode selection after the switch attempt. This closes the specific ignored-result defect without authorizing task routing or bypassing existing mutual exclusion. No blocking defect found in this narrow change.

Existing switchProject error behavior can still have earlier partial side effects (e.g. terminals destroyed before workspace persistence fails). This guard does not undo those preexisting actions; it prevents claiming successful CodeZ routing. Dirty-buffer prompts, managed task workspace versus authority project split, preserved WorkZ session identity, late task descriptor generation and real same-task roundtrip remain AZ036 design requirements, not delivered by this line change. No GUI acceptance or full AZ036 completion inferred.

Shared App fingerprint changed and affects earlier review manifests that pin App.tsx; root should refresh them against this limited independent guard review. No claim that all App behavior was retested, or that prior exact hashes remain valid. Product source is frozen for root validation after this review.


---
Source review: AZ-036-live-navigation-review.md

# AZ036 live project-navigation preflight review

2026-10-09. Independent read-only review of App.tsx and CodeZ snapshot getter plus workspaceNavigation helper. Root38904 unified frontend verification underway; host5 passed reported by root. No test/build/GUI run by this reviewer.

No blocking defect identified in this limited project-switch preflight wiring. CodeZ registers a synchronous getter reading tabsRef/current activeTabPathRef and clears it on unmount; App does not rely on the debounced editor patch for dirty detection. Missing getter fails closed. switchProject serializes navigation, retains same-project no-op semantics, invokes non-destructive dirty/terminal-confirmation/strict workspaceSave checks, and only then destroys terminals/revokes old scope/commits project. Helper checks captured project and live snapshot after confirmation/save awaits. App repeats project/editor snapshot comparison after terminal cleanup and after scope revocation; false/exception leaves CodeZ mode unchanged through the prior result guard.

Compared with old switch behavior: unsaved editor buffers now refuse switching instead of being lost by the project-change tab reset; failed workspaceSave is no longer swallowed; cancelled terminal confirmation does not destroy terminals. Snapshot getter avoids a debounce stale-buffer window. Dirty edits or editor-tab changes during preflight/cleanup cause abort rather than publishing a changed project. It does not implement automatic save/discard choice or full same-task worktree routing.

Limits and new observable outcomes: cleanup and revoke are irreversible side effects within this flow, not transaction rollback. A stale response after terminal destruction can retain tabs/project while terminals already exited. A stale response after scope revocation can retain project/tabs while old scope is revoked and needs ordinary re-acquisition. Wiki/chat element state may already be cleared before that final abort. These conservative aborts must not be described as restoring every old workspace resource. Scope/backend native cleanup errors remain bounded by their existing APIs, not proven OS process termination here.

Current snapshot compares editor/project and captured layout values; a callback holds its captured React layout values while awaiting. Do not generalize this into atomic validation of every UI layout update, filesystem write or external buffer mutation. The requirement delivered here is project-switch dirty/tab preservation with a live editor getter. closeProject still follows its separate existing path and is not covered by this package; GUI save/cancel/terminal prompts, native cleanup and actual race interaction remain unverified. All helper fixtures source-level only unless root explicitly records execution.

Shared App/CodeZ pins require parent independent review refresh; prior hashes cannot silently persist. Fingerprints:
- src/App.tsx: 78108b917bd57260d17df5f58a9fecc2a71a2e6431dfb9469b86eac1dbb8f3d5
- src/workspaces/codez/index.tsx: decf3e4f31340dec9a392ed4cb87a8791fe80a8b7709eb7d21704d82f75756eb


---
Source review: AZ-036-close-navigation-review.md

# AZ036 project-close preflight independent review

2026-10-09. Read-only source review; no code/ledger/test/build/GUI mutation or execution. Root lint/typecheck19099 underway, no result claimed here. App SHA-256 79fd684ddaad63cb573db8740358aff68a3a08a818f573325a8d41361db2f465.

No blocking defect found in the limited dirty/snapshot/save/cancel close guard. closeProject uses shared navigation exclusion and live CodeZ snapshot getter, refuses dirty buffers before confirmation, obtains terminal and browser confirmation before destructive operations, calls strict workspaceSave through preflight and requires unchanged project/editor snapshot after awaits. Browser guard refuses by confirmation result; browser guard error falls back to explicit confirmation. A terminal/browser cancellation returns blocked before terminal destruction, scope revocation or project-null commit. Save rejection is explicit blocked save_failed, not swallowed by prior logging persist wrapper.

After ready, terminal destruction and scope revoke each precede a live identity/snapshot recheck. Stale/exception leaves current project and tabs rather than changing project to null. Successful preflight retains original close behavior: closes UI panes, schedules browser close and commits project null. finally clears busy on every path.

These operations are not a rollback transaction. Stale abort after cleanup may retain project/tabs with already terminated terminals; stale abort after revoke may retain them with revoked scope. BrowserClose still runs fire-and-forget with caught error after UI commit, so this package does not confirm native browser termination. Browser-active state can change after guard confirmation and is not covered by editor snapshot. Captured layout callback does not prove atomic all-UI snapshot consistency. These are explicit limits, not reasons to claim resources restored or confirmed on cancellation.

Missing getter fails closed. No automatic dirty save/discard, full same-task routing or GUI save/cancel interaction is delivered. Existing shared App pins must refresh to this actual hash with history preserved; no unchanged hash or full regression run inferred.


---
Source review: AZ-036-validation-followup-review.md

# AZ036 validation follow-up independent review

2026-10-09. Read-only source review. Four app navigation keys exist in both en/zh: saveBeforeProjectSwitch, projectSwitchSaveFailed, saveBeforeProjectClose, projectCloseSaveFailed. Their messages match dirty-buffer save-first refusal and failed snapshot persistence cancellation; they do not promise that native cleanup or rollback succeeded. The names match current App callers, so ordinary en/zh users need not rely on English defaultValue. No GUI rendering or new frontend test executed by this reviewer.

chat_plan existing_binding now compares canonical PathBuf workspace.as_path() against Path::new(&project_id), preserving the former PathBuf-from-string lexical path equality without unnecessary allocation. Both operands still originate from canonical_root; registered short-circuit remains, resolver/identity behavior and Run creation policy unchanged. This is a comparison allocation/lint fix, not a permissions relaxation or new path normalization policy. No blocking defect found in this limited follow-up.

Root reported full desktop strictClippy exit1 with five existing lints; this change addresses only chat_plan cmp_owned. Four symbols/skills lints remain recorded. A subsequent command allowing named existing lints cannot be described as a full strictClippy pass; keep new task_view checks strict and report exact scope. No Cargo/test/lint run by this reviewer, and no Gate/desktop GUI acceptance inferred.

Shared chat_plan source fingerprint changes affect AZ017/AZ018/AZ020/AZ035 pins; parent must preserve prior review history and refresh to these actual sources using this limited semantic comparison review. Fingerprints:
- src/i18n/en.ts: bb8eb0541b49ed46198f7e60ffaa313dcb13272d286f9105f02b451e949d5184
- src/i18n/zh.ts: 04ff174670e9123ca34274447d7b254662a8b1189920f059ac315df5f6772a65
- src-tauri/src/commands/chat_plan.rs: 2392747b2def2b915e115159629cda076b8865ee62abcc90271a62cc5348f4a5


---
Source review: AZ-036-task-view-client-review.md

# AZ036 observation-only task-view client review

2026-10-09. Independent read-only source/test review; no additional tests/typecheck/GUI run. Root sessions29454/29325 underway; no outcome claimed here.

No blocking defect found in this limited DTO/client slice. invoke sends only opaque scopeHandle and sessionId to task_view_resolve, never authority/workspace paths. Local authorityProject is solely an expected comparison value, not host permission. Exact record keys, own properties, plain-object prototype, version1 and observation_only true are enforced. Task/session/original authority identity must match both top level and scope; workspace remains a separate bounded string and is not substituted as project authority. Environment string and binding hash checks reject malformed identity. Request prevalidation occurs before invoke.

Tests meaningfully cover differing authority/workspace, exact IPC arguments, substituted identities, operational claims/extra field, malformed binding and oversize path, and invalid request without invocation. They are source fixtures, not evidence of host lookup/lease/revocation or IDE action authorization. Exported validateTaskView assumes the expected arguments are supplied by a trusted client path; observeTaskView validates them. Returned DTO is an observation, not a capability or a native task-view handle. Consumers must not drive terminal/editor/send authorization directly from workspace_root. Concurrent UI response isolation remains caller responsibility and is not delivered by this service alone.

The module does not mutate project/session, resume model, mint Run or register a tool. Schema strictness will reject future fields until deliberately versioned. No native GUI or full same-task CodeZ/WorkZ routing acceptance inferred.

Source fingerprints:
- src/services/tauri/taskView.ts: 1ec02d0698eb4c4cb8ec82a31453007cbd35fe28f4502310663aebfdc2cafd7b
- src/services/tauri/taskView.test.ts: b6a238ddde57abaf2a0644a738684e45a3cfe716fb961b2c5bb3ef9de7502986


---
Source review: AZ-036-task-files-client-review.md

# AZ036 scoped task-files client and observation refactor review

2026-10-09. Read-only source/test review; no extra tests/typecheck/Cargo/GUI. Root81207 verification underway, outcome not inferred. New task_view_read implementation is explicitly excluded from this review until frozen.

No blocking defect found in the reviewed client/refactor slice. Client invokes task_view_list/read with opaque scope, session, captured expected PlanBinding and relative path; no renderer workspace path grants host authority. Rejects absolute/drive/scheme/backslash/parent/dot/empty-component/control paths before read IPC. Empty path is allowed only for root listing. Responses require exact own plain-object shape, matching complete expected binding, listing workspace equality, bounded 1000 entries, unique valid relative names and safe nonnegative sizes. Text response binds requested path, safe byte count at most1MiB and exact UTF8 bytes, not JS character count. Tests cover actual IPC argument shape, invalid input before invoke, stale identity/workspace substitution/duplicates and Chinese UTF8 mismatch. Tests were read, not run by this reviewer.

Typed TaskViewObservation is assumed validated by observeTaskView; this service is not an independent issuer or permission layer. It does not validate an arbitrary fabricated view before sending it, so native command must freshly verify full authority/binding regardless of client checks. Handle length currently uses JS units while response paths/text use UTF8; opaque ASCII native handles are the intended contract, host validation remains authoritative. Listing does not independently prove each entry is an immediate child of requested directory; future UI should not infer that from client validation alone. Both are scoped client limitations, not native containment proof.

CapturedView helper extracts the former async sequence without changing identity policy: existing_binding -> captured readonly metadata/environment -> local workspace -> execution resolver -> PlanBinding comparison. Captured metadata/environment fields remain private; only crate binding/root are exposed for new host module. verify_observation uses describe to reread metadata/environment and check workspace/project/task consistency. Original task_view_resolve still holds execution lease through async resolution and final main-window opaque scope recheck/describe. Helpers themselves do not acquire the lease/scope, so every new caller must do so; extraction is not a standalone authorization API. Existing resolver environment load may initialize/migrate registry, as prior review already records.

No tool execution, write, terminal/debug permission, model resume or task creation introduced by the client. Native file containment/reparse/hardlink/race verification requires separate task_view_read review and actual fixtures. GUI listing/selection/race behavior remains unverified. Shared task_view pin changed and must refresh with review history. Fingerprints:
- src/services/tauri/taskFiles.ts: 17510b26edc22e72b5ae4232182e4d3fa829268a4b74de43ca65d937967df1cc
- src/services/tauri/taskFiles.test.ts: a3ee5aec7f448d13cdc0d6c95efd95b6d6070b2d22c92f88365b68b8f938c6ce
- src-tauri/src/commands/task_view.rs: 31a41fbf44dca472651dc94b237161b07ce0c5ceef9f94b552930045d64d2c67


---
Source review: AZ-036-task-read-review.md

# AZ036 native task-file and WorkZ viewer independent review

2026-10-10. Read-only source review, no Cargo/API/GUI. Root fixture57843 underway; no result claimed here. Source freeze respected.

## Concrete must-address boundary

Current Windows read_text only checks metadata regular/reparse/size and handle final path within root. It does not reject hard-linked files. A same-volume hardlink inside workspace to an outside file can have a handle final name inside root, so lexical final-path containment alone does not exclude that alias. Reported immediately to root. If claiming link confinement consistent with sandbox, use same opened-handle link-count query and reject nNumberOfLinks>1, with a real same-volume hardlink fixture; otherwise explicitly disclose this unsupported confinement boundary. No exploit/test executed by this reviewer.

## Other reviewed behavior

Relative path rejects parent/dot/empty component, control, drive/ADS/scheme/backslash and Windows trailing-dot/whitespace alias forms. Captured host lookup uses main-window opaque scope, supported existing task binding and held environment lease. Expected full PlanBinding is checked after async resolve; with_verified_scope checks project identity and observations before/after IO. No renderer root or legacy unrestricted IDE route is forwarded. Windows text read opens root/file no-share-delete/no-share-write with final-component reparse handling, verifies root/file final handles and raw UTF8 size<=1MiB, rejects NUL/nonUTF8/nonregular and bounds read length. NonWindows text read is explicitly unsupported.

Directory listing is pathname-based recursive enumeration. It checks each reparse/ canonical containment, node1000/depth bounds and safe relative names, but checks and enumeration are not one atomic handle operation: concurrent directory replacement can race metadata/canonical/read_dir, and no hostile directory lifetime containment proof exists. Root/env scope lease is cooperative metadata protection, not adversarial filesystem snapshot. Lists can change, lose entries or reject midflight; no atomic directory snapshot, cross-platform or TOCTOU immunity claimed.

WorkZ entry is only nonteam/nonpool with matching selected project/canonical scope and current session. fileViewKey binds canonical project/session and remounts for another task; opening/closing viewer does not set project/session or create Run. Viewer scopehook/request key includes handle/project/session; mismatched-key render masks prior content before effect, cleanup increments generation, IO uses generation+operation guard. Text renders in pre, not HTML, and no Send/write/terminal/model action is exposed. Late response cannot populate visible other-task state. Listing recursively includes descendants, so UI directory labels are descriptive rather than immediate-child-only tree.

New commands module + two generate_handler list/read registrations only expose the fresh scoped read API; no existing AZ017/AZ035 handler initialization behavior changes. Shared pins must refresh to actual hashes. Full GUI interactions, scope revocation timing fixture through actual IPC, native adversarial directory races and hardlink handling remain acceptance items; helper tests do not validate entire desktop flow.

Reviewed pre-fix fingerprints:
- src-tauri/src/commands/task_view_read.rs: 0aa948b9000950db5efe705da1cef45a57d847faf513d1a081a39fe8eab6fb3b
- src-tauri/src/commands/mod.rs: ccab8c3717a612c8e7049c0aa2601d7ee34b814ea6068d2f60d29b3f5a122619
- src-tauri/src/lib.rs: a4444a81d65b643baaf6ddad9e7d738425cbe8e596c5c59efa6c14728d19835d
- src/workspaces/workz/index.tsx: 74ee339e609c7d3303c484b33c3542bc1b6198f3c1e5a69d654f9a4adfca1503
- src/workspaces/codez/TaskFileViewer.tsx: 0864228c8da7c7305c5698d805033a1c3b8aaa5f21d872f90d098ce42a5a4cf2


## Final hardlink-fix independent re-review

The preceding hardlink finding describes the earlier version and is now addressed in this frozen source. require_single_link queries GetFileInformationByHandle on the same opened file handle before reading content. Its repr(C) DWORD/FILETIME field order matches BY_HANDLE_FILE_INFORMATION including number_of_links. Failed query returns an error before reading uninitialized data; successful count must equal1, so unknown0 and multi-link reject. The normal metadata/final-path/budget checks remain unchanged and no share-write/delete permission was added.

New real same-volume fixture creates an outside original and inside alias, checks direct handle count and read rejection, preserves outside original content and removes only fixture alias. This fixture has not been run by this reviewer; root42179 rerun was compiling at review time. The specific static preexisting hardlink disclosure is closed by inspected same-handle validation. No blocking defect remains in the limited scoped Windows text-read/viewer candidate reviewed here. This does not prove adversarial concurrent object lifetime containment, atomic directory enumeration, GUI or cross-platform runtime acceptance.

Final fingerprints replace the earlier pre-fix list for parent shared-pin refresh while retaining historical finding:
- src-tauri/src/commands/task_view_read.rs: ad1c51ddb8123bf4790b36aa9d98bab3a13ef56fbd56ec87f4a64f13c67f25e8
- src-tauri/src/commands/task_view.rs: 18fbfe84c79ea825c0d4a9da8036b63436bdbc3acf80c6ecb4fedc1ae15970cf
- src-tauri/src/commands/mod.rs: ccab8c3717a612c8e7049c0aa2601d7ee34b814ea6068d2f60d29b3f5a122619
- src-tauri/src/lib.rs: a4444a81d65b643baaf6ddad9e7d738425cbe8e596c5c59efa6c14728d19835d
- src/workspaces/workz/index.tsx: 74ee339e609c7d3303c484b33c3542bc1b6198f3c1e5a69d654f9a4adfca1503
- src/workspaces/codez/TaskFileViewer.tsx: b3913a6fba9419721254bbfe5c1c60ea7deb8ecf0a1a9b6eaa11f098bf41b1f1


## Final candidate entry/display follow-up (immutable review append)

Root reports42179 actual11 host fixtures passed including hardlink,3.73s; reviewer did not rerun. WorkZ openTask now clears fileViewKey immediately after incrementing request identity, before asynchronous history/environment fetch; this removes the previous task viewer during new-task loading. Same current foreground live-task early return occurs before this clear, preserving its viewer and stream. No project/session authority change or new read permission introduced.

TaskFileViewer presentation changes only bound section height45vh with scrolling and pre-wrap/overflowWrap; content remains text children, no HTML evaluation. No behavioral/authorization regression found in these two narrow changes. Actual GUI/layout remains unverified. Final shared source pins:
- src/workspaces/workz/index.tsx: 2db994e386d59fbcc6d79a75433bc12fe644dbdafda130c04b9840153f944811
- src/workspaces/codez/TaskFileViewer.tsx: b3913a6fba9419721254bbfe5c1c60ea7deb8ecf0a1a9b6eaa11f098bf41b1f1
