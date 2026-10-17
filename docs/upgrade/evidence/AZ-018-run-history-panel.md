# AZ-018 shared passive lifecycle history panel candidate

2026-10-08. Exclusive new `src/workspaces/codez/RunHistoryPanel.tsx`, CSS and component fixture. The primary agent owns CodeZ/WorkZ integration and host validation; this package changes neither parent workspace nor roadmap ledger.

Props are `{projectDir: string | null, sessionId: string | null, busy: boolean}`. The component observes the authorized project scope with useProjectScope, discovers the current authoritative Run/Attempt, then reads scoped lifecycle pages through the validated RunEventCollector. It reads from the beginning at each refresh, bounded to 100 pages of at most 200 events; reaching that limit with more history is an explicit error, not silent truncation or an infinite request loop. The display shows only the latest 100 changes after successful collection.

Scope/session/busy changes and the initial mount refresh recorded history. Manual refresh, window focus/online and document becoming visible also refresh. Each async read checks the request generation before using its response; switching scope/task clears old summary, events and errors, and stale finalizers cannot clear a newer request's loading status. Listener cleanup prevents wakeups after unmount. No model action, resume, retry, plan mutation or chat transcript is invoked.

The interface displays translated execution/delivery states, a translated label for run.created/run.execution/run.retry/run.delivery and time. Raw event payloads, record identities and Run/Attempt IDs are not rendered. Empty event capture explicitly mentions that older execution may predate event recording; null discovery says no recorded execution. Invalid history or scope/storage errors are shown as real errors rather than concealed behind an empty list. Current snapshot error means partial pages are not presented as complete history.

Actual component verification: `npx vitest run src/workspaces/codez/RunHistoryPanel.test.tsx`, exit 0, five React renderer tests passed (session 95924; 112 ms). The tests mock IPC/scope hooks but exercise the actual collector. Coverage includes multi-page reading and cursor use, no raw identity/payload display or resume controls, delayed discovery discarded after project/task switch, busy/manual/focus/online/visible refresh and hidden/unmounted suppression, corrupt page failure without further reads/partial display, real discovery error, Chinese legacy-empty wording and scope removal. Scoped Prettier and diff check exited 0.

No native GUI, live IPC, desktop restart/model execution or two-workspace integration was run by this agent. Historical prior Run/Attempt selection is not exposed: this view is limited to the currently discovered Run and Attempt, while the underlying replay service supports separately authorized historical Attempt reads. Reading history does not claim execution can resume safely.

## Independent review repairs

Parent review identified that effect-based clearing alone could expose the old task for the first committed render after changing scope/session. The component now stores a single view object carrying its scope-handle/session request key; render masks mismatched Run/events/error/loading before any passive effect cleanup. Request-generation checks still discard late asynchronous results. The title explicitly says **Current execution history / 当前执行历史** to clarify the current Attempt limitation.

Payload state lookup now requires an own property in the translation map; JSON strings such as `__proto__` or `constructor` use the generic translated lifecycle label instead of rendering a prototype object/function. This prevents malformed payloads from crashing the history display without exposing their raw values.

Added first-commit identity masking test (inspected before passive effect flushing) and hostile prototype-state fixture. Final scoped Vitest run: seven tests passed, exit 0 (session 87570; 266 ms), followed by scoped diff check exit 0. No full-tree checks were repeated by this agent; parent owns final integrated typecheck/lint and manifest review.
