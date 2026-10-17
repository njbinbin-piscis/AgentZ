# AZ-028 实际执行 hook 接入（cycle 011）

日期：2026-10-05；执行者：Codex。任务仍为 doing，未作完整产品验收。

主聊天 runner 已将固定内核 v0.8.68 的 AgentHooks 与原 journal 组合。before_tool 按真实 tool_use_id、原始参数、任务/环境绑定签发意图；批准后重新验证并原子 claim，after_tool 按真实 ID 记录 executed 或保守 unknown。取消、超时、无交互后台、失效数据库均拒绝继续；此 runner 不再向旧 bool confirmation map 请求执行许可。

审批事件仅作唤醒。受来源校验的 get_permission_preview IPC 从 broker 内存读取原始快照，卡片忽略伪造事件正文并提交对应 digest。数据库保存摘要和状态，不保存原始参数；内存预览仍含原始参数，秘密引用/脱敏尚未实现。

普通 file_read/write/edit 限当前根的源码路径，阻止已解析的越界和 .git/.agentz/.codez/.piscis/.env 等元数据路径。进程及浏览器等能力逐次审批；这些检查不构成 OS sandbox。

## 已运行验证

- 真实宿主 integration：5/5，通过日志 `.cache/upgrade-approval-hook-integration-cycle-011-final.log`；编译 4m01s，测试 0.23s。实际使用 pinned AgentHooks、SQLite 和 FileJournal；手动调用 hook，在 Continue 后施加夹具文件副作用，无模型或原生窗口。
- 覆盖：精确批准/领取/结束，拒绝与无交互后台，取消撤销不可重放，普通源码编辑与 journal 路由/单次 Undo，数据库文件移走后拒绝且不重建。
- 审批领域测试 15/15；严格 domain clippy 通过。日志 `.cache/upgrade-approval-domain-cycle-011-final.log`。
- 前端 63/63，TypeScript 通过。包含不可信事件正文与可信预览、异步旧回复保护。日志 `.cache/upgrade-approval-ui-cycle-011-final.log`。
- 宿主 cargo check 通过（1m09s）：`.cache/upgrade-approval-hook-cycle-011-final.log`。
- 台账验证及隔离 fixture 回归通过，包括新增 follow-up、重复 ID/未知依赖原子拒绝，原 plan 不变。

首轮 integration 为 3/5：同轮重复编辑 Undo 发现固定内核缺陷，已单独登记 AZ-079，保留首轮日志和复现证据。另一失败是 Windows 不允许移动仍有 journal 打开的目录；改为移走审批数据库文件后通过。最终单次 Undo 测试不代表重复编辑缺陷已修复。

## 未完成范围与下一步

仅主聊天 runner 已接入；其他 harness、原生受保护审批界面、真实模型及跨平台均未验收。资源目前绑定 workspace/tool ID/provider 和输入，尚未完成真实账户/浏览器 profile/窗口身份解析及六类资源的完整授权。

后续检查：仅对已进入 delegate.before_tool 的调用配对 journal.after_tool；direct CodeZ 环境变化与根别名的执行一致性；existing 数据库版本零应拒绝初始化；真实任务绑定在等待批准期间失效的 fixture。完成这些收口及原生验收前保持 doing，不把 hook 夹具通过表述为完整桌面执行验收。
