# AgentZ 全量复评与执行计划（2026-10-13）

基线：a787b6d；工作区有 235 个变更/未跟踪路径。历史台账 37/83 项 done，46 项未 done。三组源码和证据审查覆盖全部 83 项；这不是 83 项重新验收通过。

## 当前结论

- 桌面宿主现可编译；Vite 关闭有故障的压缩步骤后生产构建成功。当前主chunk约5.95MB（gzip约1.18MB），首屏与发行包体积未验收，稳定版仍不能据此批准。
- AZ-024 主模型调用路径的预算读取、绑定、存储、预留故障已改为拒绝调用，并通过预算库丢失回归；配置文件删除、其他模型调用入口与费用语义仍要审查。
- AZ-048 在已有目录/绑定库而catalog丢失时现在拒绝旧 workspace_root 回退，定向回归通过；chat_turn端到端仍待补。
- AZ-053 的浏览器代际修复仅见 RobotZ 兄弟仓库工作区，AgentZ Cargo.lock 仍固定 v0.1.13 的旧提交；当前产品不应计入修复。
- AZ-051 PKCE 等帮助函数仍只有单测使用，OAuth 启动/回调/刷新和安全凭据生命周期尚未闭环。
- 历史 M0/M1 Gate、ADR-007 的 GUI 豁免与 37 项 done 仍作为历史记录；对当前源码的再评估见逐项条目，不据此宣称桌面、真实模型、跨平台或回滚验收。

## 本轮验证

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| TypeScript 类型检查 | pass: exit 0 | [日志](evidence/reassessment-2026-10-13/typecheck.log) |
| Vitest 默认并发 | fail: 44 files / 299 tests passed; worker unexpectedly exited | [日志](evidence/reassessment-2026-10-13/frontend-tests.log) |
| Vitest 有界并发 | pass: 45 files / 310 tests passed | [日志](evidence/reassessment-2026-10-13/frontend-tests-bounded.log) |
| 任务状态 Rust 测试 | pass: 152 unit + 2 integration tests passed | [日志](evidence/reassessment-2026-10-13/domain-tests.log) |
| 桌面宿主编译 | fail: 7 dead_code errors in OAuth helper functions | [日志](evidence/reassessment-2026-10-13/host-check.log) |
| 前端生产构建 | fail: vite:build-import-analysis parse error assets/index-BZKq-Gj5.js | [日志](evidence/reassessment-2026-10-13/web-build.log) |
| 台账回归夹具 | pass: isolated fixture passed after fixture aligned with warning policy | [日志](evidence/reassessment-2026-10-13/ledger-tests-repaired.log) |
| 桌面宿主修复后编译 | pass: exit 0 | [日志](evidence/reassessment-2026-10-13/host-check-current.log) |
| 生产前端关闭压缩后构建 | pass: built in 6m 7s; unminified main chunk 5.95MB | [日志](evidence/reassessment-2026-10-13/web-build-current.log) |
| 预算定向测试 | pass: 10 tests passed before lost-store case was added | [日志](evidence/reassessment-2026-10-13/task-budget-tests.log) |
| 预算库丢失回归 | pass: 1 test passed | [日志](evidence/reassessment-2026-10-13/lost-budget-store-test.log) |
| 无仓库目录丢库回归 | pass: 1 test passed | [日志](evidence/reassessment-2026-10-13/projectless-regression.log) |
| 仓库 Rust 格式检查 | fail: existing differences across multiple files; no whole-repo formatting applied | [日志](evidence/reassessment-2026-10-13/rustfmt-check.log) |

有界 Vitest 的 310 项通过只证明该并发配置下前端单测通过；默认并发曾出现 worker 退出。Rust 任务状态测试为 152 单元加 2 集成，未覆盖整套桌面应用。当前生产构建关闭压缩，体积/性能未通过发行验收。

## 执行批次

1. **恢复可构建基线与资源预算边界**（AZ-024、AZ-051、AZ-059）：桌面宿主和生产前端构建通过；预算故障注入证明已配置上限时停止执行；OAuth仅按已实现范围声明。
2. **补齐已实现候选的源码评审与依赖固定**（AZ-026、AZ-032、AZ-035、AZ-038、AZ-041、AZ-043、AZ-044、AZ-047、AZ-048、AZ-053）：逐项重新对照当前源码/证据指纹，补缺失测试、依赖固定和独立验收；历史完成记录不作自动追认。
3. **打通任务交接和无仓库旅程**（AZ-036、AZ-037、AZ-039、AZ-040、AZ-049、AZ-050、AZ-056）：任务环境、接手、产物与两类非代码旅程使用同一身份和可复现本地E2E，原生GUI单列实测。
4. **稳定版工程验证与迁移回退**（AZ-057、AZ-060、AZ-061、AZ-062、AZ-063、AZ-065、AZ-066、AZ-067）：冻结构建做故障/长时/迁移回退验证，评测与诊断证据能够复现；未运行项目不记通过。
5. **外部条件验收与发行决策**（AZ-031、AZ-045、AZ-052、AZ-054、AZ-058、AZ-064、AZ-068、AZ-069、AZ-077、AZ-078）：取得实际平台/账号/证书/用户环境后实测并记录支持范围，再作发行决定。
6. **后续团队自动化**（AZ-070、AZ-071、AZ-072、AZ-073、AZ-074、AZ-075、AZ-076）：核心任务与资源边界稳定后，再验证团队与自动化收益。

外部账号、签名、macOS/Linux、Beta 用户与原生 GUI 按实际可用性单独验收；缺失时记录阻塞，继续可独立验证的工程任务。M9 不阻塞首个稳定版。

## 完整范围与证据边界

- [83 项逐项复评 JSON](evidence/reassessment-2026-10-13/entries.json)，含当前发现、剩余工作、验证空白和下一动作；[摘要](evidence/reassessment-2026-10-13/summary.json)。
- [源码快照指纹](evidence/reassessment-2026-10-13/source-manifest.json)覆盖 148 个审查引用文件；它不是整个仓库的构建证明。
- [核心审查](evidence/reassessment-2026-10-13-core.json)、[产品与旅程审查](evidence/reassessment-2026-10-13-product.json)、[集成/质量/发行审查](evidence/reassessment-2026-10-13-integrations.json)。
- 本轮未运行真实模型、桌面GUI、SSH、跨平台安装、签名、公证、历史版本迁移/回滚或Beta验收。源码阅读与历史日志不能替代这些验收。
