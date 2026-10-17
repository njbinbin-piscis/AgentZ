# AZ036 project-close preflight independent review

2026-10-09. Read-only source review; no code/ledger/test/build/GUI mutation or execution. Root lint/typecheck19099 underway, no result claimed here. App SHA-256 79fd684ddaad63cb573db8740358aff68a3a08a818f573325a8d41361db2f465.

No blocking defect found in the limited dirty/snapshot/save/cancel close guard. closeProject uses shared navigation exclusion and live CodeZ snapshot getter, refuses dirty buffers before confirmation, obtains terminal and browser confirmation before destructive operations, calls strict workspaceSave through preflight and requires unchanged project/editor snapshot after awaits. Browser guard refuses by confirmation result; browser guard error falls back to explicit confirmation. A terminal/browser cancellation returns blocked before terminal destruction, scope revocation or project-null commit. Save rejection is explicit blocked save_failed, not swallowed by prior logging persist wrapper.

After ready, terminal destruction and scope revoke each precede a live identity/snapshot recheck. Stale/exception leaves current project and tabs rather than changing project to null. Successful preflight retains original close behavior: closes UI panes, schedules browser close and commits project null. finally clears busy on every path.

These operations are not a rollback transaction. Stale abort after cleanup may retain project/tabs with already terminated terminals; stale abort after revoke may retain them with revoked scope. BrowserClose still runs fire-and-forget with caught error after UI commit, so this package does not confirm native browser termination. Browser-active state can change after guard confirmation and is not covered by editor snapshot. Captured layout callback does not prove atomic all-UI snapshot consistency. These are explicit limits, not reasons to claim resources restored or confirmed on cancellation.

Missing getter fails closed. No automatic dirty save/discard, full same-task routing or GUI save/cancel interaction is delivered. Existing shared App pins must refresh to this actual hash with history preserved; no unchanged hash or full regression run inferred.
