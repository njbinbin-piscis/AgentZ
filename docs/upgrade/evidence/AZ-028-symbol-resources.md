# AZ-028 符号索引资源边界（cycle 022）

日期：2026-10-05；Codex；主实现 AZ-028。

已核对台账/进度和 cycle 021 证据；仓库未发现 AGENTS.md，开始时无 cargo/rustc，保留已有修改。

graph_search/graph_explore 的 query 是已有图节点匹配条件，不用于任意文件读取，未盲目删除正常图关系。实际问题在 symbol_search/impact 的源码收集器：它可跟随目录链接。

符号收集现在跳过链接，且使用已有源码路径校验排除越界和受保护命名。index_for 固定 canonical 根，失效根返回空索引，不创建新目录。持久缓存的符号与调用来源全部检查；若任一不安全则拒绝完整缓存并重建，保持 caller/symbol 编号关系而不是局部删行。原始旧缓存清理不是本轮单独迁移验收。

验证：

- 宿主 integration 首轮及源码冻结后最终均16/16通过；真实Windows junction 用例扩展到符号索引，外部函数不进入结果。日志 `.cache/upgrade-approval-symbols-cycle-022.log`（编译7m35s，测试5.85s）和 `.cache/upgrade-approval-symbols-cycle-022-final.log`（测试4.82s）。
- 符号专项6/6通过；新增真实SQLite来源篡改拒绝及失效根不创建；既有Rust/TS/Python/Go解析、双向调用图、传递impact和本仓库索引回归继续通过。日志 `.cache/upgrade-symbols-unit-cycle-022.log`（编译2m20s，测试5.93s）。
- 台账validate（79 tasks/11 milestones）、git diff --check通过，后者仅已有行尾提示。所有构建退出。未改前端，未重跑前端或领域测试。

仍未验证真实模型、原生桌面、跨平台、回滚或完整OS sandbox。固定内核递归搜索链接与LSP返回路径仍待收口；图缓存自身的来源/完整性不是已完成授权证明。AZ-028保持doing，AZ-079未修复。无发布、部署、对外发送或固定内核依赖变更。
