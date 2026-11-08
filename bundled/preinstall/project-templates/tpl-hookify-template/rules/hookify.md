# Lifecycle Hooks & Automation Guardrails

When configuring project automation, scripts, and `.agentz/hooks.json` in this project:

1. **Fast, Deterministic Hooks**:
   - A hook is a shell command with an `event` (`beforeAgentTurn`, `afterAgentTurn`, `beforeFileEdit`, `afterFileEdit`). It must be non-interactive, idempotent, and finish within 10 seconds (it is killed after that).
   - `beforeAgentTurn` stdout is injected into the system prompt, so keep it short and high-signal.
   - A `beforeFileEdit` hook that exits with a positive status blocks that edit; print the reason to stderr. `afterFileEdit` and `afterAgentTurn` hooks run for side effects only (formatters, linters, notifiers).
2. **Post-Edit Static Verification**:
   - After substantive code edits, always invoke `read_lints` on the modified files before concluding the turn.
