# AgentZ 0.9.13 发布清单

2026-10-18。目标：去掉 CodeZ 与 WorkZ 聊天窗口里重复的计划、历史、任务概况、交付和产物面板，恢复对话区原有布局。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README、RELEASE 均为 0.9.13。
- [x] CHANGELOG 增加 0.9.13 条目。
- [x] CodeZ、WorkZ 对话区不再挂载 `DurablePlanActions`、`RunHistoryPanel`、`TaskDetailsPanel`、`DeliveryReviewPanel`、`TaskArtifactViewer`、`TaskFileViewer`、`TaskReminderPanel`。
- [x] 打开会话时后台调用 `queryTaskPlan`，结果进入原有折叠任务列表。
- [x] WorkZ 底栏去掉未样式化的「高级选项」折叠。
- [x] `npx tsc --noEmit -p .` 通过。
- [x] `node scripts/upgrade-plan.mjs validate` 通过。

## 发布决策

- 2026-10-18，用户要求提交、推送并发布新版本。验收门禁仍未通过。若把工作区里未提交的 `release-readiness` 接入 `release.yml`，`scripts/upgrade-release-gate.mjs` 会因 M2–M8、MB 与 AZ-069 失败而阻止全部安装包任务。
- 因此 0.9.13 与 0.9.12 相同：不提交该门禁改动，通过 tag `v0.9.13` 直接构建安装包。未通过项仍记录在 `docs/upgrade/acceptance-0.9.12.md`。
- 该决定只覆盖 0.9.13，不改变 `RELEASE.md` 对后续版本的门禁要求。

## 未完成（已知）

- [ ] 发布门禁：后续版本在发布前决定是否启用 `release-readiness`。
- [ ] 用户验收：按 `docs/upgrade/acceptance-0.9.12.md` 在发布安装包上验证，并确认聊天窗口不再出现重复面板。
- [ ] 安装包签名（AZ-064）。
- [ ] macOS、Linux 真机验证。

## 发布方式

- 通过 tag `v0.9.13` 触发 `release.yml`，构建 Windows、Linux、macOS 安装包。
- 未签名、未验证的安装包在发布说明中标注。
