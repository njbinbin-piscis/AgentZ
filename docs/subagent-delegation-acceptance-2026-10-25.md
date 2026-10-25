# Sub-agent delegation: acceptance guide (2026-10-25)

Automated: Rust `cargo test --lib` (30 tests: policy, task brief, delegation config/cache/
concurrency, manifest compatibility, prompt rules) and vitest (subagent card, delegation
settings, i18n parity) all pass. The steps below need a compiled build with a live model.

## Manual verification on the built app

1. **Settings > Sub-agent delegation.** Mode defaults to Auto. Switch to Off, start a new
   chat: ask it to list its tools; `delegate`, `call_fish`, `task` must be absent. Research
   only: `delegate` and `call_fish` present, `task` absent. Max concurrency clamps to 1..16.
2. **Agent editor.** Open a registered agent, tick "Allow the main agent to delegate". The
   permission and tool allowlist fields appear. Save and reopen: values persist.
3. **Delegation happens.** In a repo, ask: "Find every caller of <some function>, across
   modules" and "Summarize this long log file". Expect the model to call `task` (or
   `delegate`); a collapsible card appears (agent, brief, status, tool calls, tokens). Expand
   it to see the brief and report. Ask for 3 independent investigations in one message: 3
   separate cards, running in parallel.
4. **Rules hold.** "Read src/main.rs and fix a typo" must NOT delegate.
5. **Writable agents.** Create an agent (for example "Test author"), tick delegable, set
   permission "Can edit files and run commands", optionally restrict its tool allowlist
   (e.g. file_read, file_search, file_write, file_edit, shell). In a git repo ask: "use the
   test author to write unit tests for X and Y" (two writers in one turn). Expect: NO approval
   cards; both run in parallel in isolated worktrees; each result ends with a "Workspace
   effects" block listing the checkpoint refs (`refs/agentz/subagent/<run>/{base,result,premerge}`),
   applied files (A/M/D), auto-merged files, conflicts and the merge order. Edit a file in the
   project while a writer runs: your edit survives the merge. Make both writers edit the same
   lines: the file stays untouched, a marker copy is saved under `.agentz/subagent-conflicts/`
   and the block lists it. In a non-git folder writers run one at a time and the block says
   no git rollback exists. `isolation: "direct"` edits in place after a checkpoint. Builtin
   presets (scout, summarizer, extractor) and `delegate` stay read-only and fully parallel.
   A delegated agent never gets `task`/`delegate`/`call_fish`, team-pool tools, chat_ui, plan or
   memory-store tools; elevate/ssh/email/app_control/browser only when its allowlist names them.
6. **Cache.** Repeat an identical `task` call without touching the repo: the card shows
   "cached". Edit any tracked file and repeat: no longer cached.
7. **Metrics.** Settings > Sub-agent delegation shows runs, failures, tool calls, tokens and
   main-model fallbacks; press Refresh after delegating. With no flash model configured and
   no agent model, fallbacks increments.
8. **Budget.** With a tiny task budget, delegation failure reads "delegation refused: the
   task budget is exhausted ...".

## Baseline comparison (T0.2 / T1.6)

Not measurable offline. Run the three fixed tasks (cross-module call chain, repo-wide config
search, long-log summary) with delegation Off and with Auto; compare the context-usage ring
and whether cards appeared. Gate: at least 2 of 3 delegate and context use is lower.

## Deviations from the plan

- T1.2: no new `parent_call_id` event stream. Sub-agent runs surface as the parent's own
  `task`/`delegate`/`call_fish` tool call, so the card's running/done/error state and parallel
  display come from existing tool events. Live per-step progress inside a card is not shown;
  the final report carries tool-call and token counts.
- T2.3/T2.6: `task` resolves capabilities through `subagent_policy`. Writable agents get the
  agent-mode registry filtered by those capabilities and run WITHOUT per-action approval:
  enabling the agent (delegable + writable permission) is the authorization. Safety is
  isolation (worktree + three-way merge, serialization outside git), not a write-scope fence.
  `delegate` and `call_fish` stay as read-only aliases.
- The project PolicyGate (dangerous-command rules) still applies to writers; only approvals are skipped.
- Writable runs are not cached. Worktree flow is covered by real-git unit tests (merge, dirty tree,
  conflicts, parallel writers, report content); the end-to-end run needs your manual check (step 5).
- Only `node_modules` is linked into a worktree; a Rust `target` dir is rebuilt there. Absolute paths
  into the original project in a brief could bypass isolation (the prompt forbids them).
- T2.4: model choice is manifest provider, then flash, then main model; fallback is counted in
  metrics but only the resolver logic is covered indirectly (no provider-selection unit test).
- T0.3: budget propagation is by code reading (shared `BudgetScope.guard`), no dedicated test.
- T4.3: budget exhaustion is detected from the failure text, not by a pre-check.

## Phase 3 design (implemented) - parallel writing sub-agents

- No approvals for delegated writers; the user enabling the agent accepts the risk.
- Git repo: each writer gets a detached worktree from a snapshot of the live working tree; on finish
  its own diff is merged per file (three-way) into the project under a global merge lock. HEAD,
  branches and the index are untouched. Conflicts leave the project file as is and are reported.
- Recovery refs per run: base, result, premerge. Non-git or worktree failure: writers serialize
  (exclusive lock), with an in-place checkpoint when git exists.
- Write-scope overlap detection was dropped as unreliable. T3.4 (session-scoped dependency plans)
  is still not implemented.
