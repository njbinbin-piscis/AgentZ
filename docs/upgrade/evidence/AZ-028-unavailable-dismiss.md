# AZ-028 不可用审批预览本地关闭（cycle 030）

日期：2026-10-05；Codex；主实现AZ-028。核对台账与cycle029证据，无cargo/rustc/link构建；未发现适用AGENTS.md，保留已有修改。

可信预览失败时首按钮改为本地Dismiss，保留Retry preview。关闭不发送缺少digest的拒绝请求，也不授予权限；宿主仍按原有超时/取消处理意图。本地关闭处理器与决策处理器一样检查挂载状态及当前previewKey，修复旧过期卡片残留关闭处理器可能关闭替换请求的行为。

新增不可用预览本地关闭无决策IPC、旧过期关闭处理器不关闭新请求两项回归。组件17项，前端74/74、10文件通过（3.57秒），typecheck及定向ESLint退出0。日志 `.cache/upgrade-preview-dismiss-cycle-030.log`。台账validate与diff --check通过，仅已有行尾提示。未改Rust，未重跑宿主/领域库。

本地关闭不等于已取消宿主执行；未运行真实原生IPC故障、桌面渲染、模型、跨平台、回滚或OS隔离验收。AZ-028保持doing，未发布、部署或对外发送。
