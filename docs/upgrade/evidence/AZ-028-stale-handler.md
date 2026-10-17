# AZ-028 旧审批处理器生命周期（cycle 028）

日期：2026-10-05；Codex；主任务AZ-028。核对最新台账与cycle027证据，无cargo/rustc/link构建；未发现适用AGENTS.md，保留已有修改。

审批决策处理器在发送IPC前检查组件仍挂载且当前预览key与闭包捕获的key相同。换卡片后残留的批准/拒绝处理器，以及卸载后残留处理器不再发送旧请求。新卡片仍能发送自身digest，宿主一次性claim保持最终权威。

新增两个组件回归：保存旧处理器→换卡片→旧批准和拒绝均无IPC→新请求批准成功；保存处理器→卸载→无IPC。扩展原投递失败用例，验证释放同步锁后再次提交成功、仅一次onResolved。组件14项；前端71/71、10文件通过（4.75秒），typecheck及定向ESLint退出0。日志 `.cache/upgrade-approval-stale-handler-cycle-028.log`；台账validate与diff --check通过，仅已有行尾提示。未修改Rust、未重跑宿主或领域库。

这是组件与模拟IPC验证，未运行原生桌面故障重试、真实模型、跨平台、回滚或OS隔离验收。AZ-028仍doing；未发布、部署或对外发送。
