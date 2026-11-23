# AgentZ 1.0.76 release checklist

2026-11-23. Based on 1.0.75: external dropped-file path fix and file-manager reveal fix.

## Release gate

- [x] Version labels are 1.0.76.
- [x] `CHANGELOG.md` and release notes updated.
- [x] Frontend tests for `absPathToProjectRel` pass; `tsc` clean; `cargo check` of `src-tauri` clean.
- [ ] Hand-check drag-in of an external file and right-click "open in file manager" in the running app.
- [ ] Release workflow for `v1.0.76` (pending).
