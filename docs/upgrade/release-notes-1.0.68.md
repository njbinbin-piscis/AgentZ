# AgentZ 1.0.68 release notes

Release date: 2026-11-16. See the 1.0.68 entry in `CHANGELOG.md`.

## Fixed

- **chat_ui choices can be selected (#23).** Options are normalized in the frontend and the backend; `options` is required by the schema; wizard step changes keep entered values.
- **Fork menu is no longer clipped (#24).** Portal + fixed positioning, flips when there is no room below.
- **Immediate Stop winds down cleanly (#25).** Read-only tools are abandoned without counting as failures, write tools get an 8 s grace, a checkpoint is written before cancelling, the session returns to idle; CodeZ and WorkZ show a "stopping" state and a `cancelled` tool state.
- **Context compaction no longer loops or loses the goal (#26).** Target bound to the trigger line, idle-compaction guard with cooldown, deterministic fallback summary that pins the user's request and read files, larger previews and summary limits, path-level re-read detection with a reminder and pause, host history budget minus measured static overhead.
- **Todo panel "Clear" persists (#8).** New `cancel_open_items` plan intent; unfinished items of a task that is not running are shown as "Not finished" without a spinner.
- **Kernel `notice` events are displayed** as a localized toast in CodeZ and WorkZ.

## Known limitations

- #23-#26 and #8 are covered by automated tests; they were not hand-verified in a running app on a second machine.
- Context ring may exceed 100% when the static prompt and tool definitions alone exceed the budget; the app now warns about it instead of looping.
