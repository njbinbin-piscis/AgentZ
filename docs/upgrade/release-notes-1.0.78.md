# AgentZ 1.0.78 release notes

Release date: 2026-11-26. See the 1.0.78 entry in `CHANGELOG.md`.

## Added

- Clicking a pending agent change in the Changes panel opens a red/green diff tab: the file as it was before the agent's first pending edit versus what is on disk now (independent of git).
- Clicking the file name on a `+N -N` edit card in the chat opens that single edit as a diff tab.

## Changed

- Kernel pin advanced to piscis-engine `v0.8.79` (commit `5da4da2`).
- Deleted files in the Changes panel are clickable and show what was removed.

## Known limitations

- The diff tab is a snapshot taken when opened; click the row again to refresh it.
- The chat edit-card diff shows only that edit, not the whole file.
- Not hand-checked in the running app (verified by tests and compilation).