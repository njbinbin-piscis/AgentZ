# AgentZ 1.0.73 release checklist

2026-11-21. Based on 1.0.72: loop-safety and file_search/file_read feedback (#37), code_run false-positive fix, kernel pin v0.8.75.

## Release gate

- [x] Version labels are 1.0.73.
- [x] `CHANGELOG.md` and release notes updated.
- [x] Kernel suite 525 passed in both `vendor/piscis-engine` and the piscis-engine repo (`22ae2df`, tag `v0.8.75`).
- [x] `cargo metadata --locked` passes with the new pin.
- [ ] Release workflow for `v1.0.73` (pending).
