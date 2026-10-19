# AgentZ 实施进度

更新时间：2026/10/19 16:32:44（Asia/Tokyo）；由台账自动生成。

任务：83；已验收：65；执行/评审：0；阻塞：0。

Gate通过：M0, M1, M2, M3, M4, M5, M6, M7, M8, M9, MB。

## 当前全面复评

复评日期：2026-10-13；覆盖：83/83。历史done与Gate记录保留，不代表当前源码重新通过验收。

报告：[当前评估与执行计划](reassessment-2026-10-13.md)。源码与检查范围见报告；未经评审修改不自动追认完成。

- 恢复可构建基线与资源预算边界：AZ-024, AZ-051, AZ-059；桌面宿主和生产前端构建通过；预算故障注入证明已配置上限时停止执行；OAuth仅按已实现范围声明。。
- 补齐已实现候选的源码评审与依赖固定：AZ-026, AZ-032, AZ-035, AZ-038, AZ-041, AZ-043, AZ-044, AZ-047, AZ-048, AZ-053；逐项重新对照当前源码/证据指纹，补缺失测试、依赖固定和独立验收；历史完成记录不作自动追认。。
- 打通任务交接和无仓库旅程：AZ-036, AZ-037, AZ-039, AZ-040, AZ-049, AZ-050, AZ-056；任务环境、接手、产物与两类非代码旅程使用同一身份和可复现本地E2E，原生GUI单列实测。。
- 稳定版工程验证与迁移回退：AZ-057, AZ-060, AZ-061, AZ-062, AZ-063, AZ-065, AZ-066, AZ-067；冻结构建做故障/长时/迁移回退验证，评测与诊断证据能够复现；未运行项目不记通过。。
- 外部条件验收与发行决策：AZ-031, AZ-045, AZ-052, AZ-054, AZ-058, AZ-064, AZ-068, AZ-069, AZ-077, AZ-078；取得实际平台/账号/证书/用户环境后实测并记录支持范围，再作发行决定。。
- 后续团队自动化：AZ-070, AZ-071, AZ-072, AZ-073, AZ-074, AZ-075, AZ-076；核心任务与资源边界稳定后，再验证团队与自动化收益。。

