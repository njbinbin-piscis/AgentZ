# AZ-018 first atomic candidate: transaction-owned events

2026-10-07. Added only `crates/agentz-task-state/src/events.rs` and this evidence. The module is not yet exported or integrated into a host/state mutation. No Cargo command, GUI/model execution, or ledger change was performed by this agent. Single-file rustfmt passed.

## Candidate contract

`Stream` carries task/run/attempt/environment IDs. `Cursor` carries the complete stream plus an exclusive nonnegative sequence. `Event` carries cursor, stable deduplication event ID, kind, JSON payload and creation timestamp. IDs are bounded and reject whitespace/control shapes; payload serialization is bounded to 1 MiB.

`initialize(&Transaction)` initializes only the durable_event namespace and rejects unsupported namespace versions. It does not alter RunStore user_version. `append(&Transaction, stream, event_id, kind, payload, timestamp)` allocates stream-local positive sequences and returns an identical prior event's cursor on duplicate input. Reusing that event ID with changed kind/payload/timestamp fails. `replay(&Transaction, stream, after, limit)` validates exact stream ownership, rejects future cursors and invalid limits, reads bounded sequence-ordered pages, and fails on a gap. Each stream starts at sequence 1; sequence is not a global chronology. No retention or deletion protocol is provided.

These functions never commit, authorize a caller, publish a notification, or restart a side effect. Identity association with stored Run/Attempt and permission checks belong to the owning storage layer. Concurrent producers must use IMMEDIATE transactions or a serialized writer; uniqueness constraints still reject conflicting writes rather than silently replacing a record.

## State/event atomicity and integration requirement

The caller must perform its state CAS and event append through **the same rusqlite Transaction**, then commit once. Failure of either must roll back both. Calling append in a new transaction after RunStore::transition committed is explicitly outside the contract and is not durable atomic delivery. Namespace initialization likewise belongs in the coordinated schema migration.

Minimum module export requested from the primary agent: `pub mod events;` in `crates/agentz-task-state/src/lib.rs`. No Cargo dependency change is expected because existing rusqlite/serde/serde_json and UUID test dependencies are used. The current RunStore private Connection and schema version boundary still require a separately reviewed transaction-aware mutation API/migration before host integration. Do not expose a raw Connection to renderer/model or claim that standalone events already cover lifecycle state changes.

## Tests added, not run yet

Four embedded SQLite tests cover rollback of state plus event in one transaction and committed replay, exact deduplication/paging/stale cross-attempt cursor rejection, malformed input/oversized payload/deleted-sequence failure, and persistence across a file database reopen. Fixtures only touch an owned UUID temporary database file.

After exporting the module, run `cargo test --manifest-path crates/agentz-task-state/Cargo.toml events::tests --locked` and the existing RunStore suite, then strict Clippy for this crate. If schema/API integration changes, rerun state/event rollback with the actual RunStore CAS, competing producer transactions, and reconnect cursor consumer tests. Current fixtures use a small standalone state table and cannot prove RunStore integration.

Missing acceptance: host event source append wiring, lifecycle/state outbox atomicity, stream transport/disconnect cursor propagation, process-death reconciliation, consumer acknowledgement, retention, authorization, real desktop or model acceptance. AZ-018 remains preparing; this is the first library primitive candidate, not task completion.

## Boundary review follow-up

The primary agent subsequently exported the module and began the crate test run (76668). This agent reviewed while that build was running and made two additional isolated fixes; that in-flight build must not be assumed to cover the new source without a subsequent incremental run.

Append/replay now check the namespace metadata on every operation, rejecting absent, empty, or future schema metadata even if the caller skipped initialize. Replay validates stream-wide contiguity using count versus maximum sequence, so a cursor after a deleted historical record cannot conceal that gap. Two additional tests cover unsupported/missing namespace use and two real SQLite connections contending for an IMMEDIATE writer transaction, then serializing allocation to sequences 1 and 2. The existing deletion test now also checks replay after the deleted cursor. **Six tests are present; none was run by this reviewing agent.** Single-file rustfmt passed and the primary agent was notified to rerun the latest source.

Limits: total removal of a stream or removal of only its trailing records cannot be distinguished from an originally shorter stream without an independent high-water mark. No retention/deletion API is provided, and arbitrary external mutation of the event tables is not supported. The stream-wide contiguity query scans the scoped stream and needs performance measurement before large-event acceptance. Deferred producer transactions may fail on lock upgrade; no retry or independent commit is hidden by the module. The owning state mutation must propagate any append error and roll back its transaction.

## Primary-agent executed verification
Latest incremental session9245: cargo test -p agentz-task-state events::tests --locked: 6 passed, 0 failed; plan::tests: 10 passed, 0 failed. cargo clippy -p agentz-task-state --all-targets --locked -- -D warnings exited0. Earlier full crate session76668:57 passed at its older snapshot, not a full-current-source rerun. No host event integration or desktop acceptance claimed. These modules are post-0.9.7-package work.

