# AgentZ 1.0.66 release checklist

2026-11-13. Based on 1.0.65: app-wide context menus (browser menu suppressed), shared ContextMenu component, per-surface semantic menus, open-file event bridge, defensive theme storage.

## Release gate

- [x] Version labels are 1.0.66.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.66.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.
- [x] Full `vitest` suite passed (91 files / 473 tests, incl. 18 new context-menu tests).
- [x] `tsc --noEmit` passed; `read_lints` on all changed files: 0 errors / 0 warnings.

## CI

- [ ] The `v1.0.66` Release workflow produced Linux x86_64, Linux aarch64, macOS aarch64, macOS Universal, and Windows installers.
