# AZ035 limited implementation review for AZ036 consumption

2026-10-09. Independent read-only review of current pure aggregation, readonly host service, chat_task_details IPC, existing binding/scope helpers, TypeScript validator and dual-view panel mounts. No source/ledger/manifest changes or Cargo/npm execution. Appropriate as candidate code dependency only; formal GUI/Gate remains unchanged.

## Authority and freshness

IPC accepts opaque scope_handle and existing session_id, never renderer workspace/project/run authority. It acquires environment execution lease before lookup; lookup_for_main verifies main window and host project scope. existing_binding reads existing session metadata, supports local CodeZ/WorkZ single-agent records, rejects unsupported team/pool/remote/background. Registered environments take precedence; direct/free WorkZ resolution uses existing metadata cwd and host-restored free scope rules, without automatic ensure-session. resolve_task_workspace and metadata-only resolve_binding yield fresh PlanBinding; random metadata ID is neither Run creation nor execution permission.

After awaited binding resolution, with_verified_scope rechecks the handle under the registry mutex and requires same canonical project. The synchronous service read stays inside that guard while the environment lease remains alive. Revoked scope cannot interleave with these reads through the application registry. This is not a proof against out-of-process filesystem or metadata changes. Passive query does not take the task mutation gate and therefore can observe successive task revisions while work runs.

## Data and UI contract

Pure aggregate consumes prevalidated sources and checks task/environment/project and full plan binding; available events must match available Run/current Attempt. Full cancelled plans retain items and revision. Source union distinguishes available/missing/unsupported/unavailable/stale/unknown; unknown budget or artifacts do not become zero or fake references. Versions identify each separate observation; atomic_snapshot is always false and cannot be a global transaction token/cache key.

Host environment/Run/plan/observational budget readers use existing readonly openers; missing files/head/policy are Missing, corruption/I/O fails the entire query explicitly. Events are currently Unknown and artifacts Unsupported. Per-source Unavailable/Stale is representable but host does not currently degrade individual failed reads into those DTO states. Reservation enforcement, precise fees/remaining balance and artifact validation are not supplied by this interface.

taskDetails.ts reuses durable plan/Run/event types and strictly validates exact DTO shape, safe integers, source status/value/version and scope identities. It rejects task mismatch and atomic_snapshot=true; it does not expose tool or authority operations. UI uses project-scope handle and render key to mask prior task/project first frame, generation/disposed checks for late replies, failure clears old data, focus/inflight pending refresh and busy transitions cause reload. WorkZ mount excludes team/pool and uses resolved canonical planProject; CodeZ uses project/session props. Panel is a passive summary, not full artifact/approval/recovery operation UI.

## Existing actual validation

- AZ-035-task-details-integration.md: root32039 pure aggregation five tests passed; strict all-targets Clippy passed.
- AZ-035-plan-readonly.md: root67342 plan16 tests and strict Clippy passed.
- AZ-035-host-readonly-integration.md: root27341 four real SQLite service fixtures passed, 0 failed/ignored; readonly bytes, full cancelled plan, missing no-create and corrupt/cross-scope rejection.
- AZ-035-two-workspace-integration.md: root64411 eleven service/panel Vitest fixtures plus tsc exit0; root61531 limited ESLint exit0. Earlier fixture failures are retained and explained. These are actual earlier recorded results, not reruns by this reviewer.

No new blocking P1/P2 found in this bounded passive API. Follow-ups: complete per-source degradation/stale policy, richer events/artifacts/approval data, review responsiveness while multiple SQLite readers hold scope mutex, native IPC/window/GUI races and real multi-view task journeys. No GUI, model, install, cross-platform or atomic snapshot acceptance claimed. AZ036 may consume DTO passive facts; must separately authorize every proposed operation and never interpret display Source::Available as permission.

## Suggested source manifest file list (not written here)

Include crates/agentz-task-state/src/task_details.rs, plan.rs, run.rs, budget.rs and lib.rs; src-tauri/src/runtime/task_details.rs, runtime/mod.rs, commands/chat_events.rs, commands/chat_plan.rs, commands/session_metadata.rs, commands/project_scope.rs, runtime/project_scope.rs, runtime/approval.rs, commands/agent_task.rs and lib.rs; src/services/tauri/taskDetails.ts, taskDetails.test.ts, chatPlan.ts, runDiscovery.ts, runEvents.ts, projectScope.ts; src/workspaces/codez/TaskDetailsPanel.tsx, TaskDetailsPanel.test.tsx, AssistantPanel.tsx and src/workspaces/workz/index.tsx. Add data_scope/projects/session metadata helpers if pinning full authority transitive closure. Root must generate actual current SHA manifest and refresh changed upstream review fingerprints; this list is not a generated or validated manifest.