- AZ-001 固定首批用户与三条核心旅程：document_complete；下一步：版本化契约与当前发布范围对齐。
- AZ-002 固定任务与环境身份契约：document_complete；下一步：审阅实际迁移与重开边界。
- AZ-003 冻结审查复现与回归夹具：historical_baseline；下一步：按当前源码重新运行确认缺陷回归集。
- AZ-004 建立成功率成本与性能基线：historical_limited_measurement；下一步：新版本重采样；真实桌面/复杂仓库任务另验收。
- AZ-005 制定兼容支持与发布分层：document_complete；下一步：同步当前版本与实际支持范围。
- AZ-006 确定容量预算和可执行排期：document_complete；下一步：按当前剩余工作量重排依赖和容量。
- AZ-007 持久化任务环境登记：bounded_source_review；下一步：当前精确版本登记/幂等/迁移测试。
- AZ-008 后端解析并验证执行环境：bounded_source_review；下一步：复核全部执行入口require_binding传播并测丢失绑定。
- AZ-009 WorkZ 按任务恢复环境：bounded_source_review；下一步：补可执行宿主A/B恢复证据；保留GUI豁免声明。
- AZ-010 提供明确起点与非 Git 创建策略：bounded_source_review；下一步：新版本dirty/specialpath/非Git回归。
- AZ-011 采集完整 worktree 变更集：bounded_source_review；下一步：完整工作树类型矩阵当前复跑。
- AZ-012 统一 journal 的环境路由：bounded_source_review；下一步：当前隔离journal路由/undo跨任务用例。
- AZ-013 更新完整变更评审界面：bounded_source_review；下一步：当前UI测试及端到端review版本检查。
- AZ-014 保存可恢复归档快照：bounded_source_review；下一步：当前恢复/故障/ignored文件矩阵。
- AZ-015 约束清理对象并报告分步失败：bounded_source_review；下一步：当前身份错配/分步故障/恢复测试。
- AZ-016 实现应用与合并事务预检：bounded_source_review；下一步：当前事务中断与恢复/并行冲突回归。
- AZ-017 持久化 Run 与 Attempt 状态机：bounded_source_review；下一步：评审当前共享变动并重冻manifest后执行状态矩阵。
- AZ-018 实现耐久事件与断流游标：bounded_source_review；下一步：当前断流去重游标/持久事件故障测试；不要扩到team。
- AZ-019 启动时对账中断任务：bounded_source_review；下一步：当前Windows kill/reopen试验，复核runtime/lock变动。
- AZ-020 持久计划并修正续聊清除语义：bounded_source_review；下一步：当前续聊/merge/replacement持久计划用例。
- AZ-021 覆盖排队与执行中的取消：bounded_source_review；下一步：当前queue/run/grace/uncooperative回归。
- AZ-022 设置进程与工具恢复契约：bounded_source_review；下一步：当前native shell/code timeout/cancel清理与恢复。
- AZ-023 固化模型重试分类与预算：bounded_source_review；下一步：当前HTTP/cancelbackoff用例及分类边界。
- AZ-024 实现任务整体资源与费用预算：local_fail_closed_fix_unreviewed；下一步：独立复核fail-closed改动；覆盖已配置文件丢失、所有模型入口与真实费用语义。。
- AZ-025 定义外部动作幂等与恢复对账：domain_only；下一步：明确接入宿主/实际外部动作并测丢响应，禁止自动重发。
- AZ-026 发布并接入内核契约版本：new_local_freeze_candidate_unreviewed；下一步：Review five changed files semantically, then independent current-candidate review and formal M2 decision.。
- AZ-027 冻结权限分级与授权有效范围：current_source_read_only_no_tests_rerun；下一步：保留文档范围完成，实际资源执行逐项由028/034/后续任务验收。。
- AZ-028 实现持久审批意图与范围校验：current_source_read_only_no_tests_rerun；下一步：重跑当前approval领域/host/PermissionCard并对齐声明入口清单。。
- AZ-029 让秘密加密失败阻止保存：current_source_read_only_no_tests_rerun；下一步：重跑损坏密钥/加密错误/写入失败与格式downgrade夹具并核对kernel source provenance。。
- AZ-030 接入系统凭据保护与迁移：current_source_read_only_no_tests_rerun；下一步：分离Windows已测范围与其他平台候选；检查provider新增源码来源，重跑迁移/保护失败。。
- AZ-031 贯通秘密引用与脱敏出口：model_mcp_batched_reference_and_v2_persistence_candidate_unreviewed；下一步：Review cross-process settings writes, historical plaintext snapshots and real v1 migration/rollback, then scan remaining outlets and confirm per-key deletion in native UI.。
- AZ-032 修复 Webview 功能与隔离策略：current_source_read_only_no_tests_rerun；下一步：保留真实Tauri正反验收缺口；验证打包nonce供给及跨panel资源泄漏。。
- AZ-033 记录技能扩展来源与授权变更：current_source_read_only_no_tests_rerun；下一步：重跑service install-update-uninstall真实临时DB/目录夹具并核查扩展范围措辞。。
- AZ-034 实现受限执行的首个策略：current_source_read_only_no_tests_rerun；下一步：重跑当前Windows正反文件/网络/取消/ACL案例，审新增kernel差异。。
- AZ-035 提供统一任务详情读取接口：implementation_candidate_unreviewed；下一步：Independently review exact source content; run cross-view task/environment navigation and desktop read acceptance in the unified validation phase.。
- AZ-036 CodeZ 打开当前任务环境：read_only_codez_route_candidate_unreviewed；下一步：Implement task-bound host authorization for CodeZ editor, terminal, debug and preview operations and same-source session continuation; then perform unified native handoff acceptance.。
- AZ-037 实现交接与人类编辑反馈：current_source_read_only_no_tests_rerun；下一步：设计最小handoff状态与人类修改上下文消费，不能把stop/newRun称pause resume。。
- AZ-038 统一产物清单与版本：current_source_read_only_no_tests_rerun；下一步：复核artifacts/host新漂移并重跑domain/host/client，明确文本候选支持范围。。
- AZ-039 改造交付评审与验收动作：current_source_read_only_no_tests_rerun；下一步：按三旅程补回执来源/评审动作；先复核AssistantPanel新pin漂移。。
- AZ-040 简化首用输入与运行状态：current_session_approval_and_latest_tool_candidate_unreviewed；下一步：Perform five representative walkthroughs and native approval/first-run journeys; independently review the current source.。
- AZ-041 建立脏缓冲与外部改动冲突流程：current_source_read_only_no_tests_rerun；下一步：审index新修改并复现typing/save/watcher/恢复交错；明确保留/查看/合并与磁盘base。。
- AZ-042 语言服务崩溃恢复：current_source_read_only_no_tests_rerun；下一步：核对ADR008豁免适用与原verification，修文档窗口描述并保留真实TS/Rust异常演练项。。
- AZ-043 调试会话生命周期回归：current_source_read_only_no_tests_rerun；下一步：重跑当前host/client/controller后本地adapter launch/attach与异常退出记录。。
- AZ-044 终端生命周期与路径回归：current_source_read_only_no_tests_rerun；下一步：独立审当前Job/terminal新增改动并重跑multiPTY/close/error，完成Task scope路由后验。。
- AZ-045 远程断连与环境恢复：current_source_read_only_no_tests_rerun；下一步：准备可认证owned POSIX fixture验证文件/LSP/terminal/Agent断连重连且零重复写。。
- AZ-046 建立首批真实扩展兼容档案：current_source_read_only_no_tests_rerun；下一步：固定6扩展ID/version/来源/能力清单，做安装运行升级正反矩阵，不标未知兼容。。
- AZ-047 整理日常工程交互一致性：current_source_read_only_no_tests_rerun；下一步：复核当前dock/terminal修改，扩展到输入审批评审产物/空态error中英走查。。
- AZ-048 无仓库任务独立工作目录：host_trusted_projectless_turn_and_observation_wired_local_checks_passed；下一步：Run a real first turn and continuation in separate projectless sessions, reopen after restart, verify catalog-loss refusal and legacy free-session compatibility; review final source and rollback evidence before acceptance.。
- AZ-049 研究报告交付模板：structured_report_render_wired_to_task_bound_artifact_capture_local_checks_passed；下一步：Review one fixed research report and open each source locator, exercise native task-bound capture/read/edit/continue, then independently review the final source and acceptance criteria.。
- AZ-050 文档产物编辑预览闭环：host_resolved_versioned_continuation_local_checks_passed；下一步：Run native create/edit/restore/continue/archive journey, verify HTML/Markdown preview and failed-send recovery, then independently review final source and upstream contracts.。
- AZ-051 连接器授权生命周期：local_oauth_expiry_callback_mcp_revalidation_and_lifecycle_guard_candidate_unreviewed；下一步：Review the current PKCE callback, refresh and per-call MCP guard; decide provider-side revocation support, then run one real provider authorization/expiry/revoke journey and in-flight cancellation case during unified acceptance.。
- AZ-052 外部发送与发布交付流程：partial；下一步：Connect POST/PUT/DELETE and gateway.send to prepare/frozen approval/start/completion/unknown reconciliation; create sandbox-local tests before live external account receipt.。
- AZ-053 浏览器上下文与抢占恢复：unshipped_dependency_fix_local_tests_pass；下一步：Review and pin an immutable RobotZ revision with generation-aware refs, then run AgentZ's pinned stale-ref and delayed-action takeover fixtures.。
- AZ-054 Windows 桌面操作最小闭环：not_started；下一步：Define one supported Windows application/action set, add host focus identity and user takeover gate plus sensitive-region policy; test pure guards first and preserve native unverified label.。
- AZ-055 建立跨任务桌面资源租约：not_started；下一步：After browser/native action binding, implement distinct task-attempt resource capability with queue/busy/revoke/drop and cancellation fixtures; do not repurpose process owner lease.。
- AZ-056 核心生命周期 E2E 门禁：isolated_kernel_lifecycle_gate_candidate_unreviewed；下一步：Run the candidate workflow after resource preflight is satisfied; add host/native A/B, review/file-list and recovery receipts before acceptance.。
- AZ-057 Windows 故障与长时运行基准：windows_resource_sampler_orphan_tracking_candidate_unreviewed；下一步：Freeze hardware/build/budget and run AgentZ cold-start, fault and two-hour workload with the sampler; preserve raw receipts.。
- AZ-058 macOS/Linux 核心旅程验证：platform_journey_protocol_prepared_native_unverified；下一步：After AZ-056/AZ-057 candidate freeze, execute separate macOS and Linux native cards with model and credential fixtures; record differences in the support matrix.。
- AZ-059 优化首屏与重型资源加载：code_map_lazy_chunk_minified_build_unreviewed；下一步：Measure controlled initial load and graph-open load, then run native offline journey after AZ-040 review.。
- AZ-060 消除长任务资源增长：owned_pty_model_listener_watcher_extension_graph_snapshot_candidate；下一步：Exercise concurrent Tauri terminal-create command invocations and a real repository graph-build close, then run the two-hour close/restart soak against a frozen AZ-057 budget.。
- AZ-061 统一诊断关联和脱敏导出：bounded_redacted_log_manifest_candidate_unreviewed；下一步：Verify task/run/environment/tool fields with a real failing tool and native export, then run all-source secret scan after AZ-031.。
- AZ-062 建立评测趋势与质量门禁：local_trend_gate_and_count_reconciliation_candidate_unreviewed；下一步：Wire the gate to the stable lifecycle harness, collect repeated actual desktop/model summaries, then perform unified review.。
- AZ-063 版本化数据迁移与备份恢复：offline_encrypted_backup_output_entry_hardened_native_unverified；下一步：Integrate enforced writer quiescence and coherent SQLite snapshot capture; verify credential/task/session/environment binding in a restored new location using historical fixtures and a native recovery journey.。
- AZ-064 签名、公证与安装器验收：local_gate_script_unwired_after_0_9_15_waiver_native_unverified；下一步：For the next non-waived release, wire release-readiness before publishing; obtain signing/notarization identities and clean test machines, then collect native install/start/uninstall/restore receipts before AZ-069 acceptance.。
- AZ-065 可信更新与失败恢复：not_started；下一步：After protected backup and trusted signing, add authenticated channel/manifest/update checks plus interrupted download/install recovery and explicit schema compatibility refusal; test locally before service deployment.。
- AZ-066 首次成功任务引导：first_task_project_tool_connection_guidance_local_checks_passed；下一步：Review first-task UI with real tool/environment failures; run five clean signed-build first-use journeys using an authorized model after upstream dependencies are accepted.。
- AZ-067 同步支持文档与发布说明：current_0_9_15_local_docs_consistent_native_unverified；下一步：Reconcile the support matrix with actual extension and macOS/Linux journeys, then review all current docs and run the CI version check on the frozen candidate.。
- AZ-068 运行 Beta 与支持分流：local_beta_intake_prepared；下一步：After engineering prerequisites, obtain named owner, cohort and window; run actual authorized Beta and record triage outcomes.。
- AZ-069 完成稳定版发布决策：not_started；下一步：Assemble explicit gate matrix after engineering/backup/signing/Beta dependencies; unresolvedP0/security/recovery gates cannot be accepted by missing evidence.。
- AZ-070 成员任务环境与集成契约：partial；下一步：Add member environment and artifact/output provenance contracts under real task binding, explicit integration owner and conflict policy; build concurrent same-file/environment fixtures before team defaults.。
- AZ-071 团队任务预算与失败传播：partial；下一步：Connect one team-owned durable budget and member reservations, cancellation fanout with settlement acknowledgement and partial-failure attribution; avoid copying limits per member.。
- AZ-072 持久化自动化触发与去重：durable_trigger_claim_domain_candidate_unreviewed；下一步：Extend the verified-scope reminder source to controlled task execution with AZ-025 permits and budgets; exercise real sleep/resume and native disable/click journeys.。
- AZ-073 建立通知收件箱与静默策略：persisted_inbox_with_rotating_approval_reconciliation_candidate_unreviewed；下一步：Exercise native reminder/approval click-to-task and unchanged-state quietness; verify multiple-read backlog behavior and complete AZ-072 trigger integration.。
- AZ-074 工作流检查点与人工续跑：partial；下一步：Unify node attempt/checkpoint/approval identities and commit action receipts before cursor advance; explicit unknown-result reconciliation, restart no-repeat and exact-human-node fixtures.。
- AZ-075 验证多 Agent 相对收益：matched_comparison_protocol_prepared_unreviewed；下一步：After team scope and budget contracts, freeze a real experiment card and run paired single/read-only-parallel/implementation-team repeats with actual model, cost, intervention, and oracle receipts.。
- AZ-076 发布自动化与团队受控能力：partial；下一步：Define per-capability supported scenarios/defaults and authoritative disable/revoke switch after prerequisites; UI limits must reflect unverified platforms, and defaults require actual benefit/safety receipts.。
- AZ-077 macOS 系统凭据保护适配：keychain_delete_idempotency_cross_target_checked_native_unverified；下一步：Choose and verify Keychain accessibility and signed-identity recovery policy in a real macOS test account; run temporary-item read/write, lock, revoke, corruption and migration tests with cleanup receipts.。
- AZ-078 Linux 系统凭据保护适配：linux_secret_service_cleanup_retry_fixture_cross_target_checked_native_unverified；下一步：Add a bounded service boundary and controlled key-create retry with fault fixtures; then run the ignored temporary-item test and unavailable-bus, lock, prompt, corruption, revoke and migration cases in a real Linux user session.。
- AZ-079 修复同轮重复编辑的完整Undo：bounded_source_review；下一步：当前重复写/新建再改/失败undone回归。
- AZ-080 复查递归预检之后的路径替换：bounded_source_review；下一步：当前junction时序fixture复跑；句柄no-follow另立范围。
- AZ-081 让固定内核遍历遵守排除目录：bounded_source_review；下一步：当前忽略canary与预检一致性测试。
- AZ-082 过滤 LSP 结构化返回中的资源路径：bounded_source_review；下一步：当前嵌套URI/rename/edit/异常scheme fixture；live LSP另测。
- AZ-083 修复 AppContainer 后代回收验收的不稳定性：bounded_source_review；下一步：当前Windows handshake连续回归，保留失败与超时断言。

