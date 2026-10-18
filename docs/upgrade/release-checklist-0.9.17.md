# AgentZ 0.9.17 发布清单

2026-10-19。目标：WorkZ 会话 LED 与 `status` 和真实在途回合保持一致。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README、RELEASE 均为 0.9.17。
- [x] CHANGELOG 增加 0.9.17 条目。
- [x] `chat_turn.rs`：`SessionRunningGuard` 保证 `running` 必收尾。
- [x] `session.rs`：列表时对账并修复无在途闸门的陈旧 `running`。
- [x] `task_activity.rs`：`is_active` 供对账使用。
- [x] `WorkzSidebar.tsx`：仅对 `runningIds` 脉冲。
- [x] `cargo test -p agentz-desktop --lib heals_running` / `same_task_excludes`：通过。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 发布决策

- 2026-10-19，用户要求提交并发布。验收门禁仍未通过，沿用 0.9.12–0.9.16：不接入 `release-readiness`，通过 tag `v0.9.17` 直接构建安装包。
- 该决定只覆盖 0.9.17，不改变 `RELEASE.md` 对后续版本的门禁要求。

## 未完成（已知）

- [ ] 用户验收：停止后 LED 熄灭/变绿；重启后陈旧 `running` 被列表修复。
- [ ] 发布门禁、安装包签名（AZ-064）、macOS 与 Linux 真机验证。

## 发布方式

- 通过 tag `v0.9.17` 触发 `release.yml`。
