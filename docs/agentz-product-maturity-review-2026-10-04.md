# AgentZ Product Maturity and Engineering Review

**English** | [简体中文](agentz-product-maturity-review-2026-10-04.zh-CN.md)

> Status (2026-11-02): re-checked against AgentZ 1.0.40 with piscis-engine pinned at v0.8.71 (`Cargo.toml`). The original findings and conclusions below are kept as written; file/line references in the evidence describe the 0.9.5/0.9.6 code at review time and many have since moved. Current state, as far as it can be confirmed in code:
>
> - **F01 fixed (in code)**: tasks now have a persisted `TaskEnvironment` record (`environment_id`, `project_root`, `workspace_root`, `branch`, `base_ref`, `base_sha`) registered in `src-tauri/src/commands/agent_task.rs` (`agent_task_create`), and `chat.rs` resolves the run directory via `resolve_task_workspace` instead of trusting a frontend ref. Not re-verified through desktop interaction.
> - **F02 fixed**: `agent_task_change_set` / `agent_task_file_view` collect changes from the task's actual `workspace_root` against the frozen `base_sha` (`agentz_task_state::changes`) and return typed errors. The legacy branch-only `agent_task_changed_files` / `agent_task_file_diff` still exist and still use `unwrap_or_default()`.
> - **F03 fixed**: `src-tauri/src/commands/journal.rs` (`open_task_journal`) resolves the task workspace before opening storage.
> - **F04 fixed**: `agent_task_discard` now archives (`agentz_task_state::cleanup::retain_task`), requires a stable task id, holds the cleanup lease and returns a receipt; `agent_task_archive_snapshot` / `agent_task_restore_snapshot` exist.
> - **F05 partially fixed**: task creation takes a `start_mode` (default `commit`) and records `base_sha`; merge is a versioned transaction (`agent_task_merge_preview` / `agent_task_merge` with expected task/target heads and a repository lock). Implicit `git init` / empty-commit behaviour was not re-checked.
> - **F06 partially fixed**: isolation is now on by default (`src/workspaces/workz/useWorkzSessionStore.ts`, `agentz-workz-isolate !== "0"`). An enforced process/network/desktop sandbox was not confirmed.
> - **F07 partially fixed**: the Webview iframe now uses `sandbox="allow-scripts"` without `allow-same-origin` (`src/extensions/ui/WebviewHost.tsx`), and the CSP uses `frame-src 'self'` (`src-tauri/tauri.conf.json`). Functional verification in a real Tauri build is still required.
> - **F08 fixed in the kernel**: `Settings::save` in `vendor/piscis-engine/piscis-kernel/src/store/settings.rs` now fails closed (SecretStore and per-field encryption errors propagate), writes atomically via a temp file, and the master key can be protected by the OS credential store (`security/secrets.rs`).
> - **F09 partially fixed**: `chatSend` rejects `clearPlan: true` and continuation preserves the plan (`src/services/tauri/chat.ts`). A unified durable Task/Run/Attempt model was not fully verified.
> - **F10 open**: the WorkZ logic moved into `useWorkzSessionStore.ts` (about 1650 lines); `chat_turn` is now a module (`mod.rs` ~940, `tool_approval.rs` ~1140, `prompt_assembly.rs` ~900 lines).
> - **F11 partially addressed**: the frontend now has 69 test files (vs 7 at review time). No full desktop E2E for the task lifecycle was found.
> - **F12 open**: no updater is configured in `src-tauri/tauri.conf.json`.

Date: 2026-10-04. Subject: the AgentZ desktop application and piscis-engine v0.8.68 as pinned by Cargo.lock.

The version was 0.9.5 when the review started; other work during the review moved it to 0.9.6 (a787b6d). This is a review of an active working tree, not certification of a frozen commit. The review did not modify product code.

## Verdict and scope

AgentZ already has a substantial foundation for a desktop Agent product: a Tauri/Rust host, a React/Monaco workbench, a shared Agent kernel, a Node extension host, remote development, tool permissions, a file-change journal, tasks and teams. It is beyond a prototype of a chat box plus tools, but it is currently best positioned as a feature-rich developer preview; the existing evidence does not yet support claiming it is a mature desktop product to which long-running tasks can be confidently delegated.

