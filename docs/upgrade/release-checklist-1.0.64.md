# AgentZ 1.0.64 release checklist

2026-11-11. Based on 1.0.63: project hooks execute, built-in browser text selection and tab close, bundled content aligned with AgentZ.

## Release gate

- [x] Version labels are 1.0.64.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.64.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.
- [x] `tsc`, `vitest`, `cargo test` (agentz-desktop) passed.
- [x] `node scripts/audit-bundled.mjs` and `node scripts/verify-preinstall.mjs --strict` passed.

## CI

- [ ] The `v1.0.64` Release workflow produced Linux x86_64, Linux aarch64, macOS aarch64, macOS Universal, and Windows installers.

