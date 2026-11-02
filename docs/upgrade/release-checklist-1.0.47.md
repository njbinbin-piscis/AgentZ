# AgentZ 1.0.47 发布检查清单

2026-11-03。基于 1.0.46，修复 call_fish 超时。

## 发布门禁

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、README、RELEASE。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.47.md` 已更新。
- [x] `cargo check -p agentz-desktop` 通过。

## 人工验收

- [ ] 让主代理用 call_fish 执行一个超过 150 秒的任务，不再报“执行超时（150秒）”。
