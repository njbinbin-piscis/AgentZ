# AgentZ 1.0.76 release notes

Release date: 2026-11-23. See the 1.0.76 entry in `CHANGELOG.md`.

## Fixed

- Dropped or picked files outside the project keep their full path, so the agent can locate them.
- "Open in file manager" opens the clicked folder (or selects the clicked file) instead of the default folder on Windows.

## Known limitations

- Both fixes were verified by unit tests / compilation only, not hand-checked in the running app.
- Kernel pin unchanged (piscis-engine `v0.8.77`).
