# AgentZ 1.0.66 release checklist

2026-11-13. Based on 1.0.65: app-wide context menus (browser menu suppressed), shared ContextMenu component, per-surface semantic menus, open-file event bridge, defensive theme storage.

## Release gate

- [x] Version labels are 1.0.66.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.66.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.
- [x] Full `vitest` suite passed (91 files / 474 tests, incl. 18 new context-menu tests and the icon-glyph regression test).
- [x] `tsc --noEmit` passed; `read_lints` on all changed files: 0 errors / 0 warnings.

## CI

- [x] The `v1.0.66` Release workflow (run #43429194091, completed successfully 2026-11-13, includes the edge-panel icon fix) produced all installers: Linux x86_64 (.deb/.AppImage/.rpm), Linux aarch64 (.deb/.AppImage/.rpm), macOS aarch64 (.dmg/.app.tar.gz), macOS Universal (.dmg/.app.tar.gz), Windows x64 + ARM64 (NSIS .exe) — 12 assets on the public release, download verified (HTTP 200 after CDN redirect, 99.4 MB x64 installer).
