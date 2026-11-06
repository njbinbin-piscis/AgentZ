# AgentZ 1.0.57 release checklist

2026-11-06. Based on 1.0.56: coerce restored form and markdown values so the wrong JSON type cannot crash render.

## Release gate

- [x] Version labels are 1.0.57.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.57.md` are updated.
- [x] `npx tsc --noEmit` passed.

## CI

- [ ] The `v1.0.57` Release workflow produced Linux x86_64, Linux aarch64, macOS aarch64, macOS Universal, and Windows installers.
