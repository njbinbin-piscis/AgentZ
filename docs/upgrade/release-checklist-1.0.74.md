# AgentZ 1.0.74 release checklist

2026-11-22. Based on 1.0.73: remove the 75-call hard pause, prune resolved-turn noise, correctable tool errors, tool statistics, kernel pin v0.8.76.

## Release gate

- [x] Version labels are 1.0.74.
- [x] `CHANGELOG.md` and release notes updated.
- [x] Kernel suite 528 passed in the piscis-engine repo (`5281fc6`, tag `v0.8.76`); `cargo check` of `src-tauri` clean.
- [x] `cargo metadata --locked` passes with the new pin.
- [ ] Release workflow for `v1.0.74` (pending).
