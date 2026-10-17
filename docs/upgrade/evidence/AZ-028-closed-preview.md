# AZ-028 已关闭等待方不再提供预览（cycle 031）

日期：2026-10-05；Codex；主实现AZ-028。核对台账与cycle030证据，无cargo/rustc/link构建；未发现适用AGENTS.md，保留已有修改。

ApprovalBroker::preview 现在检查oneshot等待方是否关闭；关闭时返回“Approval preview is no longer active”，不返回仍保存在pending中的快照。仅影响预览活跃性，未将读取操作当作批准/取消；原决策投递失败撤销及attempt取消机制保持有效。

扩展已有关闭等待方回归：drop receiver后预览拒绝，再尝试批准仍不能投递且持久意图变为Revoked；取消后的请求不能重放。审批相关18/18通过（0.37秒，其他20项未运行），领域库all-targets严格Clippy通过（4.82秒），进程退出0。日志 `.cache/upgrade-closed-preview-cycle-031.log`。台账validate及diff --check通过，仅已有行尾提示；只格式化修改的broker文件。

未改前端，未重跑宿主、非审批领域用例或原生桌面。真实模型、跨平台、回滚及OS隔离未验收。AZ-028保持doing；未发布、部署或对外发送。
