# AgentZ 1.0.68 release checklist

2026-11-16. Based on 1.0.67: known issues #23-#26 and #8, kernel notices shown in the UI.

## Release gate

- [x] Version labels are 1.0.68 (package, lockfiles, Cargo, Tauri, README, RELEASE).
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.68.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.
- [x] `npm run typecheck`, `npm run lint` (0 errors), `npm run lint:preinstall -- --strict` (0 errors).
- [x] Full `vitest` suite passed (103 files / 547 tests).
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passed.
- [x] `cargo test --workspace --no-fail-fast` passed before the final #8 change; after it, the `plan_intent` tests (9) and clippy were re-run.
- [x] Kernel `cargo test -p piscis-kernel --no-fail-fast` passed.

## CI

- [ ] The `v1.0.68` Release workflow produces all installers (Linux, macOS, Windows x64 + ARM64).
