# AgentZ 1.0.69 release checklist

2026-11-16. Based on 1.0.68: database-open freeze fix, transient network retry, full turn error chain.

## Release gate

- [x] Version labels are 1.0.69 (package, lockfiles, Cargo, Tauri, README, RELEASE).
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.69.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passed.
- [x] Kernel tests for `store::db`, `llm::error_class`, `agent::loop_` passed (130).
- [ ] Frontend unchanged since 1.0.68 (typecheck/vitest not re-run).

## CI

- [ ] The `v1.0.69` Release workflow produces all installers (Linux, macOS, Windows x64 + ARM64).
