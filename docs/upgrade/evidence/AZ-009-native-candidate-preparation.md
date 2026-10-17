# AZ-009 原生候选准备

2026-10-05，Codex。遵循ADR-003，不新开doing。台账82任务、14验收、4review、1blocked；开始无cargo/rustc/link。检查原生入口发现上次恢复选择错误：按窗口标题匹配AgentZ会包含Cursor和浏览器。此次按sky返回进程路径精确选中唯一安装版窗口，include_screenshot=false的可访问性树读取成功。此前截图超时不等于所有原生读取不可用；未发送UI输入，安装版结果仍不计候选验收。

确认src-tauri/src/commands/data_scope.rs现有AGENTZ_CONFIG_DIR覆盖入口，可隔离候选全局配置，不需修改用户配置。已启动串行npm run build→cargo build -p agentz-desktop，日志`.cache/upgrade-native-candidate-build.log`；本记录写入时构建尚未完成，未记通过。下轮先检查该进程和日志，避免重复构建。

候选完成后需在独立配置和临时项目中启动，精确按候选进程选择窗口，核对A/B任务与重启后的文件证据。未完成A/B、重启、评审、审批原生点击、模型或跨平台验收，未发布或部署。