The main gaps are in the contracts between task identity, execution environment, change review, recovery and authorization, not in adding more feature entry points. We recommend first completing a reliable end-to-end loop for a single task, then expanding teams, channels and automation.

This review covered architecture, the core execution path, task isolation and review, permissions and credentials, extension integration, persistence, CI, release and product positioning. Methods: source cross-checking, frontend checks, host compile checks and experiments in a temporary Git repository. We did not drive the desktop UI, and did not run long real-model tasks, remote hosts, installers or platform fault injection, so visual experience, real-world exploitability, long-term stability and cross-platform compatibility still need dedicated verification.

## Product definition

Recommended positioning: a desktop workbench, with developers as the first users, where people and Agents deliver code, research and documentation together in the same workspace.

CodeZ is the view where a person works directly: reading, editing, debugging, testing, inspecting details. WorkZ is the view where a person delegates work: stating goals, watching progress, adding constraints, handling blockers, reviewing deliverables. Both share tasks, execution environments, authorization, context, artifacts and history.

The key experience should be: the user hands WorkZ a bug fix → the Agent runs in a designated environment → the user opens the same environment in CodeZ and takes over → the human's edits become subsequent context → the Agent continues the same task → the user reviews the full change set and verification evidence → applies it or opens a PR → the environment can be safely archived and restored.

The current README's Cursor/Codex analogy explains the entry points but cannot replace a product definition. The official Worktrees documentation shows a chat bound to a stable workspace, Local/Worktree handoff, and snapshot-before-archive with restore; these can serve as a lifecycle design reference without copying the UI: https://learn.chatgpt.com/docs/environments/git-worktrees .

### Promises the product should make to users

1. The user can always see which directory, branch, model and authorization the Agent is running with.
2. Switching tasks or views never changes a task's execution environment.
3. Stop actually stops subsequent actions; reopening explains what was completed, what was not, and what awaits confirmation.
4. Review covers the actual changes, including uncommitted and new files; "no changes" must never mask a read failure.
5. Delete, publish, send, merge and similar actions have an explicit target, status feedback and a recovery strategy.
6. An Agent being "done" means the delivery goal and verification evidence meet the requirements; the model finishing its output is not success by itself.

## Value of the existing foundation

| Area | Existing implementation and its value | Maturity gap |
| --- | --- | --- |
| Desktop host | Tauri 2, Rust commands, PTY, LSP/DAP, browser | Child-process failure recovery, resource governance and platform verification |
| CodeZ | Monaco, Git, search, terminal, language features, inline edit | Real-project benchmarks, extension compatibility evidence, a contract for concurrent human/Agent editing |
| WorkZ | Session tasks, concurrency slots, task cancellation, optional worktree | Durable task–environment binding, restart reconciliation, reliable review |
| Kernel | Pinned shared execution loop, policy gates, tools, context capabilities | Versioned host–kernel contract and fault tests |
| Recovery | Session database, file journal, recovery of unsaved editor buffers | Unified recovery of plan/run state/environment for ordinary tasks |
| Teams | Pool swarm, visual workflows, workflow run JSON | Member isolation, budget allocation, integration acceptance, cross-task resource governance |
| Extensions | Node host that executes extension JS, plus smoke tests | Extension compatibility matrix, dedicated Webview security and functional tests |
| Delivery | lint, typecheck, Vitest, Rust clippy/test CI, multi-platform builds | Signing, updates, migration, post-install run acceptance |

The existing CI is genuinely valuable; the project must not be misjudged as "having no tests". However, CI runs mainly on Ubuntu, and packaging for several platforms is not the same as verifying desktop behaviour on each.

## Code review findings

Priorities: P0 is a high-impact correctness issue that should block wider user delivery; P1 must be solved in the next maturity milestone; P2 is ongoing governance. "Confirmed" below refers to source behaviour; only items explicitly marked as experiments have runtime reproduction evidence.

### F01 / P0: worktree binding is view state, so switching tasks can reuse the wrong environment

Evidence: `src/workspaces/workz/index.tsx:192` kept the worktree in component state; lines 303–304 mapped it to a single global ref; lines 784–789 took the environment for the next turn from that ref; `openTask` around line 978 restored chat and cwd but not the matching worktree. Creating a new task and switching projects cleared that state, and reopening an old task did not restore it from a persisted task record.

