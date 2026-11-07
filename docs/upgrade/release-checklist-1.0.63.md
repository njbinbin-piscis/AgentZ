# AgentZ 1.0.63 release checklist

2026-11-08. Based on 1.0.62: backup-and-confirm for every destructive action, WorkZ fork / restore-to-here replaces the snapshot button, plan ordering fix.

## Release gate

- [x] Version labels are 1.0.63.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.63.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.
- [x] `tsc`, `vitest`, `cargo test` (agentz-desktop, piscis-kernel) passed.

## CI

- [ ] The `v1.0.63` Release workflow produced Linux x86_64, Linux aarch64, macOS aarch64, macOS Universal, and Windows installers.