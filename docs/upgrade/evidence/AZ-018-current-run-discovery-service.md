# AZ-018 current Run discovery service candidate

2026-10-08. Exclusive frontend service `src/services/tauri/runDiscovery.ts` and its test file. No UI/host/ledger changes, Cargo build, external model call or automatic recovery.

`readCurrentTaskRun(scopeHandle, sessionId)` calls only `chat_current_run` with `{scopeHandle, sessionId}` and returns a frozen `CurrentTaskRun` or null. The DTO mirrors Rust Run snake_case fields: run_id, task_id, environment_id, attempt_id, execution, delivery, updated_ms. Rust Execution uses **cancelled** (double l); unsupported canceled/resuming values reject. All execution and delivery enums are explicitly validated, as are task equality, identifier shape/UTF-8 size and a nonnegative safe-integer timestamp. Delivery must be not_ready for nonsuccess execution; succeeded requires awaiting_review, accepted or rejected.

The service makes no cache assumption about a previous Attempt: each call observes the host's current Run snapshot. A retry response returns its current attempt_id rather than reusing a prior one. Null is no stored current Run, and authorization/unknown-task/storage errors propagate without retry, fallback mint, execution start or model resumption. The UI caller remains responsible for discarding responses after its task/project selection changes.

Six meaningful mock tests exercise exact scope-only IPC arguments, null and unknown-task behavior, successive failed→retry current Attempt snapshots, all Rust execution/delivery variants, immutable DTOs, foreign task and malformed IDs, unsafe timestamps, unknown enums and inconsistent delivery, and invalid requests rejected before invoke. Command-call checks ensure only discovery IPC occurs, including failures.

Actually run: scoped Prettier, diff check and `npx vitest run src/services/tauri/runDiscovery.test.ts`, all exit 0. Six tests passed (session 72230; 26 ms test time). No native host discovery, whole-project TypeScript check, GUI display or restart acceptance was run by this agent; primary agent owns integration validation.
