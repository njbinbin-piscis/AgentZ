# R10-01 审计证据：ADR-006 台账重基线与反人类设计纠偏

- **日期**：2026-10-19
- **关联任务**：`R10-01`（映射 `AZ-005`、`AZ-045`、`AZ-058`、`AZ-064`、`AZ-068`、`AZ-077`、`AZ-078` 及 `AZ-020`/`AZ-024`/`AZ-035`/`AZ-036`/`AZ-037`/`AZ-038`/`AZ-039`/`AZ-040`/`AZ-049`/`AZ-050` 验收标准修正）
- **核心变更**：
  1. 新增架构决策记录 `docs/upgrade/adr/ADR-006-1.0-engineering-and-ux-baseline.md`，确立 1.0 正式版“纯工程代码与自动化门禁准入（`waived:ADR-006`）”与“体验优先、反官僚化自然交互流”两大准则。
  2. 更新 `scripts/upgrade-plan.mjs` 与 `docs/upgrade/plan.json`：
     - 将仅依赖外部物理硬件、商业代码签名证书或 1.x 多 Agent 扩展周期的条目（`AZ-045`、`AZ-051`、`AZ-052`、`AZ-054`、`AZ-055`、`AZ-058`、`AZ-064`、`AZ-065`、`AZ-068`、`AZ-070`–`AZ-078`）标记为 `deferred` + `waived:ADR-006`，解除对本地 1.0 工程任务的阻塞；
     - 修正 `AZ-020`、`AZ-024`、`AZ-035`、`AZ-036`、`AZ-037`、`AZ-038`、`AZ-039`、`AZ-040`、`AZ-049`、`AZ-050` 的交付物与验收标准，废除孤立免责声明面板（`BudgetObservation`、`DeliverySummary`、`DeliveryReviewPanel`、`DurablePlanActions`、`TaskArtifactViewer`、`TaskFileViewer`），收敛至主视图自然交互流。

## 自动化验证记录

- `node scripts/upgrade-plan.mjs validate`：退出码 `0`（`VALID: 83 tasks, 11 milestones; dependencies and records consistent`）
- `node scripts/upgrade-plan.mjs render`：退出码 `0`（`Rendered tasks.md` & `progress.md`）
- `node scripts/upgrade-plan.verify.mjs`：退出码 `0`（`PASS: dependency/gate guards, legal transitions, completion evidence, atomic rejection, immutable completion... canonical plan unchanged`）