## 当前执行

- 暂无活动任务。

## 阻塞

- 暂无登记阻塞。

## 下一批可执行


## 已完成与证据

- AZ-001 固定首批用户与三条核心旅程：[证据](evidence/AZ-001-contract.md)；冻结首批用户、三条旅程和首期范围。
- AZ-002 固定任务与环境身份契约：[证据](evidence/AZ-002-identity-adr.md)；确定稳定Task和Environment身份与后端权威。
- AZ-003 冻结审查复现与回归夹具：[证据](evidence/AZ-003-baseline.json)；固定基线与可重跑Git夹具，保存源码追踪证据。
- AZ-004 建立成功率成本与性能基线：[证据](evidence/AZ-004-live-design.md)；[证据](evidence/AZ-004-live-execution.md)；[证据](evidence/AZ-004-live-results-v1.json)；[证据](evidence/AZ-004-live-results-review.md)；首次固定12项synthetic真实kernel基线×3，36样例/108调用；自动33/36、独立语义复核36/36分列，记录耗时tokens价格估算与未验项，不提升MB/桌面Gate。
- AZ-005 制定兼容支持与发布分层：[证据](evidence/AZ-005-support-matrix.md)；冻结预览Beta稳定版与已验证能力分类。
- AZ-006 确定容量预算和可执行排期：[证据](evidence/AZ-006-current-cycle.md)；冻结本轮执行顺序，不虚构长期人力与日期。
- AZ-007 持久化任务环境登记：[证据](evidence/AZ-007-registry.md)；持久任务环境登记与冻结起点已实现，4项SQLite测试及宿主check通过。
- AZ-008 后端解析并验证执行环境：[证据](evidence/AZ-008-execution.md)；每轮后端解析验证环境，排队后再次校验，路径重定向和失效不回退。
- AZ-009 WorkZ 按任务恢复环境：[证据](evidence/AZ-009-routing.md)；[证据](evidence/AZ-009-native-capture-attempt.md)；[证据](evidence/AZ-009-native-candidate-preparation.md)；[证据](evidence/AZ-009-native-candidate-running.md)；[证据](evidence/AZ-009-native-input-limitation.md)；[证据](evidence/AZ-009-desktop-state-request.md)；[证据](evidence/AZ-009-visible-candidate-retry.md)；[证据](evidence/ADR-007-gui-waiver-wip.md)；代码与单元测试完成；按 ADR-007 用户豁免 GUI 验证。GUI 未在本机原生验证。。
- AZ-010 提供明确起点与非 Git 创建策略：[证据](evidence/AZ-010-explicit-start.md)；支持显式提交或当前本地源码起点，冻结SHA，不修改源HEAD/index；非Git及unborn不自动初始化提交。
- AZ-011 采集完整 worktree 变更集：[证据](evidence/AZ-011-change-set.md)；冻结基点采集四类工作区变更，NUL路径、来源、改名、二进制和结构化错误已交付。
- AZ-012 统一 journal 的环境路由：[证据](evidence/AZ-012-journal-routing.md)；journal查询与回滚统一任务根；主目录及A/B同名编辑、重开和独立Undo验证通过。
- AZ-013 更新完整变更评审界面：[证据](evidence/AZ-013-review-ui.md)；[证据](evidence/AZ-013-parallel-review.md)；[证据](evidence/ADR-007-gui-waiver-wip.md)；代码与单元测试完成；按 ADR-007 用户豁免 GUI 验证。GUI 未在本机原生验证。。
- AZ-014 保存可恢复归档快照：[证据](evidence/AZ-014-archive.md)；独立index与原始Git blob保存源码快照，版本化元数据和可达ref保留；新目录恢复逐文件核对，原HEAD/index不变。
- AZ-015 约束清理对象并报告分步失败：[证据](evidence/AZ-015-retained-cleanup.md)；旧强制删除替换为完整checkout保留，持久身份与运行租约约束，分步失败和恢复位置可见。
- AZ-016 实现应用与合并事务预检：[证据](evidence/AZ-016-merge-design.md)；[证据](evidence/AZ-016-merge-preflight.md)；[证据](evidence/AZ-016-merge-transaction.md)；稳定任务与冻结HEAD授权合并，仓库串行和运行租约预检，恢复ref/源码快照与真实分步失败已接宿主和评审。
- AZ-017 持久化 Run 与 Attempt 状态机：[证据](evidence/ADR-005-candidate-implementation.md)；[证据](evidence/AZ-017-durable-state-candidate.md)；[证据](evidence/AZ-017-implementation-manifest-scoped-r2.json)；[证据](evidence/AZ-017-implementation-manifest-scoped-r2.json)；Run/Attempt 状态机：非法跳转被拒绝（穷举 7x7 集成测试）；运行结束不自动发布；历史失败与新 attempt 分开。cargo test 147 单元 + 2 集成通过。。
- AZ-018 实现耐久事件与断流游标：[证据](evidence/AZ-018-events-candidate.md)；[证据](evidence/AZ-018-run-events-integration.md)；[证据](evidence/AZ-018-event-replay-ipc.md)；[证据](evidence/AZ-018-current-run-ui-integration.md)；[证据](evidence/AZ-018-metadata-integrity-integration.md)；[证据](evidence/AZ-018-environment-readonly-integration.md)；[证据](evidence/AZ-018-chat-stream-integration.md)；[证据](evidence/AZ-018-passive-preview-integration.md)；[证据](evidence/AZ-018-committed-notice-integration.md)；[证据](evidence/AZ-018-budget-throughput-integration.md)；[证据](evidence/AZ-018-incremental-replay-integration.md)；[证据](evidence/AZ-018-two-phase-replay-integration.md)；[证据](evidence/AZ-018-local-candidate-freeze.md)；[证据](evidence/AZ-018-completion.md)；本地单 Agent 耐久事件已交付：标识、递增序号、去重、断流补齐、按任务隔离；149+4+6 项相关测试通过；远程/团队/池化不在范围。
- AZ-019 启动时对账中断任务：[证据](evidence/current-release-scope.md)；[证据](evidence/AZ-019-owner-integration-design.md)；[证据](evidence/AZ-019-explicit-recovery-candidate.md)；[证据](evidence/AZ-019-recovery-integration.md)；[证据](evidence/AZ-019-owner-lease-implementation.md)；宿主 owner 租约（OS 独占锁，绑定 Binding 记录）+ 启动对账：杀进程故障注入下僵尸 running 标为 interrupted，活 owner 保留，不重放工具；无租约 Run 保留为 unverifiable。
- AZ-020 持久计划并修正续聊清除语义：[证据](evidence/AZ-020-plan-candidate.md)；[证据](evidence/AZ-020-host-service-candidate.md)；[证据](evidence/AZ-020-host-service-candidate.md)；[证据](evidence/AZ-020-host-service-review.md)；[证据](evidence/AZ-020-durable-tool-candidate.md)；[证据](evidence/AZ-020-durable-tool-review.md)；[证据](evidence/AZ-020-heartbeat-environment-lease.md)；[证据](evidence/AZ-020-chat-turn-wiring-review.md)；[证据](evidence/AZ-020-plan-intent-actual-gate.md)；[证据](evidence/AZ-020-codez-plan-controls.md)；[证据](evidence/AZ-020-workz-plan-controls.md)；[证据](evidence/AZ-020-closure-evidence.md)；续聊保留开放计划：省略 merge 默认合并，替换不丢未完成项；显式取消/替换经 chat_plan_apply；重启（同进程 SQLite 重开）保留计划。
- AZ-021 覆盖排队与执行中的取消：[证据](evidence/AZ-021-cancel-integration-baseline.md)；[证据](evidence/AZ-021-owned-completion-candidate.md)；[证据](evidence/AZ-021-host-cancellation-review.md)；[证据](review-execution-map.md)；[证据](evidence/AZ-021-cancellation-integration.md)；[证据](evidence/AZ-021-cancel-grace-fix-r4.md)；排队与执行中取消闭环：宿主 cancelling→cancelled、宽限收尾、真实 shell 与 code_run 取消夹具、无协作工具宽限后中断；GUI Stop 由 ADR-008 豁免。
- AZ-022 设置进程与工具恢复契约：[证据](evidence/AZ-022-execution.md)；[证据](evidence/AZ-022-windows-e2e.md)；Kernel recovery contract (recovery_v1, Job Object tree termination) verified end-to-end in AgentZ on Windows: a real two-level cmd tree started via the kernel shell tool is terminated on timeout and on cancel, with structured outcome and tree_terminated termination and no surviving marker-carrying processes. UAC/elevation and GUI are excluded and covered by ADR-008.。
- AZ-023 固化模型重试分类与预算：[证据](evidence/AZ-023-retry-baseline.md)；[证据](evidence/AZ-023-error-class-candidate.md)；[证据](evidence/AZ-023-retry-integration.md)；[证据](evidence/AZ-023-reconstruction.md)；[证据](evidence/AZ-023-http-reconstruction.md)；[证据](evidence/AZ-023-review-fix-r3.md)；Retry classification (401/403 permanent, quota permanent, 5xx transient, 429 rate-limited) and bounded budget verified by classification/loop cases and 4 real local HTTP fault cases; cancel during backoff interrupts within bounds (lower and upper bound); candidate patch chain reproduces on pinned baseline. Kernel-only, no GUI surface.。
- AZ-024 实现任务整体资源与费用预算：[证据](evidence/AZ-024-budget-integration-baseline.md)；[证据](evidence/AZ-024-budget-core-review.md)；[证据](evidence/AZ-024-accounting-core-integration.md)；[证据](evidence/AZ-024-reservation-core-integration.md)；[证据](evidence/AZ-024-reservation-core-review.md)；[证据](evidence/current-release-scope.md)；[证据](evidence/AZ-024-closure-evidence.md)；[证据](evidence/AZ-024-closure-evidence.md)；按用量预算收口：token、active 时长、并发配额按任务累计，超限拒绝并给出 budget denied 说明，续跑不重置，未知价格明示；金额成本不在本任务范围（另立任务）。
- AZ-025 定义外部动作幂等与恢复对账：[证据](evidence/AZ-025-contract.md)；[证据](evidence/AZ-025-execution.md)；[证据](evidence/AZ-025-closure-evidence.md)；External action intent, single-dispatch permit, lost-result reconciliation without blind replay, and atomic secret-free audit implemented in agentz-task-state (external_action.rs); 12 real SQLite fixtures including a two-connection permit race and an in-process controlled service that loses its result, then reopens with no replay. Domain only; no host provider adapter or real external service.。
- AZ-026 发布并接入内核契约版本：[证据](evidence/AZ-026-contract.md)；[证据](evidence/AZ-026-execution.md)；[证据](evidence/AZ-026-independent-review.md)；[证据](evidence/R10-04-kernel-and-browser-pin.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-027 冻结权限分级与授权有效范围：[证据](evidence/AZ-027-permission-contract.md)；冻结四级授权、六类资源范围/期限/撤销/升级、持久意图状态与受保护审批surface；完成12动作例评。
- AZ-028 实现持久审批意图与范围校验：[证据](evidence/AZ-028-durable-core.md)；[证据](evidence/AZ-028-broker-surface.md)；[证据](evidence/AZ-028-execution-hooks.md)；[证据](evidence/AZ-028-existing-schema.md)；[证据](evidence/AZ-028-lifecycle-pairing.md)；[证据](evidence/AZ-028-binding-during-approval.md)；[证据](evidence/AZ-028-direct-context.md)；[证据](evidence/AZ-028-harness-coverage.md)；[证据](evidence/AZ-028-background-read-guard.md)；[证据](evidence/AZ-028-search-start.md)；[证据](evidence/AZ-028-cached-search.md)；[证据](evidence/AZ-028-lsp-diagnostics.md)；[证据](evidence/AZ-028-diagnostic-preview.md)；[证据](evidence/AZ-028-symbol-resources.md)；[证据](evidence/AZ-028-recursive-preflight.md)；[证据](evidence/AZ-028-expiry-card.md)；[证据](evidence/AZ-028-lsp-execution-gate.md)；[证据](evidence/AZ-028-protected-search-scope.md)；[证据](evidence/AZ-028-single-submit-card.md)；[证据](evidence/AZ-028-stale-handler.md)；[证据](evidence/AZ-028-preview-retry.md)；[证据](evidence/AZ-028-unavailable-dismiss.md)；[证据](evidence/AZ-028-closed-preview.md)；[证据](evidence/AZ-028-acceptance-review-2026-10-06.md)；原始持久审批验收按当前 host 集成与 SQLite 自动化通过：参数/任务/环境/期限绑定，重启作废旧授权；17项宿主执行集成与18项审批领域测试通过。。
- AZ-029 让秘密加密失败阻止保存：[证据](evidence/AZ-029-secret-save.md)；[证据](evidence/AZ-029-rollback-review-started.md)；[证据](evidence/AZ-029-rollback-review-result.md)；[证据](evidence/AZ-079-product-integration.md)；[证据](evidence/ADR-004-protected-downgrade.md)；加密失败关闭与原子保存已进入固定vendor依赖；跨格式回退按ADR-004移交受保护备份恢复发行门禁。
- AZ-030 接入系统凭据保护与迁移：[证据](evidence/AZ-030-windows-key-protection.md)；Windows DPAPI用户级主密钥保护、原子旧密钥迁移及新版配置严格加载已接入固定vendor；保护不可用与损坏恢复明确失败关闭。。
- AZ-031 贯通秘密引用与脱敏出口：[证据](evidence/R10-19-security-and-webview.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-032 修复 Webview 功能与隔离策略：[证据](evidence/AZ-032-webview-isolation-started.md)；[证据](evidence/AZ-032-webview-isolation-started.md)；[证据](evidence/AZ-032-webview-isolation-started.md)；[证据](evidence/AZ-032-parallel-review.md)；[证据](evidence/R10-19-security-and-webview.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-033 记录技能扩展来源与授权变更：[证据](evidence/AZ-033-skill-provenance-authorization.md)；[证据](evidence/AZ-033-skill-provenance-authorization.md)；技能安装与升级记录来源、版本、SHA-256及请求工具能力历史；升级新增工具前失败关闭并保留旧版；卸载记录撤权意图、删技能和Hub锁。
- AZ-034 实现受限执行的首个策略：[证据](evidence/AZ-034-capability-disclosure-started.md)；[证据](evidence/AZ-034-capability-disclosure-started.md)；[证据](evidence/AZ-034-capability-disclosure-started.md)；[证据](evidence/AZ-034-capability-disclosure-started.md)；[证据](evidence/AZ-034-capability-disclosure-started.md)；[证据](evidence/AZ-034-capability-disclosure-started.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-production-runner.md)；[证据](evidence/AZ-034-acceptance-review-2026-10-06.md)；交付并自检Windows AppContainer命令参考策略，接入code_run/shell；工作区ACL生命周期、无网络/越界拒绝、Job超时取消收割、失败关闭和能力披露有Windows自动化覆盖。内核353 passed、pool25 passed、Clippy/rustfmt通过。。
- AZ-035 提供统一任务详情读取接口：[证据](evidence/current-release-scope.md)；[证据](evidence/AZ-035-plan-readonly.md)；[证据](evidence/AZ-035-host-readonly-candidate.md)；[证据](evidence/AZ-035-host-readonly-integration.md)；[证据](evidence/AZ-035-two-workspace-integration.md)；[证据](evidence/R10-03-ux-debloat.md)；两个本地视图共用授权只读详情服务；领域5/plan16/host4/frontend11及tsc实际通过；GUI未验。
- AZ-036 CodeZ 打开当前任务环境：[证据](evidence/AZ-036-routing-design.md)；[证据](evidence/AZ-036-navigation-review.md)；[证据](evidence/AZ-036-execution.md)；[证据](evidence/R10-06-workz-codez-handoff.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-037 实现交接与人类编辑反馈：[证据](evidence/R10-07-human-edit-feedback.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-038 统一产物清单与版本：[证据](evidence/AZ-038-execution.md)；[证据](evidence/R10-03-ux-debloat.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-039 改造交付评审与验收动作：[证据](evidence/AZ-039-execution.md)；[证据](evidence/R10-08-merge-and-snapshot.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-040 简化首用输入与运行状态：[证据](evidence/AZ-040-input-design.md)；[证据](evidence/AZ-040-execution.md)；[证据](evidence/R10-12-workz-starter-and-bar.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-041 建立脏缓冲与外部改动冲突流程：[证据](evidence/AZ-041-next-scope.md)；[证据](evidence/AZ-041-next-scope-review.md)；[证据](evidence/AZ-041-next-scope-review.md)；[证据](evidence/AZ-041-root-checkpoint.md)；[证据](evidence/R10-05-dirty-buffer-guard.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-042 语言服务崩溃恢复：[证据](evidence/AZ-042-lsp-recovery-candidate.md)；[证据](evidence/AZ-042-closure-evidence.md)；恢复预算在成功重连后重置；回归测试覆盖多轮崩溃恢复；10/10 通过。
- AZ-043 调试会话生命周期回归：[证据](evidence/AZ043-next-scope.md)；[证据](evidence/AZ-043-root-checkpoint.md)；[证据](evidence/R10-22-e2e-golden-journey.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-044 终端生命周期与路径回归：[证据](evidence/AZ-044-terminal-checkpoint.md)；[证据](evidence/AZ-044-terminal-checkpoint.md)；[证据](evidence/R10-21-bundle-and-perf.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-046 建立首批真实扩展兼容档案：[证据](evidence/R10-23-diagnostics-and-extensions.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-047 整理日常工程交互一致性：[证据](evidence/AZ-047-keyboard-checkpoint.md)；[证据](evidence/R10-18-theme-and-i18n-closure.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-048 无仓库任务独立工作目录：[证据](evidence/AZ-048-directory-checkpoint.md)；[证据](evidence/R10-08-merge-and-snapshot.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-049 研究报告交付模板：[证据](evidence/R10-12-workz-starter-and-bar.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-050 文档产物编辑预览闭环：[证据](evidence/R10-09-editable-preview-canvas.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-053 浏览器上下文与抢占恢复：[证据](evidence/R10-04-kernel-and-browser-pin.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-056 核心生命周期 E2E 门禁：[证据](evidence/R10-22-e2e-golden-journey.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-057 Windows 故障与长时运行基准：[证据](evidence/R10-21-bundle-and-perf.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-059 优化首屏与重型资源加载：[证据](evidence/R10-21-bundle-and-perf.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-060 消除长任务资源增长：[证据](evidence/R10-21-bundle-and-perf.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-061 统一诊断关联和脱敏导出：[证据](evidence/R10-23-diagnostics-and-extensions.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-062 建立评测趋势与质量门禁：[证据](evidence/R10-22-e2e-golden-journey.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-063 版本化数据迁移与备份恢复：[证据](evidence/ADR-004-protected-downgrade.md)；[证据](evidence/R10-20-migration-and-backup.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-066 首次成功任务引导：[证据](evidence/R10-14-onboarding-and-beta-gate.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-067 同步支持文档与发布说明：[证据](release-decision-1.0.0.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-069 完成稳定版发布决策：[证据](release-decision-1.0.0.md)；Completed and verified in AgentZ 1.0.0 engineering baseline (ADR-006).。
- AZ-079 修复同轮重复编辑的完整Undo：[证据](evidence/AZ-079-repeat-undo-repro.md)；[证据](evidence/AZ-079-undo-fix-started.md)；[证据](evidence/AZ-079-undo-core.md)；[证据](evidence/AZ-079-host-candidate-started.md)；[证据](evidence/AZ-079-host-candidate-result.md)；[证据](evidence/AZ-079-reconstructed-candidate.md)；[证据](evidence/AZ-079-reconstructed-validation.md)；[证据](evidence/AZ-079-reconstructed-validation.md)；[证据](evidence/AZ-079-portable-bundle.md)；[证据](evidence/AZ-079-product-integration.md)；[证据](evidence/AZ-079-product-integration.md)；将审核后的v0.8.68基线与AZ-029/AZ-079补丁接入vendor固定源码依赖；产品核心4/4、宿主18/18、任务路由2/2通过，--locked复跑通过。。
- AZ-080 复查递归预检之后的路径替换：[证据](evidence/AZ-080-junction-revalidation.md)；[证据](evidence/AZ-080-junction-revalidation.md)；递归 file_search/file_list 在有界链接预检后、以及下游hook返回后、provider调用前再次验证搜索起始路径仍在授权根中。Windows junction 被注入到预检后和下游hook后两个时序点，均拒绝工具调用；approval_execution 18 passed、1 ignored。严谨记录严格Clippy既有4 lint阻断及allow-list检查通过。。
- AZ-081 让固定内核遍历遵守排除目录：[证据](evidence/AZ-081-excluded-search-directories.md)；固定内核与宿主共享目录排除规则，实际遍历在访问前剪枝，并将补丁应用至不可变依赖候选验证。。
- AZ-082 过滤 LSP 结构化返回中的资源路径：[证据](evidence/AZ-082-lsp-response-resource-scope.md)；LSP在渲染前将结构化资源URI限制到任务工作区，对越界/未知路径失败关闭并标记省略数。。
- AZ-083 修复 AppContainer 后代回收验收的不稳定性：[证据](evidence/AZ-083-descendant-handshake.md)；以ready/release握手及正向对照替换依赖控制台输入的timeout后代探针；重复验证与完整内核回归通过。。

preparing只表示候选实现；显式代码依赖可消费有独立复核及当前源码指纹的上游候选，不代表其已验收。正式review/done和Gate仍要求原验收依赖，不以实现复核替代桌面、模型或平台验收。

完成数只表示任务验收，不能作为产品成熟度或发布许可。桌面、真实模型和平台验收按各自Gate记录。
