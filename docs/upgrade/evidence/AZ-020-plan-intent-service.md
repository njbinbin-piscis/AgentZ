# AZ020 explicit plan intent authority draft

Added only runtime/plan_intent.rs and this record. Not exported or registered as IPC; post-0.9.7. No Cargo/model/GUI execution; formatting and diff checks passed.

PlanQuery returns fresh-resolved PlanBinding plus full persisted current Plan, including completed/cancelled status and historical items; it is not a projection-only UI answer. ExpectedPlan requires identity/revision. Typed Replace carries explicit user objective, generates a new host UUID and creates an empty new objective plan while preserving replaced history. Cancel preserves items/history and neither stops Run nor revokes execution authority. Store CAS is executed once; failures are returned, not replayed.

apply requires an ExclusiveTaskMutation implementation and calls its authoritative require_idle check under caller-owned exclusion before plan CAS. The trait cannot by itself prove the host's synchronization: the real implementation must hold an exclusive task mutation gate shared with **every queue/start path**, freshly query queued/active Runs while locked, and remain held through CAS. A caller-supplied boolean alone does not establish race safety. The owned fixture implementation simulates authority for isolated tests only and is not suitable for IPC registration. Run and plan databases remain non-atomic; no cross-store rollback is claimed.

Three actual SQLite fixtures were written but not run here: active/queued authority refusal and stale identity preserve plan; INSERT trigger abort rolls back old-plan replacement before cancelled DTO survives reopen; explicit replacement has new identity/preserved old history and old token cannot cancel it. Main agent must export, run tests and design the shared host queue/mutation gate before activating commands. Fresh resolver binding errors propagate; plan intent never selects arbitrary task/Run from renderer input or grants tool execution rights.

Primary exported runtime::plan_intent and started unique targeted test48342; result pending. Actual task gate and authoritative activeRun query implementation assigned in exclusive files. Fixture boolean is not a production exclusivity guarantee, IPC remains unregistered. No model/GUI or public release execution.


Primary48342 targeted runtime::plan_intent::tests:3 passed/0failed exit0, compile5m27s. Fixture gate remains simulation; realTaskActivity/activeRun package is undergoing primary70584 validation and unregistered IPC scope checks remain unresolved. No task acceptance claimed.

