# AgentZ 1.0.80 release notes

Release date: 2026-11-27. See the 1.0.80 entry in `CHANGELOG.md`.

## Fixed

- Dead-loop control now escalates before the next LLM request: warn on the third identical unchanged call, refuse that call on the fourth, and end the turn on the fifth with a handoff note (completed / stuck / tried / next).
- Loop reminders are recognized as `[系统提醒]` again, so they are cleaned from the request view instead of counting as real user turns.
- Repeated identical tool rounds are collapsed in the request view (real result kept, refusals and intervening reminders dropped), which also avoids provider 400s on repetitive tool-call histories.
- Poll/wait-like calls are left alone until eight unchanged results; rotating read-only re-reads stay with the existing read guard.

## Known limitations

- `call → sleep → call → sleep` with an unchanged result still escalates only at the long global threshold, so normal wait-for-build patterns are not cut short.
- Edit-then-rerun cycles that keep failing the same way warn and refuse once per eight identical outcomes, but do not end the turn by themselves.
- Behavior was verified by kernel and UI unit tests, not hand-checked against a live overnight agent run.
