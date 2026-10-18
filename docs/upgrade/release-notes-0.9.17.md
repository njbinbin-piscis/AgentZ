# AgentZ 0.9.17 发布说明

发布日期：2026-10-19。版本号与 `CHANGELOG.md` 的 0.9.17 条目一致。

本版本修复 WorkZ 会话标题前状态灯在回合已停止后仍闪烁的问题，并保证库内 `status` 与真实在途回合一致。

**本版本未通过上线验收门禁。** 未验收项与用户测试用例仍见 `docs/upgrade/acceptance-0.9.12.md`。

## 修复

- **会话状态落库：** 回合写入 `running` 后由 `SessionRunningGuard` 保证结束时写成 `idle` 或 `error`；中途失败退出时也会清回 `idle`。
- **列表对账：** `chat_list_sessions` 若发现 `running` 但任务闸门中没有对应在途回合，则写回 `idle`（覆盖进程中断后的陈旧行）。
- **侧栏指示：** 仅当会话在前端 `runningIds` 中时黄灯脉冲；库里单独残留的 `running` 按空闲显示。

## 已知限制

- 未通过验收门禁。发布时未把 `release-readiness` 接入 `release.yml`，否则安装包任务不会开始。
- 安装包未签名。macOS 安装包未公证，首次打开需要手动允许。
- macOS 与 Linux 安装包未在真机上运行。
- 0.9.16 的已知限制仍然适用，见 `docs/upgrade/release-notes-0.9.16.md`。
