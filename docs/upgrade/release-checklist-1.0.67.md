# AgentZ 1.0.67 release checklist

2026-11-13. Based on 1.0.66: immediate Stop response (frontend + kernel startup checkpoint + tiered cancel grace), context-ring refresh after each tool batch, capped ring label, queued-message image-chip removal while busy, conversation open/switch pins to newest message.

## Release gate

- [x] Version labels are 1.0.67 (package, lockfiles, Cargo, Tauri, README, RELEASE).
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.67.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.
- [x] Full `vitest` suite passed (95 files / 483 tests, incl. new ChatComposer chip-removal, ContextUsageRing cap, useStickToBottom reset and message-window regression tests).
- [x] Kernel `cargo test -p piscis-kernel --lib agent::loop_` passed (69 tests, incl. stop-responsiveness and post-tool-batch ContextUsage regressions). Known environment limitation: `real_shell`/`real_code_run` cancel tests hang in this sandbox (process-teardown environment issue, unrelated to this change); deterministic fixtures cover the cancel-grace logic.
- [x] `tsc --noEmit` passed; `read_lints` on all changed files: 0 errors.

## CI

- [ ] The `v1.0.67` Release workflow produced all installers: Linux x86_64/aarch64 (.deb/.AppImage/.rpm), macOS aarch64/Universal (.dmg/.app.tar.gz), Windows x64 + ARM64 (NSIS .exe); assets verified on the public release.