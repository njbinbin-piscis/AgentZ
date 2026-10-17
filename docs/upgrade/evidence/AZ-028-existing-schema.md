# AZ-028 数据库替换失效检查（cycle 012）

日期：2026-10-05；执行者 Codex。

核对台账、最新 hook 证据及进度；仓库和祖先目录未发现 AGENTS.md，开始时无运行中的 cargo/rustc 构建。保留现有修改，继续单一主任务 AZ-028。

open_existing 现在只接受 schema 1。若数据库被移走，拒绝且不创建文件；若被替换为 schema 0 的 SQLite 文件，拒绝且不创建表、不写 user_version。只有签发入口 open 仍可显式初始化新数据库。已有高版本拒绝降级逻辑保留。

新增实际 SQLite 回归，检查不存在文件不重建、空替换数据库无表且版本保持零，以及显式签发入口可初始化。审批测试 16/16（0.43s）、领域严格 clippy 通过（4.91s）。日志 `.cache/upgrade-approval-domain-cycle-012.log`。台账 validate：79 tasks/11 milestones，一致。

本轮仅修改领域数据库打开条件，未重新运行宿主 integration、前端或原生桌面；上一轮相应证据保留，不能作为本轮新运行结果。AZ-028 仍 doing：journal before/after 配对、direct 根/环境一致性、等待期间绑定变化及原生 surface 验收待完成。AZ-079 未修复；未发布、部署或更改固定内核依赖。
