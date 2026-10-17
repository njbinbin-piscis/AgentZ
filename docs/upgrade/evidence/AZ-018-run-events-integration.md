# AZ-018 RunStore transaction integration candidate

2026-10-07. Design was reported to the primary agent before implementation. Only `crates/agentz-task-state/src/run.rs` and this evidence were edited; events.rs was consumed read-only. No Cargo build/tests, host code, lib/Cargo changes, ledger update, model/GUI work, or publication was performed. Single-file rustfmt passed. Run source changed and the AZ-017 dependency review fingerprint is now stale; the primary agent must independently review and refresh it before further candidate consumption.

## Same-transaction implementation

RunStore owns namespace initialization under its IMMEDIATE opening transaction. Existing Run registry user_version remains 1. Before initializing an existing namespace, it rejects partial presence and unexpected column/type/primary-key layout; the events initializer rejects future namespace versions. No raw Connection is exposed. Existing constructor/transition/retry/delivery signatures remain compatible.

Managed and direct create insert Run and Attempt, then append `run.created` in that same transaction. Execution CAS changes both records, then appends `run.execution` with a deterministic execution-state event ID. Retry switches to the new Attempt, preserves old history, and appends `run.retry` to the **new** Attempt stream with the previous/new Attempt IDs in its details. Delivery now uses an IMMEDIATE transaction, CASes only the current succeeded/awaiting_review Attempt, appends `run.delivery`, and commits once. All events include authoritative post-change execution/delivery and their scoped task/run/attempt/environment identity, read from stored records inside the transaction. No emitter, callback, or independent event commit is involved.

Any append failure propagates and drops the uncommitted transaction, rolling back the real Run/Attempt mutation. State CAS errors return before append. Stable lifecycle IDs use a nonrepeating state edge/one-time operation per Attempt; existing public state APIs retain their stale-request errors rather than making a repeated successful state mutation silently idempotent. Event append's internal deduplication refuses content conflicts.

`replay_events(&mut self, run_id, attempt_id, cursor, limit)` derives identity from the stored Run and associated current or historical Attempt and delegates to scoped replay under a read transaction. It does not accept caller-supplied task/environment identity, permit a cross-Attempt cursor, authorize external callers, or acknowledge events. Host transport/authorization belongs to a later package.

## Old v1 compatibility boundary

Existing runs/attempts survive namespace activation unchanged. Their prior lifecycle events are not reconstructed or fabricated; replay may be empty for a Run that executed before event capture was enabled. Subsequent valid state changes begin its event sequence. This is additive namespace activation, not a claim of schema-upgrade/rollback compatibility with old binaries. Old binaries that ignore events could mutate state without appending events; mixed-version writers are unsupported and must be blocked by deployment policy. Future schema or partial namespace failures require explicit recovery, not automatic repair.

## Tests added, awaiting primary-agent execution

Four real RunStore fixtures test:

1. A SQLite BEFORE INSERT event trigger forces append failure for create, state CAS, retry, and delivery; real Run/Attempt state remains unchanged and no partial event is committed.
2. Reopening retains the failed old Attempt's events and the new retry event; stale old-Attempt transition and cross-Attempt cursor are rejected.
3. A legacy v1 database with no event tables activates without invented history, captures the next state change, retains user_version 1, and rejects a future event version on reopening.
4. A partial namespace is rejected without recreating its missing schema table or changing the existing Run state.

Run `cargo test --manifest-path crates/agentz-task-state/Cargo.toml --lib --locked` and strict Clippy after the current source is integrated. Existing state fixtures must also pass; no results are invented here. Add process interruption and transport reconnect tests only when host delivery is wired.

Remaining: live host stream emission, durable cursor transport/acknowledgement, reconstruction/retention protocol, authorization and model/desktop lifecycle acceptance. No task is marked done by this storage candidate.

## Reopen-constraint follow-up

The primary agent reported the prior RunStore 12-test suite and strict Clippy passing. Independent review then found that existing empty schema metadata could be reinitialized and that column/PK comparison did not establish event-id uniqueness. The follow-up now requires **exactly one version-1 metadata row** for any existing namespace before initialization, NOT NULL on all event fields, and a nonpartial unique index whose exact columns are task/run/attempt/environment/event_id. Existing empty/future/multiple metadata is rejected without repair; the new-namespace initialization path remains available when neither table exists.

Two additional tests require an existing empty metadata table to remain empty while preserving queued Run state after rejected reopen, and reject both a missing dedup unique constraint and a nullable event_id despite matching columns/PK. Single-file rustfmt passed; the new source and now 14 Run tests were not built or run by this agent. The prior 12-test/Clippy result does not cover this follow-up; primary-agent incremental verification and dependency fingerprint refresh are required.

## Primary-agent verification and additional review
Session45732 ran current run::tests --locked:12 passed/0failed; strictClippy all-targets exited0. Primary review inspected create/transition/retry/delivery single commits, authoritative replay, namespace activation and failure fixtures. It found remaining open-time validation gaps for empty namespace metadata and missing dedup unique constraints; these are assigned for correction before refreshing the consumed AZ017 review. This passing snapshot is not the post-correction result. Old dependency fingerprint remains intentionally stale during this implementation change; downstream activation is suspended until explicit new review.


Final primary verification54983: latest Run tests14 passed/0failed; all-targets strictClippy exit0. Primary independently reviewed lock/schema, metadata cardinality, NOT NULL/unique constraints, actual mutation singlecommit/rollback and authoritative replay. No host streaming/desktop/cross-version writer compatibility claimed.

