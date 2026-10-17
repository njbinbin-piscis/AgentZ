# AZ-028 等待审批期间的环境失效（cycle 014）

日期：2026-10-05；Codex，仍只有一个主实现任务 AZ-028。

已读取台账、生成进度和 cycle 013 证据；仓库/祖先未发现 AGENTS.md，开始时无 cargo/rustc 构建。保留现有修改。

新增真实 Git + SQLite + pinned AgentHooks 集成用例：建立 main 仓库与 managed worktree，登记 task-a/env-a，主机签发 shell 意图并发出预览后，在实际 worktree 切换到 changed-after-preview 分支，再批准原摘要。

结果：before_tool 返回 Deny，delegate.before_tool 次数为零，夹具源码仍为 base；审批从 approved 转为 revoked，没有 claimed 或执行。验证了已有等待后 fresh 校验与 PendingGuard 撤销在真实注册任务中的协作。本轮未修改生产逻辑。

完整宿主审批集成 7/7 通过，编译 42.46s，测试 5.08s。日志 `.cache/upgrade-approval-binding-cycle-014.log`。台账 validate 通过：79 tasks/11 milestones。git diff --check 无错误（已有行尾提示）。构建已退出。

测试人工驱动 hook，未启动真实模型或原生桌面；不能据此声称 OS sandbox、跨平台或完整 UI 验收。direct CodeZ 环境/根别名一致性和原生 protected preview 仍待收口，AZ-028 保持 doing，AZ-079 未修复。本轮无发布、部署或固定依赖变更。
