# AZ-020 host continuation intent candidate

2026-10-08. Existing working-tree changes in chat.rs and heartbeat.rs were retained. Only those two implementation files and this evidence were edited by this agent. No Cargo command, model call, slot increase, ledger update, or publication was performed.

`chat_send` now validates clear_plan intent as its first operation, before the environment lease, directory validation/store opening, session UUID, Run queue creation, cancellation registration, semaphore waiting and model execution. Some(true) returns an explicit deprecated error directing the caller to continue or use a separate plan replacement action. None and Some(false) resolve to false, replacing the old default true. No replacement API is invented by this change.

Background heartbeat now passes false to the shared turn's clear_plan argument. Its existing environment lease remains acquired before opening session/runtime state and held through the whole turn; no nested lease or concurrency change was introduced.

A small reusable `continuation_plan_intent` helper and one fixture cover omitted/explicit continuation and rejection of legacy clear with the replacement guidance. The helper test does not exercise a full Tauri IPC or demonstrate absence of database writes at runtime; the early command ordering is source-reviewed and requires primary-agent host integration verification. Single-file rustfmt on both implementation files passed. The fixture was not run by this agent; primary agent owns the unique combined build/tests.

Remaining work includes updating renderer call sites that still explicitly send clear_plan=true and enforcing the same rule in the shared turn runner, owned by the primary agent. A rejected legacy client call is intentional compatibility behavior, not silent plan clearing. Restored cache and durable tool registration belong to the parallel host continuation package. Original task acceptance remains incomplete, and chat.rs source fingerprint must be refreshed after primary-agent review.
