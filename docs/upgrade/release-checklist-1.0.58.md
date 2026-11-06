# AgentZ 1.0.58 release checklist

2026-11-06. Workflow-only follow-up to 1.0.57: move both macOS bundle jobs from the retired `macos-14` runner to `macos-15`.

## Release gate

- [x] Version labels are 1.0.58.
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.58.md` are updated.
- [x] No application source changed, so `npx tsc --noEmit` was not re-run.

## CI

- [ ] The `v1.0.58` Release workflow produced Linux x86_64, Linux aarch64, macOS aarch64, macOS Universal, and Windows installers.