Trigger path: isolated task A creates worktree A → creating B clears the worktree → B creates worktree B → open A → send a follow-up message to A. The code still takes worktree B from the current ref. After a restart, opening A may also create another environment instead of returning to A's original one. This is derived from the code path and has not been reproduced through desktop interaction.

Fix: persist environment_id, project_id, path, branch and starting SHA per task; the backend resolves the execution environment from task_id; the frontend is no longer the authority for the run path. Before each turn, validate the owning repository, the registered worktree and the branch.

Acceptance: repeated A/B switching, switching while running, and continuing after restart all produce changes only in each task's own environment; an invalid environment must block and explain, never silently switch directories.

### F02 / P0: branch review misses uncommitted and untracked changes, and read errors appear as empty

Evidence: around `src-tauri/src/commands/agent_task.rs:240`, changed_files used `git diff base...branch`; file_diff used `git show branch:path`. Both can only read committed trees. Git failures were turned into empty results via `unwrap_or_default()`. The WorkZ completion path did not force a snapshot containing all task changes, nor require the task to commit first.

Experiment: in a temporary repository, create a task branch, modify tracked.txt and create new.txt; a branch diff identical to the current review produced empty output, while `git status --short` showed one modification and one new file. The experiment directory was kept at `%TEMP%/agentz-review-5e44bb8f19364728ae98d6b57ee99fb4`.

Fix: review explicitly distinguishes committed diffs, staged, unstaged and untracked; read the actual worktree or a recoverable complete snapshot. Freeze the base SHA and the review snapshot SHA. Git errors must return explicit errors; the allowed "does not exist" cases for new/deleted files are handled separately.

Acceptance: edit-only-without-commit, new-file-only, deletion, rename, binary, paths with special characters and invalid base all produce correct and explainable results.

### F03 / P1: journal write location for isolated tasks differs from where the frontend queries it

Evidence: `src-tauri/src/commands/chat.rs:99` opened the journal using agent_workspace; in isolated mode the journal lives in the worktree's `.agentz/journal.db`. Around `src/workspaces/workz/index.tsx:869`, journalListChanges was called with the main projectDir; `src-tauri/src/commands/journal.rs:18` opened the main-directory journal for that project.

Impact: an isolated task with real file changes may still show an empty journal review after the turn. Combined with the Git branch review gap, this weakens trust in delivery.

Fix: resolve journal storage and file root by task/environment; queries and Undo use the same binding; unify a task artifact API so the UI does not guess storage paths.

### F04 / P1: discarding a task force-deletes with no recovery, and errors are swallowed

Evidence: around `src-tauri/src/commands/agent_task.rs:329`, `worktree remove --force` and `branch -D` ran back to back, both errors were ignored and success was returned. The frontend has a confirmation dialog, so this is a confirmed destructive-design and feedback issue; it should not be described as deletion without confirmation.

Impact: uncommitted files may be permanently lost; a failed deletion may still show the task as resolved; the API did not validate that the given path and branch belong to the task.

Fix: archive by default; before deletion, save a recoverable snapshot including uncommitted and non-ignored new files; state the retention rules for ignored files explicitly. Validate the registered task environment, execute steps in order and return per-step status; stop and wait for a running task first. Make permanent deletion a separate operation.

### F05 / P1: task creation and merge have implicit workspace side effects

Evidence: ensure_git_repo ran `git init` automatically; when there was no HEAD it ran `git commit --allow-empty`. If content was already staged, that commit could include the staged files. The default base was the current branch name, not a commit frozen at creation; a new worktree does not contain uncommitted changes from the main directory. Merge checked out base directly in the user's main directory and then did a no-ff merge; the success path did not restore the original branch, and restoring on failure was best effort; there was no repository-level transaction/mutex or complete pre-check.

Fix: non-Git projects use an explicit file-snapshot environment; creating Git management becomes an explicit project setting. Offer "branch commit" and "include current uncommitted work" as starting options and record the SHA. Merge uses an explicit target and a workspace mutex, and pre-checks dirty files, current runs, conflicts and HEAD changes; do not let Agent reasoning decide these low-level guarantees.

### F06 / P1: default isolation does not match what is advertised; a worktree is not a system sandbox

