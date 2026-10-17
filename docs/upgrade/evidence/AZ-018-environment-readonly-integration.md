# AZ-018 环境登记只读贯穿与下一实现包

2026-10-08，主线程集成复核。适用 AGENTS.md 搜索无结果；读取现有台账、最新证据及构建状态后，沿用 AZ-018 单主任务。环境库 lib.rs 由一个代理独占，另一个代理只读检查正常调用/后续流接线位置。

public load 原签名不变，统一委托 load_existing_readonly；因此 workspace、approval、ManagedRun、plan 和 event-replay 原调用均使用只读登记读取，不引入双套 host resolver。缺库返回 None 且不创建目录；try_exists IO 错误明确拒绝。既有库必须 version1 及所需 TEXT NOTNULL/主键列，记录 task/environment 与列键、schema/project 匹配；version0/未来版本/缺列/损坏不初始化、不修复。register 仍负责合法登记写入与正常建库。

独立设计审查没有找到正常调用依赖 load 初始化的路径：任务创建缺库读 None 后 register 初始化，既有执行/查询只读取身份。刻意兼容变化是既有 version0 空/旧库明确要求恢复；不能把未知现有库当成缺失登记后走 Direct。SQLite WAL/SHM 锁等辅助文件行为不以主 DB 字节一致证明完全 filesystem-immutable。真实 GUI、模型、跨平台、恢复流程、安装/回滚仍未验收；源码晚于现有安装器。

下一包转入聊天流耐久化，避免继续扩大本轮健壮性范围：

- RunStore 新增宿主绑定的事件 append 接口，验证 authoritative Run/current Attempt/environment；事务内稳定事件 ID、递增序号及 bounded JSON 写入，复用 replay。
- 本地 interactive chat_turn 的 AgentEvent collector 在 emit_session(agent_event) 前先提交记录，身份来自当前实际执行 captured binding，不从 session 或 UI 猜测。先覆盖 Text/ToolStart/ToolEnd/Error/Done 的被动记录，终态仍由 ManagedRun 完成。
- 持久化失败必须显式可见并停止声称耐久成功；不得通过重新执行工具补写。超预算事件与大附件必须给出明确兼容策略，不能静默截断后宣称完整恢复。
- 先以真实 SQLite 加模拟 AgentEvent 验证先写后发、失败、分页、跨任务、重试与丢连接补读；真实模型和 GUI 旅程继续单独记录未验收。远程/团队/池不自动扩入支持范围。

最终实际验证完成后追加，不将已编写夹具追认为通过。

最终主线程 session20486 整个顺序命令链 exit0：task-state 全量 83 项测试通过，all-targets 严格 Clippy 通过；registered desktop lib event_replay 6 项、ManagedRun 1 项、chat_plan 3 项通过。包含新增 readonly missing/v0/future/corrupt/v1-reopen 三组真实 SQLite 夹具，以及原登记、执行、归档、合并回归。独立最终源码复核限定通过，未发现必须修复的新 bug；结论不涵盖环境唯一索引或全库历史完整性、外部并发 schema 篡改。没有前端源码改动，不重复已通过的前端检查；没有运行真实桌面、模型或发行验收。
