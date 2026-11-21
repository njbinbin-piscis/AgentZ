# AgentZ 1.0.73 release notes

Release date: 2026-11-21. See the 1.0.73 entry in `CHANGELOG.md`.

## Fixed

- **Loop safety (#37).** One ignored loop warning now escalates to a block; an explore-only streak of 75 read-only calls pauses the run.
- **`file_search` / `file_read` feedback.** Truncation and searched-scope notes, deterministic ordering, EOF-offset error, 400-line cap without `limit`, richer demoted receipts.
- **`code_run` pre-launch detection** no longer misfires on quoted data.

## Changed

- Kernel pin advanced to piscis-engine `v0.8.75` (commit `22ae2df`).

## Known limitations

- The read guard is per-run, so a user "continue" resets its streak.
- Not hand-verified in the running app; verified by kernel tests (525 passed).