Evidence: `src/workspaces/workz/index.tsx:190` enabled isolation only when the localStorage value was 1, so it was off on first run, while the README described task isolation as a standard capability. The execution registry includes side-effecting tools such as shell; a Git worktree provides a code copy but cannot restrict shell access to other directories, the network or the desktop.

Fix: show the run target clearly in the input area; prefer isolated environments for autonomous coding tasks. Build actual enforcement boundaries for file scope, processes, network, desktop and connector permissions; make "permissive permissions" a configuration with an explicit scope. README guarantees of the "cannot damage the main workspace" kind should be rewritten accurately.

### F07 / P1: the Webview implementation conflicts with the global CSP and leaves isolation risks

Evidence: `src/extensions/ui/WebviewHost.tsx:35` injected an inline script; line 52 used a srcDoc iframe with allow-scripts and allow-same-origin. `src-tauri/tauri.conf.json:26` set frame-src none and script-src self.

Functional risk: the CSP at that time conflicted with what the iframe/inline shim expected, so extension Webviews need real Tauri verification. Security risk: if the CSP were simply opened up, a same-origin srcdoc that allows both scripts and same-origin weakens iframe isolation; unsafe-inline must not be enabled just to make it work. Exploitability in the then-current build was not confirmed.

Fix: a separate origin or removal of the same-origin permission, independent state per panel, nonce CSP, controlled resource mapping and a strict messaging protocol. Add two kinds of acceptance: "can render" and "cannot access host DOM/IPC/other panels".

### F08 / P1: credentials are encrypted, but encryption failure may fall back to plaintext storage

Evidence: in the `527e554` kernel pinned by Cargo.lock, Settings::save skipped encryption when creating the SecretStore failed, encrypt_field kept the original field when encryption failed, and the result was then serialized and written. SecretStore uses ChaCha20-Poly1305 with the key in `.secret_key` in the config directory; this is not "no encryption at all".

Impact: in cases such as a corrupted or unavailable key file, plaintext may still be saved; if the config and key are copied together, local encryption offers limited protection. Settings pass secrets through the frontend; redaction in logs, exports and app_control return values needs a dedicated check.

Positive fact: app_control's get_settings already calls redact_secrets; follow-up verification should check redaction coverage and other data exits, and must not describe that interface as returning raw keys to the model.

Fix: refuse to save when encryption fails; protect the master key with the OS credential store; create keys and config with atomic writes and access control. The credential model should pass only secret references; reading settings returns presence/mask; replace and delete are separate. This needs to be fixed in piscis-engine and released as a new pinned version.

### F09 / P1: ordinary task run state does not yet form a durable unified model

Evidence: AppState's cancellation, permission replies and plan store live in memory; WorkZ runningSessionsRef belongs to the frontend; there are a session database, workspace-state and workflow run JSON, each storing part of the state. Opening history after a restart is not resuming execution.

Another continuity gap: `src/services/tauri/chat.ts:131` defaulted clearPlan=true; WorkZ's chatSend did not override it; backend chat_turn deleted the current session's plan when clear_plan was set. Continuing an ordinary task could clear an unfinished plan; a new goal must be distinguished from continuing the current goal.

Positive fact: the app already checks the number of running sessions on exit and cancels all chats; the editor also saves unsaved buffers. It is wrong to say there is "no exit protection at all" or "no persistence at all".

Fix: a unified backend Task/Run/Attempt state machine that reconciles to real states such as interrupted on startup; persist plans, budgets, pending approval intents, environment and the last event sequence number. After a restart, start a new attempt from a checkpoint, reconcile external actions first, and never blindly replay non-idempotent tools such as send/publish.

### F10 / P2: core components carry too many responsibilities, with scattered state

Evidence: at review time the WorkZ index was about 1869 lines, the CodeZ index 1360 lines and chat_turn.rs 2668 lines. WorkZ simultaneously managed event routing, streaming UI, task state, workspaces, teams and review; several refs, localStorage and databases each acted as different authorities.

Fix: separate TaskService, EnvironmentService, EventStore, ApprovalService and ArtifactService around the domain; the frontend keeps presentation state and the backend holds execution authority. Extract incrementally along the discovered problems such as environment mix-ups; a large framework rewrite is unnecessary.

### F11 / P1: quality gates cannot yet prove user journeys are reliable

