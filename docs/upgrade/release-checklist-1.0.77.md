# AgentZ 1.0.77 release checklist

2026-11-25. Based on 1.0.76: mid-stream disconnect retry, open-with-registered-program fix, kernel pin v0.8.78.

## Release gate

- [x] Version labels are 1.0.77.
- [x] `CHANGELOG.md` and release notes updated.
- [x] Kernel suite 530 passed (`9c5cd60`, tag `v0.8.78`); `cargo check` of `src-tauri` clean.
- [ ] Hand-check: click an unpreviewable file with and without a registered program.
- [ ] Release workflow for `v1.0.77` (pending).
