# AgentZ 1.0.0 正式发布签署报告（Release Decision & Audit Sign-off）

- **产品名称**：AgentZ Desktop (`com.agentz.desktop`)
- **签署版本**：`1.0.0`
- **签署日期**：`2026-10-19`
- **适用架构决策**：[ADR-006：1.0 正式版工程验收基线与去官僚化交互收敛](adr/ADR-006-1.0-engineering-and-ux-baseline.md)
- **发布结论**：**APPROVED（本地工程与自动化门禁 100% 通过，准予签署 1.0.0 正式基线）**

---

## 一、 24 项 1.0 原子任务（`R10-01` – `R10-24`）审计签署总表

| 阶段 | 任务 ID | 任务名称 | 状态 | 审计证据路径 |
| --- | --- | --- | --- | --- |
| Phase 0 | `R10-01` | 编写 `ADR-006` 并重基线化 `docs/upgrade/plan.json` | `DONE` | `docs/upgrade/evidence/R10-01-ledger-rebaseline.md` |
| Phase 0 | `R10-02` | 清理 `.gitignore` 并规范化 Git 工作树跟踪基线 | `DONE` | `docs/upgrade/evidence/R10-02-git-hygiene.md` |
| Phase 0 | `R10-03` | 清退 18 个孤立反人类合规组件并收敛至自然交互流 | `DONE` | `docs/upgrade/evidence/R10-03-ux-debloat.md` |
| Phase 0 | `R10-04` | 固化 Agent 内核五阶段契约测试与浏览器 `snapshot_generation` 校验 | `DONE` | `docs/upgrade/evidence/R10-04-kernel-and-browser-pin.md` |
| Phase 1 | `R10-05` | `CodeZ` 编辑器未保存脏缓冲（Dirty Buffer）冲突保护弹条 | `DONE` | `docs/upgrade/evidence/R10-05-dirty-buffer-guard.md` |
| Phase 1 | `R10-06` | `WorkZ` 任务一键「在 CodeZ 中打开隔离工作树」及顶栏返回胶囊 | `DONE` | `docs/upgrade/evidence/R10-06-workz-codez-handoff.md` |
| Phase 1 | `R10-07` | 人工编辑保存自动记账并向下一轮 Agent 注入 `<user_recent_edits>` | `DONE` | `docs/upgrade/evidence/R10-07-human-edit-feedback.md` |
| Phase 1 | `R10-08` | `WorkZ` 未提交改动自动暂存合入与非 Git 文件夹任务级快照回滚 | `DONE` | `docs/upgrade/evidence/R10-08-merge-and-snapshot.md` |
| Phase 1 | `R10-09` | `WorkZ` 右侧 `AgentFilePreview` 升级为可编辑文档画布（Preview/Edit/Diff） | `DONE` | `docs/upgrade/evidence/R10-09-editable-preview-canvas.md` |
| Phase 2 | `R10-10` | `CodeZ` 全局命令面板（`Ctrl+Shift+P`）与快速打开文件（`Ctrl+P`） | `DONE` | `docs/upgrade/evidence/R10-10-command-palette.md` |
| Phase 2 | `R10-11` | `WorkZ` 与 `CodeZ` 对话流单轮「一键回滚本轮文件修改（`journalUndoTurn`）」 | `DONE` | `docs/upgrade/evidence/R10-11-per-turn-undo.md` |
| Phase 2 | `R10-12` | `WorkZ` 空状态 4 大高频场景卡片与极简化底栏（默认沙盒隔离） | `DONE` | `docs/upgrade/evidence/R10-12-workz-starter-and-bar.md` |
| Phase 2 | `R10-13` | 旁路 LLM 调用统一用量记账与状态栏模型切换 + Token/费用胶囊 | `DONE` | `docs/upgrade/evidence/R10-13-budget-and-statusbar.md` |
| Phase 2 | `R10-14` | 首次启动 3 步极简引导向导与 Swarm/Workflow `Beta` 默认折叠 | `DONE` | `docs/upgrade/evidence/R10-14-onboarding-and-beta-gate.md` |
| Phase 3 | `R10-15` | 拆分 `src/workspaces/workz/index.tsx` 为 4 个职责单一模块 | `DONE` | `docs/upgrade/evidence/R10-15-workz-split.md` |
| Phase 3 | `R10-16` | 拆分 `src/workspaces/codez/index.tsx` 与 `AssistantPanel.tsx` | `DONE` | `docs/upgrade/evidence/R10-16-codez-split.md` |
| Phase 3 | `R10-17` | 拆分后端 `src-tauri/src/commands/chat_turn.rs` 为 4 个子模块 | `DONE` | `docs/upgrade/evidence/R10-17-rust-chat-turn-split.md` |
| Phase 3 | `R10-18` | CSS 硬编码色值收敛至主题语义 Token 与 TSX 硬编码中文清零 | `DONE` | `docs/upgrade/evidence/R10-18-theme-and-i18n-closure.md` |
| Phase 4 | `R10-19` | 全出口密钥脱敏过滤器与扩展 Webview 隔离边界验证 | `DONE` | `docs/upgrade/evidence/R10-19-security-and-webview.md` |
| Phase 4 | `R10-20` | 历史版本（`v0.9.0`/`v0.9.12`）迁移夹具、写入静默与一致性加密备份 | `DONE` | `docs/upgrade/evidence/R10-20-migration-and-backup.md` |
| Phase 4 | `R10-21` | 生产构建 Minify 与 Chunk 拆分优化及 50 轮资源/内存基准门禁 | `DONE` | `docs/upgrade/evidence/R10-21-bundle-and-perf.md` |
| Phase 5 | `R10-22` | 跨视图黄金用户旅程 E2E 自动化测试套件并接入 CI/Release | `DONE` | `docs/upgrade/evidence/R10-22-e2e-golden-journey.md` |
| Phase 5 | `R10-23` | 一键脱敏诊断包导出（含 `task/env/run` 摘要）与 Top 8 扩展兼容档案 | `DONE` | `docs/upgrade/evidence/R10-23-diagnostics-and-extensions.md` |
| Phase 5 | `R10-24` | 统一升级版本号至 `1.0.0`、同步文档矩阵与全量门禁发布签署 | `DONE` | `docs/upgrade/evidence/R10-24-release-signoff.md` |

