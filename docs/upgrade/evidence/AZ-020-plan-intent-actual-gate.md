# AZ-020 plan intent: actual task gate / Run activity adapter

2026-10-08. Exclusive candidate implementation in `src-tauri/src/runtime/plan_intent.rs`; no host/IPC registration, environment-gate modification, Cargo build or ledger change.

New host adapter:

```rust
apply_under_task_gate(project: &Path, activity: &TaskActivity,
    binding: &PlanBinding, expected: &ExpectedPlan,
    intent: PlanIntent, now: u64) -> Result<Plan, String>
```

Caller must supply a freshly host-validated PlanBinding and retain its environment read lease. The adapter compares the canonical project with binding.project_id, uses that project and exact task_id to acquire `TaskActivity::try_enter` internally, and stores the real owned guard in a private ActiveRunGate. No external guard or idle boolean is accepted. Private gate scope equality is checked again before querying the authoritative RunStore snapshot. Any queued/running/cancelling Run rejects mutation; corrupt/unknown Run/Attempt history propagates failure. The same guard remains alive through synchronous plan identity/revision validation and the actual SQLite replacement/cancel CAS. Errors release it by RAII.

The former injectable gate trait and raw apply helper are now module-private; fixture-only booleans cannot be used by production callers outside this module. Query remains available and does not mutate plans. This adapter does not grant renderer permission, validate an IPC project path, acquire the caller's environment lease or automatically stop/reconcile stale Runs.

Three additional actual database/task-gate fixtures:

- Persist a queued Run, release its handle, then reject plan cancellation using the authoritative Run registry with unchanged plan revision; error releases task exclusion.
- Hold the actual same-key task gate with no Run database present; reject before opening that database or mutating the plan.
- Acquire the private actual adapter gate and invoke the real replacement helper: another thread cannot enter the same project/task queue key, while another task can enter. The real replacement changes identity, stale expected identity then rejects, corrupt Run state rejects, wrong canonical project/task gate scope rejects, and plan state remains unchanged on these failures.

The thread fixture observes exclusion inside the guarded mutation scope before the synchronous CAS and again after replacement while the owned guard remains alive. It does not simulate process death or assert cross-process locking across the separate Run/plan databases. External writers bypassing TaskActivity are outside this same-host protocol; all actual queue/start paths must use the same AppState TaskActivity instance and retain their guards. An independently created TaskActivity instance is not equivalent authority.

Actually run: `rustfmt --edition 2021 src-tauri/src/runtime/plan_intent.rs` and scoped `git diff --check`, exit 0. New fixtures were not run by this agent; primary agent owns the single Cargo validation. This package does not establish public intent command acceptance or renderer authorization.

Parent's first actual six-test run reported four passing and two failures at managed `RunStore::create`: the fixture had a plan binding but no registered task environment. Corrected the shared fixture to register a real local TaskEnvironment with matching canonical project/task/environment before Plan creation, and replaced its misleading direct fingerprint environment label with an explicit managed fixture identity. Production Run validation is unchanged. This correction is not itself a claimed passing rerun; the parent must repeat the targeted tests.

Root actual validation 2026-10-08: corrected real environment registration fixture rerun68174 passed all6 plan_intent tests,0 failed,0 ignored. Initial55673 had4 pass2 fail (fixture registry absent); failure was not a production bypass and was repaired using real task-environment registration. Project_scope3 and continuation_intent_tests3 also passed8552. This verifies SQLite/host primitives only; native GUI, registered IPC, model and cross-process acceptance remain unrun.