The existing 7 frontend test files / 38 tests mainly covered LSP, workflow expressions and API, i18n, path filtering and the extension UI store; Rust CI has workspace clippy/test, and the extension host has a smoke test. No complete desktop E2E covering "create isolated task → run in parallel → reopen → review → apply/archive" was found.

Fix: add backend integration tests and desktop acceptance for high-risk lifecycles; run post-install verification on real Windows/macOS/Linux. Build an extension compatibility matrix that records "compatible with extension X version Y" from real runs; a complete set of API names is not proof of compatibility.

### F12 / P1-P2: release and documentation governance are still at preview-product level

README and RELEASE.md stated that builds are unsigned and updated manually; the tauri config provided no updater flow. Documentation on versions, log directories, timeouts and auto-continue contained historical descriptions. The kernel at that time already had exponential-backoff retries, a 60-second permission wait and tool timeout paths; chat_turn's default total time budget was about one year, so the old "default 600 seconds" issue did not apply to the source at the time.

Fix: signing, notarization, update verification, data migration tests, rollback-capable upgrades, diagnostic export; add version and closure rationale to old issues. Before release, align versions and user docs and distinguish "implemented", "verified" and "planned".

## Recommended architecture

Keep Tauri + React + Rust + the Node host. There is no evidence that switching to Electron or another editor would solve the main current problems.

Establish the following entities and references:

- Project: repository/non-repository identity and working root.
- Task: user goal, acceptance criteria, context and stable identity.
- Environment: host, path, starting SHA, worktree, permission scope and archive snapshot.
- Run/Attempt: one execution, model/policy snapshot, resource budget and lifecycle.
- Approval: action, target, parameter summary, validity period, authorization scope, execution status.
- Artifact/ChangeSet: artifacts, file changes, verification results and publishable versions.
- Event: task/run/attempt/environment/tool_call identifiers, monotonically increasing sequence number and a resumable cursor.

A Task can have multiple Runs; each Run references a definite Environment. A session is an interaction record; neither the session source label nor current panel state should carry the task run contract.

Recommended run states: queued, running, waiting_approval, waiting_input, paused, interrupted, failed, cancelled, succeeded. Delivery state is represented separately: draft, needs_review, accepted, applied, published, archived. Finishing execution does not automatically mean finishing publication.

The Agent Runtime manages queueing, cancellation, budgets, retries and recovery under host control, and CodeZ/WorkZ read it through commands and events. Initially it can be an internal Rust service; introduce an independent background process only when execution must continue after the app closes, independent upgrades are needed, or strong fault isolation is required.

## Concrete maturity work for CodeZ and WorkZ

### CodeZ

The primary goal is sustainable use by a person in their own project: conflicts between dirty buffers and Agent writes are explainable; external edits never overwrite unsaved content; language-service crashes are recoverable; Git operations and the Agent do not contend for the same workspace. Every editor, terminal, debugger and preview should show its environment rather than relying only on a global projectDir.

Describe extension support in tiers: verified, partially supported, unverified. First guarantee the language, formatting, Git and debugging extensions users actually rely on; test compatibility regressions after upgrades. Sharing a list of VS Code API names does not prove full behavioural compatibility.

### WorkZ

By default the input area shows only the goal, run environment, model and authorization summary; advanced skills and teams expand on demand. During execution show the phase, the most recent real action, blocking reasons and budget; streaming tokens should not stand in for meaningful progress.

The first screen of the review page answers: what changed, where the artifacts are, what was verified, what was not, and what the user needs to do. Tool logs expand when needed. Research and documentation tasks use openable/versioned artifacts rather than just a final block of text.

Multi-Agent should serve verifiable gains: parallel read-only research, implementation in independent workspaces, and a clear integration owner. Budgets are controlled for the whole task, so each member cannot exhaust a full quota separately. Measure a single-Agent baseline first, then prove that teams improve success rate or time.

Priority order for desktop operations: purpose-built local tools/connectors → structured browser control → native desktop operations. Desktop operations should define the usable apps, focus, user preemption, credential zones and failure recovery; adding a screenshot tool does not complete a Desktop Agent.

## Maturity roadmap and acceptance

Organized by milestone below, without committing to dates that lack a team-size basis.

### M0: restore correctness and trustworthy review

