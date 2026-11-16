# AgentZ 1.0.70 release checklist

2026-11-16. Based on 1.0.69: extension install command, extension host start retry, Linux keyring unlock prompt.

## Release gate

- [x] Version labels are 1.0.70 (package, lockfiles, Cargo, Tauri, README, RELEASE).
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.70.md` are updated.
- [x] `npx tsc --noEmit` passed.
- [ ] `node scripts/upgrade-version-check.mjs` re-run after docs.
- [ ] Rust change (removed `agentz-no-prompts` feature) not compiled locally; verified by CI only.
- [ ] Linux keyring unlock not tested on a real Linux desktop.

## CI

- [ ] The `v1.0.70` Release workflow produces all installers (Linux, macOS, Windows x64 + ARM64).
