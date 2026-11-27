# AgentZ 1.0.79 release checklist

2026-11-27. Based on 1.0.78: git panel live refresh, discard-icon fix, auto-clear pending review after commit.

## Release gate

- [x] Version labels are 1.0.79.
- [x] `CHANGELOG.md` and release notes updated.
- [x] Kernel pin unchanged (`v0.8.79`); `tsc` clean; `GitPanel` + `src/components` tests pass.
- [ ] Hand-check: agent commit clears git panel and Changes drawer.
- [ ] Release workflow for `v1.0.79` (pending).