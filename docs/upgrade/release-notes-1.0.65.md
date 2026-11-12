# AgentZ 1.0.65 release notes

Release date: 2026-11-12. See the 1.0.65 entry in `CHANGELOG.md`.

## Fixed

- **Interactive forms keep selections while the agent is streaming.** The chat view re-parses a form's UI definition on every streaming chunk; the card's initialization keyed on object identity and wiped in-progress radio/text selections each time, so options appeared unclickable and submit failed validation. Initialization now keys on the definition's content. Regression tests cover real-DOM clicks and streaming re-renders.
- **Koi team reliability (state machine audit follow-up).**
  - Transient Koi turn failures (timeout / connection reset / 5xx) are now requeued automatically with exponential backoff (30s·2^n, default 2 retries, configurable via `CoordinatorConfig.max_auto_retries`) instead of blocking the board forever. The error classifier also recognizes "timed out" (with a space) as transient.
  - The patrol watchdog now cancels live turns bound to stale todos *before* resetting the board, so a re-dispatch can no longer race a still-running turn on the same worktree. All dispatchers (chat, patrol, workflows, member tools) share one process-wide Koi runtime, so cancel/wait reach the turn that actually runs.
  - `wait_koi_turn` outcomes are re-readable (cached) instead of consumed once; dropped unclaimed outcomes are logged.
  - The collaboration board shows a dedicated **Needs review** column and reconciles on a 15s poll in addition to events; stale `busy` member badges heal to `idle` on read when the board has no in-progress work.
  - Patrol coverage: every team mode gets a patrol (not only swarm), resuming a paused pool restarts it, and restoring a project with pools re-arms recovery after an app restart. The startup sweep uses the real staleness threshold; the kernel watchdog refuses unconditional resets. Paused pools no longer cause the patrol to retire.
  - Workflow nodes default to a 600s task timeout instead of waiting forever on a hung LLM call.
- **Built-in browser recovers from a dead Chromium process.** When the Chromium process died (crash, external kill, system sleep), the manager kept a stale handle and every later call hung until "browser did not open a page in time", with no relaunch until the IDE restarted. Page acquisition now drops the dead instance and retries once with a fresh browser; errors report both the first and retry failures.
- **Desktop User-Agent applied to agent-only browser sessions.** The override previously only ran on panel viewport sync; headless agent sessions leaked `HeadlessChrome/<v>` and could be served mobile/bot-gated layouts. It is now applied at page acquisition for every session.

## Changed

- **Kernel pin advanced to piscis-engine v0.8.74** (authored by AgentZ): carries the Koi reliability and tooling hardening work upstream; AgentZ-only overlays remain in `vendor/piscis-engine`. Provenance recorded in `AGENTZ-BASELINE.md`.
- **AgentZ added as a contributor** (`.mailmap` + README): the product's own agent improves itself.
- **Agent tooling hardening (self-hosted friction fixes).**
  - `shell` accepts `timeout_secs` / `timeout_ms` aliases (previously silently ignored, falling back to 120s); Windows max timeout raised 300s → 600s. PowerShell wrappers keep `ErrorActionPreference=Continue` and propagate the real exit code, so native-command stderr (git progress, npm notices) no longer flips success to failure.
  - Tool-output spill files under `.agentz/logs/tool-output/` are readable again (the guard previously blocked the very path the tool result tells the agent to read).
  - `web_fetch` gains a `raw` option that keeps long lines intact for base64 / minified JSON payloads.
  - `file_edit` matches snippets in mixed-EOL files by trying both line-ending styles and converts the replacement to the matched region's style.
  - `plan_write` validation reports every missing field at once and its success message directs the agent to the Build panel; `file_diff` documents that `new_content` is a full-file replacement.
  - LSP discovery distinguishes "rustup proxy present but component missing" from "not installed"; devenv probes project-local `node_modules/.bin` and npm's configured global prefix, and reports why a found binary failed to run.
