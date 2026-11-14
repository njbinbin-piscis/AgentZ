# AgentZ 1.0.67 release notes

Release date: 2026-11-13. See the 1.0.67 entry in `CHANGELOG.md`.

## Added

- **Context-usage ring updates during tool execution.** The kernel re-emits `ContextUsage` after each tool batch via a shared `emit_context_usage` helper (post-compaction request view, same estimation as the iteration-start event). Previously the ring froze at the iteration-start snapshot for the whole tool run — a several-hundred-call tool batch could sit at a stale (even >100%) value for minutes. Regression test: `context_usage_reemitted_after_tool_batch` (kernel `agent::loop_`).

## Fixed

- **Stop responds immediately (CodeZ / WorkZ / BotZ).** The click now flips the UI out of the running state and discards unflushed stream deltas at once instead of waiting for the backend to unwind (tool teardown + drain previously took seconds to tens of seconds). The authoritative `agent_final` / send resolution still reconciles state afterwards.
- **Stop during startup prevents the first model call.** The kernel agent loop checks the cancel flag on entry (`stop_before_start_never_calls_the_model`).
- **Tiered cancel grace per tool.** New `Tool::observes_cancel()`: self-tearing-down tools (shell) keep the 8s cooperative grace and their own results (`cooperative_teardown_tool_still_gets_the_full_grace`); tools that ignore cancel are dropped after 1s (`uncooperative_tool_stops_promptly_without_the_long_grace`), so a blocked tool cannot hold Stop for 8s.
- **Queued-message editing can delete image chips.** The remove button was gated on `!busy`, but editing a queued message happens while a turn is running, so the button never rendered and pasting a new image replaced the old one. Chips are part of the next draft; removal now works while busy (`ChatComposer.test.tsx`, busy + idle cases).
- **Context ring label is capped at 100%.** The stroke used the capped percentage but the label printed the uncapped estimate (e.g. 123% with a full ring). Label and ring now agree; the true estimate/budget stays in the tooltip (`ContextUsageRing.test.tsx`).
- **Conversation open/switch lands on the newest message.** `useStickToBottom` gains a `resetKey` (session / task / pool / bot id) that re-arms follow and pins to bottom on change; the CodeZ message list resets its lazy render window per session; the IM messages panel, collaboration board feed, pool activity feed and bot home adopt it. A bot without a home session clears the previous bot's transcript instead of keeping it on screen.