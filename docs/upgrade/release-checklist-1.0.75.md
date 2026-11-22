# AgentZ 1.0.75 release checklist

2026-11-22. Based on 1.0.74: grep-with-file-path fallback, escalating read-only reminders, kernel pin v0.8.77.

## Release gate

- [x] Version labels are 1.0.75.
- [x] `CHANGELOG.md` and release notes updated.
- [x] Kernel suite 529 passed (`efb4adc`, tag `v0.8.77`).
- [x] `cargo metadata --locked` passes with the new pin.
- [ ] Release workflow for `v1.0.75` (pending).
