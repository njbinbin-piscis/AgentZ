# AZ-028 后台只读实际调用检查（cycle 017）

日期：2026-10-05；Codex；唯一主实现 AZ-028。

已核对台账、进度和 cycle 016 证据，仓库/祖先未发现 AGENTS.md，开始时无 cargo/rustc；保留现有修改。

上一轮后台 registry 过滤只在空输入上检查 needs_confirmation，不能证明真实参数无需确认。现在对保留工具加调用包装：在真正调用 provider 前，用真实输入再次检查只读声明与 needs_confirmation；需要确认时直接返回错误，不进入 provider。元数据及完整/最小 schema 转发，避免破坏原工具注入。该检查不依赖 ToolContext.bypass_permissions。

file_read 另校验 canonical 本地工作根、目标路径和受保护文件名。越界、.agentz/.git 等宿主元数据、.env 等凭据文件名被拒绝。研究子任务原有 Cargo checkout/registry 源码读取通过宿主明确传入的 canonical 只读根保留，Koi/IM 无此额外根；依赖根例外仍排除受保护路径。

## 验证

宿主 integration 11/11 通过，编译 4m13s，测试 4.53s，日志 `.cache/upgrade-approval-read-cycle-017.log`。

新增实际调用测试：空输入不要求确认的工具在真实危险参数下被阻止；使用 bypass_permissions=true 仍不可绕过；元数据、.env、越界不调用 provider（计数零），普通源码可读取；未授权外部依赖被拒绝，明确只读根内 lib.rs 可读取而 .env 仍拒绝。其余十项审批/Git/SQLite/journal 回归继续通过。

台账 validate（79 tasks/11 milestones）与 git diff --check 通过（已有行尾提示）；未改前端和领域库，未重跑它们。测试人工驱动 hook/provider，无真实模型、原生窗口、跨平台或 OS sandbox 验收。

剩余边界：本轮专门保护 file_read；其他搜索工具的目录/参数范围、所有凭据命名、远程资源及后台环境别名一致性未全面验证。只读声明仍不是 OS 隔离或秘密数据策略。AZ-028 保持 doing，继续原生 protected preview 和剩余资源审查；AZ-079 未修复。未发布、部署或对外发送。
