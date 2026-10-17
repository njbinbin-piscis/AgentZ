# AZ036 candidate implementation and verification

2026-10-09. Project navigation now honors the awaited switch result. Busy, cancelled or failed navigation no longer changes mode to CodeZ. This is independently reviewed; it is not full same-task environment routing.

The editor registers a synchronous getter over current tab refs. Project switching uses this getter rather than the 800 ms debounced persistence patch. Dirty buffers, including edits to empty content, block switching before terminal confirmation or save. Clean switching confirms terminals without destroying them, requires a successful workspace_save, rechecks current project/editor after awaits, then proceeds with cleanup. Save failure no longer uses the logging-and-swallowing persistence wrapper. The global workspace snapshot is not a retained per-project dirty backup. Terminal cleanup/scope revocation cannot be rolled back if later input causes a stale abort; no full navigation transaction or layout atomicity is claimed. Close-project remains a separate outstanding path.

The new task_view_resolve command reuses main-window opaque scope lookup, environment lease and existing supported-session binding resolution. It preserves original project/session identity while returning the verified execution workspace separately, with observation_only=true. It neither creates a Run nor executes a model. The asynchronous shared environment resolver can initialize/migrate registry storage; the entire command is not claimed to be disk-readonly. No terminal, editor write, debug or Send permission is conferred by this descriptor.

Actual verification:
- Session 27637: offline/locked desktop library task_view fixtures, 5 passed, 0 failed/ignored, 0.43 s after 8m23s compilation. Real local database fixtures cover identity, missing/unsupported sources, changed metadata, unrelated path and registered workspace differing from authority. They do not exercise the complete native command/window race.
- Session 29454: taskView frontend fixtures 3 passed; scoped ESLint exit0.
- Session 38904: workspaceNavigation6 + taskView3, 9 passed, 0 failed; scoped ESLint exit0 with 11 warnings; TypeScript noEmit exit0.
- Session 32261: strict desktop library Clippy is in flight, not yet passed.

Independent reviews: AZ-036-navigation-review.md, AZ-036-task-view-client-review.md, AZ-036-host-registration-review.md, AZ-036-live-navigation-review.md. Shared AZ017/AZ035 lib/mod/App source pins refreshed through the ledger, preserving prior review history and formal Gates. Root independently inspected the subagent resolver and preflight source and fixtures before integration.

Outstanding: close-project dirty guard; complete task routing consumer and late response isolation; host action authorization; task session/source continuation; native dirty editor/active terminal/worktree roundtrip. The latest archived unsigned installer predates this change. No release or GUI acceptance has occurred.

## Final bounded candidate checkpoint — 2026-10-10 Australia/Darwin

Close-project now uses the same live-buffer preflight; terminal and browser confirmations finish before destructive cleanup, dirty/save failure blocks closing. The specific older close-project gap above is superseded by AZ-036-close-navigation-review.md and final type checking. Cleanup or scope revocation still cannot be rolled back if later edits cause a stale abort. The existing browserClose completion is not acknowledged by this UI, and no native browser exit guarantee is added.

WorkZ exposes an explicitly read-only task-file viewer for supported single tasks with canonical selected project/session. It preserves the active IDE project, displays original authority and observed workspace separately, rejects late replies after task/scope changes, and closes the old preview before a new task's asynchronous history lookup. The region has bilingual states/errors and bounded scroll height; no modal/focus-trap claim or GUI layout acceptance.

Each task_view_list/read request accepts opaque scope, existing session, expected binding and normal relative path, re-resolves host binding under environment lease, checks main-window scope and matching metadata before and after IO. Windows reads verify final root/file paths through held handles, reject reparse links and same-handle hard-link counts other than 1, and bound UTF-8 text to 1 MiB. Directory listing is capped at1000 nodes/depth3; pathname traversal is not an adversarial atomic snapshot, and hard-link alias metadata may remain visible in listings. Text reads on other platforms fail as unsupported. No writes, process creation or legacy unrestricted IDE forwarding is exposed.

Final actual verification:
- Session42179: current host task_view metadata5 + IO6 =11 passed, 0 failed/ignored,3.73s after2m28s compilation; includes real Windows outside-root, junction and hard-link rejection, binary/UTF8/empty/1MiB boundaries and list truncation.
- Session1490: current existing chat-plan authorization fixtures3 passed,0 failed/ignored,.16s.
- Session16852: current navigation6 + task-view client3 + task-files client4 + viewer5 =18 passed,0 failed,3.10s.
- Session77219: scoped ESLint exit0 with5 existing App/WorkZ warnings; TypeScript noEmit exit0.
- Session32261 full strict desktop-library Clippy failed with5 existing lints. One allocation-only path comparison was fixed and independently reviewed. Remaining symbols/skills lint classes are recorded rather than erased: collapsible_if, cmp_owned, redundant_closure.
- Session62556: desktop-library Clippy exit0 with exactly those3 lint classes explicitly allowed. This is an allow-listed check, not a full strict Clippy pass. Earlier session43201 also passed that limited command before the final read module.

Root inspected the new read module and its same-handle Windows FFI layout, then verified real fixtures. Independent final source review is AZ-036-task-read-review.md; older hardlink finding is retained and explicitly superseded by its final fix review. AZ-036-implementation-manifest.json freezes18 candidate source files. Current unsigned installer remains unchanged and predates this candidate.

Still outstanding for full AZ036: all-panel task editor/terminal/debug/preview action binding, same-source session continuation, native dirty editor/active process/task roundtrip and cross-platform acceptance. This checkpoint supports only the explicitly bounded candidate and cannot mark AZ036 or its milestone done.
