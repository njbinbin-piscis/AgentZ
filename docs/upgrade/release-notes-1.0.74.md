# AgentZ 1.0.74 release notes

Release date: 2026-11-22. See the 1.0.74 entry in `CHANGELOG.md`.

## Fixed

- **Long research tasks no longer stop at 75 read-only calls.** The hard pause introduced in 1.0.73 is removed; reminders at 25 and 50 remain.

## Changed

- Earlier turns' correctable failures, guard blocks and system reminders are dropped from what the model sees once the user moves on.
- Correctable tool errors carry detailed help and are pruned after a successful retry; `file_search` has a minimal description.
- Per-tool call statistics (`tool-stats.json`, `tool_stats_snapshot`).
- Kernel pin advanced to piscis-engine `v0.8.76` (commit `5281fc6`).

## Known limitations

- Only `file_search` was slimmed; other long tool descriptions (`pool_org`, `app_control`, `lsp`, ...) wait for the collected statistics.
- The statistics have no UI yet.
