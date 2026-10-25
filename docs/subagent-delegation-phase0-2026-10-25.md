# Sub-agent delegation: Phase 0 findings (2026-10-25)

Static verification from source. Live baseline (T0.2) is NOT collected here; see bottom.

## T0.1 Tool registration by path

`build_tool_registry` (`src-tauri/src/commands/chat_turn/tool_approval.rs`) builds
`NeutralToolsConfig` with `db`, `settings`, `plan_store` always `Some(..)`, so the
delegation guards `(Some, Some, Some)` always hold. Registration depends only on `chat_mode`.

| Path | delegate | call_fish | task (new) |
|---|---|---|---|
| codez, agent mode | registered | registered | registered |
| workz single agent | registered | registered | registered |
| workz team (coordinator) | registered (non-plan) | registered | registered |
| plan mode (any workspace) | registered | not registered | not registered |

Conclusion: the tools exist everywhere; non-use is a prompting and visibility issue,
not a registration issue.

Extra finding: `build_subagent_registry` builds the sub-agent registry in `"plan"` mode,
and plan mode registers `delegate`. The sub-agent could therefore delegate again
(recursion), contradicting the documented "cannot delegate further". Fixed: the
sub-agent registry is now filtered by `subagent_policy::resolve`, which always strips
`delegate`, `call_fish`, `task`, `pool_chat`, `pool_org`.

## T0.3 Budget accounting

`DelegateTool`, `CallFishTool` and `TaskTool` all pass `budget_scope.clone()` into the
sub-agent run, which wraps the LLM client via `budget.guard(client)`. Sub-agent model calls
are counted against the parent task. Not covered by a dedicated test yet.

## T0.2 Baseline (not collected)

Requires live LLM runs against three fixed tasks (cross-module call chain, repo-wide
config search, long log summary). Record here: main-context tokens and whether delegation
occurred, before/after the prompt change. Delegation counters are now exposed via
`runtime::subagent_policy::metrics_snapshot()`.
