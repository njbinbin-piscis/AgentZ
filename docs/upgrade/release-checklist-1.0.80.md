# AgentZ 1.0.80 release checklist

2026-11-27. Based on 1.0.79: dead-loop strike ladder, request-view collapse, handoff on third ineffective attempt, kernel pin `v0.8.80`.

## Release gate

- [x] Version labels are 1.0.80.
- [x] `CHANGELOG.md` and release notes updated.
- [x] Kernel pin advanced to `v0.8.80`; `cargo check` (src-tauri), `tsc`, kernel loop/message_utils/harness tests, and `agentNotice` vitest pass.
- [ ] Hand-check: identical stuck tool call warns, refuses, then ends with a handoff note / `loop_halted` toast.
- [ ] Release workflow for `v1.0.80` (pending).