---

## 二、 全量自动化验证门禁结果

1. **版本与文档一致性门禁**：
   - 命令：`node scripts/upgrade-version-check.mjs`
   - 结果：`AgentZ 1.0.0: package, lockfile, Cargo, Tauri, README, RELEASE and candidate docs agree; README/RELEASE local links resolve; extension-host 0.1.0 package and lockfile agree`
2. **台账完整性与发布门禁**：
   - 命令：`node scripts/upgrade-plan.mjs validate` / `node scripts/upgrade-plan.verify.mjs` / `npm run test:scripts`
   - 结果：全部里程碑（`M0`–`M8`, `MB`）状态为 `passed`，`AZ-069` 状态为 `done`，发布门禁脚本测试 100% 通过。
3. **前端类型检查与全量单测 / E2E 门禁**：
   - 命令：`npm run typecheck` (`tsc --noEmit`)、`npm test` (`vitest run`)、`npm run test:e2e`
   - 结果：`0` TypeScript 错误；全量单元测试与 `e2e/agentz.spec.tsx` 黄金旅程 100% 通过。
4. **生产包体与性能句柄平衡门禁**：
   - 命令：`node scripts/perf-harness.mjs`
   - 结果：`passed: true`，主入口 `index-*.js` 体积 `706,750 bytes`（`< 1.2 MB` 预算），50 轮资源开闭句柄泄漏为 `0`，RSS 内存 `53.62 MB`（`< 450 MB` 预算）。
5. **数据清点与一致性加密备份恢复门禁**：
   - 命令：`node scripts/upgrade-data-inventory.verify.mjs`、`node scripts/upgrade-data-backup.verify.mjs`
   - 结果：写入静默、WAL/SHM 活跃阻断、AES-256-GCM 加密备份与篡改拒绝校验 100% 通过。
6. **Rust 后端核心测试门禁**：
   - 命令：`cargo test -p agentz-host --test kernel_contract`、`cargo test -p agentz-task-state`、`cargo test -p agentz-desktop --lib`
   - 结果：内核五阶段契约、浏览器 `snapshot_generation` 校验、脱敏导出、历史配置迁移、预算旁路记账全部通过。

本签署仅覆盖所列本地工程与自动化检查。备份 CLI 的写入静默依赖调用者提供 `--quiesced true` 声明，WAL/SHM/journal 检查和清点前后哈希比对不能证明在线原子 SQLite 快照；原生恢复、历史任务/会话/环境绑定、干净机器安装与回滚、签名公证及跨平台实机旅程尚无验收记录。详见 [AZ-063 后续审计](evidence/AZ-063-2026-10-19-post-signoff-audit.md)及 ADR-006 豁免范围。
