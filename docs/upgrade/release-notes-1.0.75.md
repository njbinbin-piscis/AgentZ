# AgentZ 1.0.75 release notes

Release date: 2026-11-22. See the 1.0.75 entry in `CHANGELOG.md`.

## Fixed

- `file_search` grep accepts a file path and searches that file.

## Changed

- Read-only streak reminders repeat every 25 calls and escalate to demand a concrete action; they never ask the user a question.
- Kernel pin advanced to piscis-engine `v0.8.77` (commit `efb4adc`).

## Known limitations

- The reminders are advisory; a model can still ignore them. Nothing is blocked by design (long research tasks legitimately read many files).
