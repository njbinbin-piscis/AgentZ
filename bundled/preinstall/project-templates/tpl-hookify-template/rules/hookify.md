# Lifecycle Hooks & Automation Guardrails

When configuring project automation, scripts, and `.agentz/hooks.json` in this project:

1. **Fast, Deterministic Hooks**:
   - A hook is a shell command with an `event` (`beforeAgentTurn`, `afterAgentTurn`, `beforeFileEdit`, `afterFileEdit`). It must be non-interactive, idempotent, and finish within 10 seconds (it is killed after that).
   - Today only `beforeAgentTurn` hooks are executed by the agent runtime: their stdout is injected into the system prompt, so keep the output short and high-signal. The other events can be saved but are not run automatically yet.
2. **Post-Edit Static Verification**:
   - After substantive code edits, always invoke `read_lints` on the modified files before concluding the turn.
