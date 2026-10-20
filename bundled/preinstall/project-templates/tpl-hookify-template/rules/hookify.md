# Lifecycle Hooks & Automation Guardrails (`hookify`)

When configuring project automation, scripts, and `.agentz/hooks.json` in this project:

1. **Fast, Deterministic Hooks**:
   - Any hook configured in `.agentz/hooks.json` (`beforeAgentTurn`, `afterAgentTurn`, `beforeFileEdit`, `afterFileEdit`) must be non-interactive, idempotent, and complete within 10 seconds.
   - Prefer concise, high-signal output in `beforeAgentTurn` hooks so system context stays compact.
2. **Post-Edit Static Verification**:
   - After substantive code edits, always invoke `read_lints` on the modified files before concluding the turn.
