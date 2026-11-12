# AgentZ 1.0.65 release checklist

2026-11-12. Based on 1.0.64: Koi state-machine reliability (auto-retry, watchdog cancel-before-reset, shared runtime, board reconciliation, patrol coverage), browser dead-process recovery and desktop UA for agent sessions, agent tooling hardening.

## Release gate

- [x] Version labels are 1.0.65.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.65.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.
- [x] `vitest` (workz 51 tests, e2e agentz.spec) and `cargo test` (kernel pool/coordinator/output/web_fetch/plan_doc/file_write, ide-tools lsp, agentz-desktop runtime/commands) passed.
- [x] `cargo check -p agentz-desktop --lib` passed.

## CI

- [ ] The `v1.0.65` Release workflow produced Linux x86_64, Linux aarch64, macOS aarch64, macOS Universal, and Windows installers.
