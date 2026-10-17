# AZ-028 LSP 与诊断资源（cycle 020）

日期：2026-10-05；Codex；主实现 AZ-028。

用户要求汇报并继续。已核对最新 plan/进度及 cycle 019 证据；79项、14 done、4 doing/review、1 blocked，仅 M0 passed。开始时无 cargo/rustc，未发现适用 AGENTS.md，保留已有修改。

实际固定依赖 read_lints 源码声明只读，却会回退 cargo check/tsc。这可能运行项目构建逻辑，不应凭只读标志自动放行。现将 read_lints 列入主聊天逐次审批能力，要求非空显式 paths 并逐文件校验源码范围；无可信审批的后台 registry 移除它。

LSP 主聊天及后台实际调用都校验 file 目标，排除越界/元数据/受保护命名；后台研究保留显式依赖源码只读根。普通源码 LSP 查询仍可继续。未声称 LSP 返回引用、服务器进程及全部图资源均已隔离。

宿主 integration 16/16 通过：`.cache/upgrade-approval-lsp-cycle-020.log`，编译 3m18s，测试 2.54s。新增 LSP越界与.env拒绝、正常源码允许、后台诊断拒绝；主聊天诊断签发→预览精确文件列表→批准→claimed→结束。测试模拟生命周期，未启动编译器或真实 LSP。扩展 registry 回归证明声明只读的 read_lints 仍移除。已有14项Git/SQLite/journal/缓存回归继续通过。

台账 validate（79 tasks/11 milestones）与 git diff --check 通过（已有行尾提示）；构建退出。未改前端/领域库，未重跑它们。

AZ-028 仍 doing：审批预览需明确说明诊断可能启动构建；固定内核递归链接、图/LSP输出路径与原生 protected preview 继续待办。真实模型、原生桌面、跨平台及回滚未验收；真实模型基线仍 blocked，AZ-079 Undo缺陷未修复。未发布、部署或对外发送。
