# AZ-005 · 支持与发行分层（1.0.0 工程基线）

基线日期：2026-10-19，版本：`1.0.0`（关联 [ADR-006](../adr/ADR-006-1.0-engineering-and-ux-baseline.md) 与 [1.0.0 发布签署报告](../release-decision-1.0.0.md)）。以下为历史工程基线；当前源码版本及发行边界见 [RELEASE.md](../../../RELEASE.md) 和 [实施进度](../progress.md)。2026-10-21 复核发现备份一致性声明需按下表收窄。

| 能力域 | 1.0.0 验证证据 | 当前状态 | 外部硬件/证书豁免说明 (`ADR-006`) |
| --- | --- | --- | --- |
| Windows 桌面宿主与双工作区 (`WorkZ` + `CodeZ`) | `e2e/agentz.spec.tsx`、`R10-05`–`R10-16` 证据包 | **已验证 (Verified - Local Engineering)** | 本地构建、单测、组件集成与跨视图 E2E 100% 通过 |
| Agent 内核契约与浏览器自动化 (`RobotZ`) | `crates/agentz-host/tests/kernel_contract.rs`、`R10-04` 证据 | **已验证 (Verified)** | 五阶段生命周期契约与 `snapshot_generation` 防陈旧引用门禁已固化 |
| WorkZ 隔离工作树、合并预检与非 Git 快照回滚 | `crates/agentz-task-state/src/merge.rs`、`R10-08` 证据 | **已验证 (Verified)** | 支持脏工作树自动提交合入与普通目录任务快照回滚 |
| 可编辑文档画布与人工编辑反馈增量 | `AgentFilePreview.tsx`、`R10-07` / `R10-09` 证据 | **已验证 (Verified)** | `Preview / Edit / Diff` 三态画布与 `<user_recent_edits>` 自动注入 |
| VS Code 扩展宿主 (`HOST_VSCODE_VERSION = 1.96.0`) | `docs/extension-compatibility-1.0.md`、`R10-23` 证据 | **已验证 (Top 8 扩展档案)** | 5 款完全支持、3 款降级运行并由 `compatStore` 显式追踪未实现提案 API |
| 全出口密钥脱敏、Webview 隔离与诊断导出 | `R10-19`、`R10-23`、`diagnostics::tests`、`webviewSecurity.test.ts` | **已验证 (Verified)** | 4 大出口脱敏清零，诊断包附带 `task/env/run` 摘要 |
| 历史数据迁移 (`v0.9.0` / `v0.9.12` -> `1.0.0`) 与离线备份夹具 | `R10-20`、`upgrade-data-backup.verify.mjs`、[AZ-063 复评](AZ-063-2026-10-21-consistency-claim-correction.md) | **本地夹具通过；原生恢复未验证** | `--quiesced true` 是操作者声明；CLI 拒绝检测到的 SQLite sidecar 并验证前后文件清单，但尚无宿主写入静默、原子 SQLite 快照或任务身份原生恢复证据 |
| 生产包体分包与冷启动/内存基线 | `scripts/perf-harness.mjs`、`R10-21` 证据 | **已验证 (Verified)** | 主入口 `706 KB < 1.2 MB`，50 轮资源开闭句柄平衡 `live === 0` |
| macOS / Linux 跨平台构建与商業代码签名 | `.github/workflows/release.yml`、`ADR-006` | **CI 构建定义完备 / 外部证书与真机豁免 (`waived:ADR-006`)** | 商业 Authenticode / Apple Developer 公证证书与非 Windows 物理真机按 `ADR-006` 豁免 |
