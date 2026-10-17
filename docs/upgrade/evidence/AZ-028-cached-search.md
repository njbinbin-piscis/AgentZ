# AZ-028 旧索引结果边界（cycle 019）

日期：2026-10-05；Codex；单一主实现仍为 AZ-028。

已读台账、进度及 cycle 018 证据；未发现适用 AGENTS.md，开始时无 cargo/rustc；保留现有修改。

codebase 搜索现在在旧缓存行排名/返回之前检查来源：必须为正常相对路径、实际存在的源码文件，排除绝对路径、ParentDir、宿主元数据和 .env 等受保护命名；逐层 symlink_metadata 拒绝链接，并验证 canonical 文件仍位于 canonical 工作根内。同一查询按文件路径缓存检查结果，避免同文件多代码块重复检查。

已有数据库不删除、不改 schema；危险旧记录被排除于搜索输出，数据库中的原始缓存内容未做清理或秘密脱敏。此次是返回边界修复，不能声称完成历史缓存迁移。

## 已运行验证

- 宿主审批 integration 14/14，通过 `.cache/upgrade-approval-cache-cycle-019.log`；编译 3m22s，测试 3.72s。新增真实 SQLite 危险历史行夹具：外部/绝对/不存在/.env/元数据路径全部不返回，正常 safe.rs 唯一命中。
- 扩展真实 Windows junction 夹具，主动插入 linked/outside.rs 历史行后，查询仍无外部源码结果。
- 已有 codebase 排序/过滤单元回归 1/1，通过 `.cache/upgrade-index-ranking-cycle-019.log`；编译 4m43s，测试 0.05s。正常代码排名、文档/扩展路径过滤未破坏。
- 台账 validate（79 tasks/11 milestones）及 git diff --check 通过（已有行尾提示）。构建全部退出。本轮未改前端/领域库，未重跑它们。

未运行模型、原生桌面、跨平台或回滚验收。仍需处理固定内核递归搜索链接、图/LSP 资源范围及缓存数据清理策略；不存在完整 OS sandbox 或全面秘密识别证明。AZ-028 保持 doing，AZ-079 未修复。本轮未发布、部署、对外发送或更改固定内核依赖。
