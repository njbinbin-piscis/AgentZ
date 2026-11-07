# AgentZ 1.0.60 release checklist

2026-11-07. Based on 1.0.59: keep a timed-out CodeZ `chat_ui` form in the reply instead of the session footer.

## Release gate

- [x] Version labels are 1.0.60.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.60.md` are updated.
- [x] `npx tsc --noEmit` passed.

## CI

- [ ] The `v1.0.60` Release workflow produced Linux x86_64, Linux aarch64, macOS aarch64, macOS Universal, and Windows installers.
