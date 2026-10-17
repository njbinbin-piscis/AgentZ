# AZ-018 当前执行身份与历史补读候选

2026-10-08，主线程集成；原验收保持 preparing。

宿主新增 chat_current_run：主窗口 opaque scope 授权、环境读租约、existing-session fresh binding、查询前 scope 再校验；返回当前 task/current environment 最新创建 Run 的当前 Attempt。数据库缺失返回 None，不创建 runs.db；元数据损坏不伪装为空记录。只读发现复用与事件查询相同的绑定验证，不接任何执行/重试/模型恢复动作。

CodeZ AssistantPanel 与 WorkZ 普通本地任务接共享 RunHistoryPanel。使用 host 发现身份，再用真实 RunEventCollector 和 scoped cursor 分页补读当前 Attempt 的生命周期历史。初始、busy 变化、手动刷新、窗口 focus、online 和文档重新可见时重读；最多 100 页、每页 200，显示最近 100 条，超预算/损坏明确错误，不展示不完整成功记录。不覆盖聊天全文或恢复工具副作用；团队/池任务没有接此入口。

实际宿主验证 session60450 全部 exit0：新增 current_task_run 两项 SQLite 测试通过，task-state all-targets 严格 Clippy 通过，registered desktop lib event_replay 六项测试通过。覆盖 rowid 创建排序与 retry 当前 Attempt、跨环境/任务、同任务腐败拒绝、数据库字节不变；宿主已有缺失数据库 fixture 增加发现 None、不创建目录，以及真实发现 a2 重试身份。前端最终验证和独立复核结果追加在下方。

限制：这里只显示最新 Run 的当前 Attempt，旧 Attempt 可由后端 scoped API 查询，未提供 UI 选择器。每次重新分页读取而非持久化前端 cursor；完整 token/tool 聊天流尚未全部耐久化。真实 Tauri GUI/IPC、模型断流、跨平台、安装/回滚仍未验收。本轮源码晚于现有 0.9.7 本地安装器，不能声称已包含或已发布。

独立只读代理复核：授权/归属/创建排序限定通过，发现 filesystem exists 会把访问错误当作缺失；主线程已改 try_exists 显式传播 IO 错误。共享 existing_binding 调用正常 kernel metadata 初始化入口，因此仅 Run/events 正文只读，不能声明整个 IPC 所有 metadata 文件不写入。历史 Attempt 时间戳损坏的宿主统一校验尚未补齐（前端已拒绝负 Run updated_ms，事件页拒绝负 created_ms），作为下一包健壮性夹具记录，不隐瞒该限制。

主线程 session85168：集成两工作区后 TypeScript、限定 ESLint 均 exit0，18 文件全量 Vitest 130 项通过。随后组件独立复核修复切换第一帧旧结果、JSON 状态名字查找原型问题，最终检查另行追加。

最终主线程 session87178：try_exists 修复后 registered host event_replay 六项测试再次全部通过。session20351：最终 TypeScript、组件/发现服务限定 ESLint exit0，组件 7 + 发现 6 + 游标服务 6 共 19 项测试通过。独立组件代理也实际复跑 7 项通过；主线程复核组件 requestKey 在 render 阶段隐藏旧任务结果，sequence 在每次 await 后拒绝旧响应；未知状态名只取 own property，否则友好 fallback。所有结果只证明代码/夹具层，不替代真实桌面验收。
