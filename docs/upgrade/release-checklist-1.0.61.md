# AgentZ 1.0.61 release checklist

2026-11-07. Based on 1.0.60: show live sub-agent work, keep browser tabs on their product surface, describe the real command sandbox, and write PowerShell scripts with a UTF-8 BOM.

## Release gate

- [x] Version labels are 1.0.61.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.61.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.

## CI

- [ ] The `v1.0.61` Release workflow produced Linux x86_64, Linux aarch64, macOS aarch64, macOS Universal, and Windows installers.
