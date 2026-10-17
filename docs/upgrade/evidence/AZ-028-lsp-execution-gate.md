# AZ-028 LSP 进程执行审批（cycle 025）

日期：2026-10-05；Codex；主实现 AZ-028。

检查固定依赖 v0.8.68（527e554）的 piscis-ide-tools LspTool：调用会根据源码探测项目根并取得语言服务器客户端，首次调用可启动服务器；结果经 format_result 转为文本。不能仅凭 is_read_only 放行，也不能从最终文本可靠恢复所有结构化资源路径。

主聊天现在将 lsp 列入必须一次性审批的能力，仍先校验输入 file 路径。无可信审批入口的后台 registry 移除 lsp。可信宿主预览明确服务器启动、项目配置、构建脚本及工作区外依赖读取影响，模型参数 description 不能覆盖该提示。本轮替代 cycle020 的“正常源码 LSP 自动继续”行为；后台研究也暂不再提供 LSP，保留其他安全读取工具。

验证：agentz-task-state 全部38/38通过（95.66秒），新增可信 LSP 提示回归；宿主 approval_execution 17/17通过（4.21秒），现有生命周期测试覆盖 read_lints 与 lsp 的 pending→预览→批准→claimed→executed，并验证非交互正常源码 LSP 拒绝、后台 registry 移除。测试使用工具替身，不启动真实 LSP 或模型。日志 `.cache/upgrade-lsp-domain-cycle-025.log`、`.cache/upgrade-lsp-host-cycle-025.log`；构建退出0。git diff --check通过，仅已有行尾提示。仅对三个修改Rust文件执行rustfmt。

未实现语言服务器 OS 沙箱、全部返回路径过滤、结构化输出包装或项目构建隔离；显式审批不等于这些隔离已完成。未运行原生桌面、真实模型、跨平台与回滚验收。AZ-028仍doing；未发布、部署、对外发送。
