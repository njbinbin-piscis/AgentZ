# AZ-028 诊断审批影响说明（cycle 021）

日期：2026-10-05；Codex；唯一主实现 AZ-028。

已核对进度/台账及 cycle 020 证据；仓库未发现 AGENTS.md，开始时无 cargo/rustc。保持现有修改。

可信 broker 为 read_lints 的预览生成明确说明：可能启动语言服务器、运行 cargo check/tsc、执行项目构建脚本并创建构建输出，仅批准这次操作。说明由 action 对应宿主代码决定，不采用模型参数内的 description。原参数和摘要绑定不变，前端通过现有可信预览 IPC 展示该描述。

新增 broker 回归：传入“harmless read only”的参数描述，预览仍含构建脚本/命令/输出说明，且保留精确文件列表。审批领域 17/17，通过（0.42s）；严格领域 Clippy 通过（8.40s）。日志 `.cache/upgrade-approval-preview-cycle-021.log`。台账 validate（79 tasks/11 milestones）及 git diff --check 通过（已有行尾提示），构建退出。

本轮仅修改 broker 文本与领域夹具；未重跑宿主集成、前端或原生窗口，不将此前测试作为本轮新运行结果。实际桌面呈现、多语言、真实模型、跨平台和回滚未验收。AZ-028 保持 doing：固定内核递归链接、图/LSP输出资源以及 protected preview 原生验收待办。AZ-079 未修复；未发布、部署或对外发送。
