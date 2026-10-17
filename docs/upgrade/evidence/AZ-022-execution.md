# AZ-022 工具恢复候选执行证据

2026-10-09。版本1结构化观察已接入实际 vendor shell/code_run AppContainer 路径。保留已有 ToolResult 类型，通过内容通道附带被动 JSON footer；不把模型或终端输出中的相似文本当作授权。通用契约同步到独立内核源码；已有源仓非沙箱工具未被整文件覆盖。

进程正常完成、非零退出、超时、取消和启动前拒绝分别报告。非零退出现在 is_error=true。runner Err/worker JoinError 保守为未确认；只有真实根退出、Job active=0、两条输出读取线程已结束以及工作区 ACL finish 返回之后的 Ok 才提供清理确认。UAC未响应/拒绝有类型契约，当前产品提权不支持，绝不以未启动提示框冒充真实UAC测试。

修复捕获超过上限后静默丢弃输出而未标记的问题：两流分别记录真实读取总字节与保留字节，读到 EOF 后才确认；展示截断与重复行压缩另有单调标记，不以UTF8 lossy或截断提示字符串长度假充原始字节。stdout join失败也会先尝试stderr join与ACL清理，再返回保守错误。重试观察固定 forbidden，不新增执行重放。

实际运行：74168 `cargo test -p piscis-kernel --offline --locked --lib tool_recovery -- --test-threads=1`，8 passed、0 failed，0.03秒。85747 `... --lib production_runner -- --test-threads=1`，6 passed、0 failed，47.28秒，包括本机Windows双流实际超限、超时与取消Job收割、越界读取拒绝、hard-link拒绝及ACL授予失败回退。专项通过不能代替完整回归或真实桌面Stop。

独立接线审查见AZ-022-integration-review.md；增量vendor overlay见AZ-022-tool-recovery-v0.8.68.patch，隔离apply/reverse与实际字节检查由复核代理运行通过。它不是完整vendor从原tag重建证据。当前已归档20261009安装器在这些后续修改前构建，不能据此声称安装器包含AZ022或新Flash别名修复。

63832实际完整内核库串行回归389 passed、0 failed、1 ignored，91.36秒；随后严格 `cargo clippy -p piscis-kernel --offline --locked --lib -- -D warnings` exit0，1分43秒。ignored要求真实评估数据库，不计通过。完整回归包括两工具调用结果接线、Job后代收割以及真实ACL失败与恢复夹具；不以这些夹具推广其他平台或桌面体验。

未验收：真实GUI、完整UAC交互、reader+ACL同时故障、launch/Job assignment/resume API故障、低权限/跨卷，以及全局detached/team/pool/remote资源。capture join缺少独立超时；多阶段失败仍保守归并Unknown，首错诊断不代表其余阶段成功。AZ022保持preparing，AZ021与正式Gate不提升。
