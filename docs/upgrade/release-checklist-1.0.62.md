# AgentZ 1.0.62 release checklist

2026-11-08. Based on 1.0.61: WorkZ queue and running-state fixes, per-session cancel, BotZ waits behind the session gate, sub-agent and browser tab hardening.

## Release gate

- [x] Version labels are 1.0.62.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.62.md` are updated.
- [x] `node scripts/upgrade-version-check.mjs` passed.

## CI

- [ ] The `v1.0.62` Release workflow produced Linux x86_64, Linux aarch64, macOS aarch64, macOS Universal, and Windows installers.
