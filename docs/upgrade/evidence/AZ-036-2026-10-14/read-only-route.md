# AZ-036 CodeZ read-only task entry (2026-10-14)

WorkZ now exposes an explicit selected-task action to view its files in CodeZ. It sends the canonical authority project and current session ID, rather than a workspace path. App uses its existing project-switch preflight, honors rejection, and shows the existing host-authorized `TaskFileViewer` beneath the IDE. The viewer resolves the current binding through `task_view_resolve` and revalidates scoped file reads in the host. When the active project changes, App clears the route; a mismatched route is not rendered.

This is deliberately a read-only CodeZ view. The ordinary editor, terminal, debugger, preview, and AssistantPanel still belong to the IDE project; they are not presented as task-bound actions. The observed workspace path is displayed only as context and is never passed to an unrestricted IDE command. Full AZ-036 remains open until these operations receive verified task-bound host adapters and a same-task session continuation path.

Checks: `npx tsc --noEmit` passed; bounded task-view/file/viewer Vitest passed (12 tests); scoped ESLint exited 0 with five existing hook-dependency warnings. Logs are adjacent. No native GUI roundtrip or cross-platform file read has been asserted. This change and the adjacent AZ-035 source are still pending unified independent review; no formal status or Gate promotion.
