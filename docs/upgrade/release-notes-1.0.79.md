# AgentZ 1.0.79 release notes

Release date: 2026-11-27. See the 1.0.79 entry in `CHANGELOG.md`.

## Fixed

- After an agent commits, the Source Control panel refreshes instead of keeping the committed files listed as dirty.
- Discard-change buttons no longer use a refresh-like `↺` icon; they use a trash glyph so they are not mistaken for refresh.
- Pending agent-change entries in the Changes drawer are auto-accepted when their files leave the dirty git set (typically after a commit).

## Known limitations

- An agent that edits and commits in the same turn without an intervening git refresh may leave a pending entry; keep it manually.
- Auto-clear and panel refresh were verified by compilation/tests, not hand-checked after a live agent commit.