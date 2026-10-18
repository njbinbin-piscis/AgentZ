# AgentZ 0.9.12 发布清单

2026-10-18。目标：修复 Windows 文件打开与 git 窗口问题，移除聊天窗口中的说明面板，保留一行首次配置提示。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README、RELEASE 均为 0.9.12。
- [x] CHANGELOG 增加 0.9.12 条目。
- [x] 项目目录返回界面时去掉 `\\?\` 前缀（`renderer_path`），内部仍使用规范路径做越界检查。
- [x] git 子进程在 Windows 上隐藏控制台窗口（`agentz-task-state` 的 `git_process`，以及 agent_task、approval 中的 git 调用）。
- [x] 焦点刷新合并（`src/utils/coalesce.ts`），用于任务详情、运行历史、通知收件箱。
- [x] 删除聊天窗口配置摘要与首次运行清单，增加一行首次配置提示，清理对应文案。
- [x] `npx tsc --noEmit` 通过。
- [x] `npx vitest run`：337 个测试通过。
- [x] `cargo test -p agentz-desktop --lib`：362 个测试通过，7 个已标记 ignore。
- [x] `cargo test -p agentz-task-state`：通过。
- [x] `node scripts/upgrade-version-check.mjs`：通过。

## 发布决策

- 2026-10-18，用户明确授权以豁免方式发布 0.9.12：不启用 `release-readiness` 发布门禁（`scripts/upgrade-release-gate.mjs` 未接入 `release.yml`），未通过的台账项在发布说明与 `docs/upgrade/acceptance-0.9.12.md` 中标注。
- 该豁免只适用于 0.9.12，不改变 `RELEASE.md` 对后续版本的门禁要求。

## 未完成（已知）

- [ ] 发布门禁：`.github/workflows/release.yml` 的 `release-readiness` 任务要求 M2 至 M9 与 MB 通过、AZ-069 完成。0.9.12 未接入该门禁，后续版本需在发布前决定是否启用。
- [ ] 用户验收：按 `docs/upgrade/acceptance-0.9.12.md` 的测试用例在发布版本上逐项验证。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 仍有 0.9.11 之前就存在的告警，本版本未修复。
- [ ] 安装包签名（AZ-064）。
- [ ] macOS、Linux 真机验证。

## 发布方式

- 通过 tag `v0.9.12` 触发 `release.yml`，构建 Windows、Linux、macOS 安装包。
- 未签名、未验证的安装包会在 Release 说明中标注。
