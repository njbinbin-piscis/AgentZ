# AgentZ 1.0.0 发布说明

发布日期：2026-10-19。版本号与 `CHANGELOG.md` 的 1.0.0 条目一致。

AgentZ 1.0.0 完成了 `R10-01` 至 `R10-24` 全部 24 项工程与产品闭环任务，打通了 `WorkZ`（自治任务工作台）与 `CodeZ`（AI 原生 IDE）的双向协同闭环，并通过了本地工程、安全脱敏、数据迁移、性能分包与跨视图 E2E 自动化门禁（详见 [ADR-006](adr/ADR-006-1.0-engineering-and-ux-baseline.md) 与 [1.0.0 发布签署报告](release-decision-1.0.0.md)）。

## 核心特性与架构升级

- **WorkZ ↔ CodeZ 双向接力与防冲突护栏：**
  - `CodeZ` 编辑器支持未保存脏缓冲（Dirty Buffer）冲突保护弹条（查看 Diff / 采纳磁盘 / 保留修改）。
  - `WorkZ` 任务评审面板与文件预览栏支持一键「在 CodeZ 中打开」隔离工作树，并在顶栏提供返回胶囊。
  - 人工在 `CodeZ` 或 `WorkZ` 保存修改后自动记入编辑台账，下一轮对话自动向 Agent 注入 `<user_recent_edits>` 增量。
  - `AgentTaskReview` 支持未提交改动自动暂存合入；非 Git 普通文件夹支持任务级快照一键回滚。
  - `WorkZ` 右侧 `AgentFilePreview` 升级为可编辑文档画布（`Preview / Edit / Diff`、`Ctrl+S` 保存、基于最新修改继续对话）。
- **Vibe Coding 与白领办公高频体验：**
  - `CodeZ` 全局命令面板（`Ctrl+Shift+P`）与快速打开文件（`Ctrl+P`）。
  - `WorkZ` 与 `CodeZ` 对话气泡内置单轮「一键回滚本轮文件修改（`journalUndoTurn`）」。
  - `WorkZ` 空状态 4 大高频场景卡片、默认开启沙盒隔离并收起高阶 SHA 控件；首次启动提供 3 步极简引导向导，Swarm/Workflow 等高阶实验特性标注 `Beta` 默认折叠。
  - 旁路 LLM 调用（自动标题、长期记忆、行内编辑）纳入统一预算记账，状态栏新增模型切换与 Token/USD 费用胶囊。
- **工程解耦、安全合规与质量门禁：**
  - 拆分前后端巨型模块（`workz/index.tsx`、`codez/index.tsx`、`AssistantPanel.tsx`、`src-tauri/src/commands/chat_turn/`），清退 18 个孤立反人类合规演示组件。
  - 固化 Agent 内核五阶段契约测试与 `RobotZ` 浏览器 `snapshot_generation` 世代防陈旧引用门禁。
  - 全量主题语义 Token 收敛、生产 `.tsx` 硬编码中文清零、全出口密钥脱敏、`v0.9.0`/`v0.9.12` 历史夹具迁移与写入静默加密备份、生产构建分包（主入口 `< 1.2 MB`）及 Top 8 VS Code 扩展兼容性档案（[docs/extension-compatibility-1.0.md](../extension-compatibility-1.0.md)）。

## 范围说明（ADR-006）

- 本地工程、单测、集成测试与 E2E 门禁已 100% 闭环；外部物理硬件（macOS/Linux 真机）、商业代码签名证书（Windows Authenticode / Apple 公证）与外部 Beta 样本队列按 `ADR-006` 记录为外部环境豁免项。