Scope: F01–F05 — task environment registration, complete change review, journal routing, error feedback, archive snapshots and merge pre-checks.

Acceptance: two tasks continued alternately and across restarts never mix environments; uncommitted and new files both enter review; an invalid ref shows an error; archives restore all data declared as supported; the main workspace's dirty state and conflicts are never changed unexpectedly.

### M1: build the foundation for long-running tasks

Scope: durable Task/Run/Environment/Event, recovery reconciliation, cancellation confirmation, resource and cost budgets, permission intents, fail-closed secret storage.

Acceptance: after process kill, network loss, 429/5xx, stuck tools, waiting for the user, and sleep/wake, the real state is shown and the task can safely continue. Retries of external side effects are deduplicated/reconciled, with no duplicate sends or publishes.

### M2: same-task handoff between the two views

Scope: WorkZ→CodeZ opening the task environment, feedback from human edits, checkpoints, structured artifacts and verification results, consistent workbench interaction.

Acceptance: the whole journey of fixing a real bug requires no manual guessing of directories or branch switching; the user can take over, continue, review, apply and undo. Additionally complete artifact delivery for one non-code research/documentation task.

### M3: controlled desktop capabilities and a supportable distribution

Scope: platform install acceptance, signing/updates/migration/recovery, connector authorization, Webview, remote reconnection, verified desktop tools.

Acceptance: an end-to-end record from installation to the first successful delivery; failed upgrades and migrations are recoverable; extensions/remote/desktop operations have compatibility and failure lists.

### M4: expand teams and automation

Scope: team governance on top of the run foundation above, scheduled tasks, notification inbox, templates and governance of marketplace resource sources.

Acceptance: quantified gains over the single-Agent baseline; automation does not repeatedly disturb the user when nothing has changed; activity is bounded by whole-task budgets and authorization boundaries.

## Recommended measurement and release gates

Build a fixed task set that separates model capability from host failures. Include at least a frontend bug, a Rust change, a test fix, a refactor, code research, a non-code report and a browser task; record model and version, environment, cost, human intervention and verification evidence.

Priority metrics: task environment mix-ups / unrecoverable data loss (zero tolerance in testing); first-time successful delivery rate; interruption recovery rate; number of human interventions; total cost per task; leftover processes after cancellation; time to first visible progress; review accuracy.

Establish baselines before setting performance budgets. Measure cold start and time-to-interactive, memory broken down by idle/editor/chat/LSP/extensions/browser, long-conversation responsiveness, indexing peak and two-hour resource growth; with a single WebView, Monaco, Markdown, graph layout and extension RPC contend for the render thread, so decide on Workers, lazy loading or service splitting based on measurements.

Recommended 1.0 gate: high-impact gaps in M0–M3 closed; core journeys pass on all three platforms; signed releases and update recovery verified; internal results of real-task benchmarks published; no unexplained environment mix-ups, review omissions or unrecoverable losses. A version number alone is not proof of maturity.

## Check log for this review

- `npm run typecheck`: passed.
- `npm test`: 7 test files, 38 tests passed.
- `npm run lint`: 0 errors, 65 warnings; some Hook dependency warnings need individual judgement and cannot all be treated as pure style issues.
- `npm run build`: passed; the Vite build took about 7 min 8 s for 4132 modules. The main index JS was about 2.96 MB and the Monaco chunk about 3.83 MB (both minified, not gzipped); there were large-chunk and mixed static/dynamic import warnings. A sampled working set of the build process exceeded 3 GB; this is not a benchmark of desktop app runtime memory or cold start and must be measured separately.
- `npm --prefix extension-host run typecheck` / `smoke`: passed; the sample extension's command, message and completion assertions passed; this does not imply all third-party VSIX are compatible.
- `cargo check -p agentz-desktop --locked`: passed, about 6 min 39 s; reported version 0.9.5 at startup. Local Rust test/clippy were not run, so cargo check must not be described as full Rust verification.
- Experiment showing branch review missing uncommitted/new files: reproduced.
- Desktop UI, real model, remote and installer run acceptance: not performed.

This report provides review findings and implementation recommendations and did not mark any finding as fixed at the time (see the status note at the top for later changes). Implement M0 first, using one reliable task loop as the foundation for further maturity work.
