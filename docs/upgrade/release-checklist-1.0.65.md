# AgentZ 1.0.65 release checklist

2026-11-12. Based on 1.0.64: Koi state-machine reliability (auto-retry, watchdog cancel-before-reset, shared runtime, board reconciliation, patrol coverage), browser dead-process recovery and desktop UA for agent sessions, agent tooling hardening.

## Release gate

- [x] Version labels are 1.0.65.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.65.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.
- [x] `vitest` (workz 51 tests, e2e agentz.spec) and `cargo test` (kernel pool/coordinator/output/web_fetch/plan_doc/file_write, ide-tools lsp, agentz-desktop runtime/commands) passed.
- [x] `cargo check -p agentz-desktop --lib` passed.

## CI

- [x] The `v1.0.65` Release workflow (run #162, completed successfully 2026-11-13) produced all installers: Linux x86_64 (.deb/.AppImage/.rpm), Linux aarch64 (.deb/.AppImage/.rpm), macOS aarch64 (.dmg/.app.tar.gz), macOS Universal (.dmg/.app.tar.gz), Windows x64 + ARM64 (NSIS .exe) — 12 assets on the public release, download verified (HTTP 200, 99.4 MB x64 installer).
