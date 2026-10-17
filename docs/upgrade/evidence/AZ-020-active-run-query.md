# AZ-020 authoritative active Run query candidate

2026-10-08. Exclusive change to `crates/agentz-task-state/src/run.rs`; no Cargo build, host change, ledger change or live-model action.

Added `RunStore::has_active_task(&mut self, task_id: &str) -> Result<bool, String>`. A single read transaction snapshots all stored Runs for that task and every associated Attempt. It validates identifiers and parses every stored execution/delivery value, requires delivery consistency (succeeded permits awaiting_review/accepted/rejected; other execution states require not_ready), and requires a matching current Attempt owned by each Run with the same execution. Historical Attempts must be terminal. Missing/mismatched current Attempts, unknown states and corrupt history return an error rather than false. The query deliberately checks all matching Runs before returning, even when one valid active Run has already been seen.

Queued, Running and Cancelling count as active. Succeeded, Failed, Cancelled and Interrupted do not. Leftover queued/running/cancelling records after reopening still count as active; no records are deleted or silently marked Interrupted. The query performs no state/event write and preserves existing lifecycle append transactions. `&mut self` enables the ordinary rusqlite transaction API without introducing raw connection access.

This is snapshot observation, **not an atomic check-and-write lock across databases**. A host operation consuming it must retain its exclusive task/environment control or otherwise serialize with Run creation/transition for the operation it protects. No process liveness is inferred from a durable active record and no stale recovery policy is added.

Added three fixtures:

- Actual transitions cover queued/running/cancelling and each terminal execution, both accepted/rejected delivery, task isolation and invalid task input.
- Reopen after retry preserves the old Interrupted Attempt and the new queued active Attempt; repeat query does not change event history.
- Injected unknown execution/delivery, inconsistent delivery, malformed current Attempt state, cross-Run current Attempt reference and missing current Attempt fail closed despite a second valid active Run. A separate historical retry corruption fails closed too.

Actually run: scoped rustfmt and `git diff --check`, exit 0. The three fixtures have not been run by this agent; the primary agent owns the unique Cargo invocation and source review fingerprint refresh. This candidate does not by itself establish host plan replacement acceptance.

Primary actual70584 run::tests17 passed/0failed exit0, including the three new active query groups. No stale Run reconciliation or model/desktop acceptance performed.

