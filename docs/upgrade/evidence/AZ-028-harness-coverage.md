# AZ-028 其他 harness 覆盖（cycle 016）

日期：2026-10-05；Codex；主实现仍为 AZ-028。

核对台账/进度和 cycle 015 证据；仓库/祖先未发现 AGENTS.md，开始时无运行构建。保留已有修改。

源码入口审查发现：主聊天/heartbeat 使用已接审批的 run_agentz_turn；研究子任务使用独立 scheduler harness；Koi 和 IM gateway 分别调用内核 run_piscis_turn_cancellable/run_piscis_turn，无持久审批 hook，且原注册表带写文件、命令和连接器能力。

新增统一 restrict_unapproved_background，置于研究子任务、Koi、IM gateway 的注册表构建结束后、进入 harness/headless runner 前。保留声明只读且空输入不要求确认的工具；明确移除执行/浏览器/桌面/MCP/API connector/委派/pool 能力，即使其声明只读也移除。默认非只读工具全部移除。

行为变化：Koi/IM 当前只能使用该受限只读工具面，不能继承主任务审批进行写入、执行或连接器调用。后续恢复这些能力必须接可信授权，不直接恢复旧全量注册表。研究子任务保留只读用途。此过滤不是完整资源授权或秘密读取隔离，参数依赖确认与所有第三方只读声明仍需后续审查；未声称 OS sandbox。

## 验证

宿主审批 integration 10/10 通过；编译 3m59s，测试 4.75s。日志 `.cache/upgrade-approval-harness-cycle-016.log`。新增 registry 回归证明写文件移除、声明只读的命令/MCP/连接器/委派/pool 也移除、要求确认工具移除，普通 file_read 保留。其余九项实际 SQLite/Git/journal/hook 回归继续通过。宿主完整编译覆盖三个调用点，但未执行真实 Koi/IM 模型旅程。

台账 validate（79 tasks/11 milestones）和 git diff --check 通过；后者只有已有行尾提示。未修改前端或领域库，未重跑它们。构建已退出。piscis-engine 仍只有之前 AZ-029 三文件修改；固定依赖未变。

AZ-028 仍 doing，下一步审查只读工具的参数/资源边界及原生 protected preview。原生桌面、真实模型、跨平台、回滚均未验收。AZ-079 未修复，本轮未发布、部署或对外发送。
