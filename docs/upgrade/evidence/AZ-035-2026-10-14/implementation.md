# AZ-035 bounded task-detail implementation (2026-10-14)

The read-only host task-detail service now obtains artifact references from the task/environment-scoped artifact store and the latest event cursor from the current Run Attempt. It returns `unavailable` when the bounded 200-version overview would be partial, and `unknown` for legacy or empty event history. Neither state is presented as an empty manifest. The client parser now accepts the host's versioned `stale` and `unavailable` statuses while retaining scope validation.

The host source reads remain separate, so the DTO declares `atomic_snapshot: false`. This is an implementation candidate, not acceptance of the full cross-view handoff journey or a desktop GUI run. The working tree contains extensive prior unreviewed changes; these specific files are presently untracked, so a later independent source review must pin exact content and provenance.

Local checks: `cargo test -p agentz-task-state --offline --locked` (152 unit and 2 additional tests passed), `cargo test -p agentz-desktop --lib runtime::task_details --offline --locked` (8 passed), bounded Vitest on task detail and delivery views (19 passed), and `npx tsc --noEmit` (passed). Logs are adjacent. The initial host test failure was a mistaken assertion that the two appended agent events started a fresh sequence; Run transitions had already emitted events. The assertion now compares with the returned append cursor and the rerun passes.

Remaining: independent code review and source manifest; exact cross-view task/environment navigation test; real Tauri desktop read with an environment lease; explicit atomicity/revision UX acceptance. No task or Gate status is promoted by this note.
