# 2026-10-13 integrations / quality / release reassessment — AZ049–078

This is a fresh read-only code-and-evidence reassessment of 30 tasks, not a replay of the previous status table. No source, ledger or old evidence was edited; no Cargo, models, external APIs, installers, native GUI or release operations ran in this pass. `implementation_state` and `verified_state` describe code coverage, not acceptance. Every item retains concrete inspected files and outstanding verification. Parent owns the actual test queue and final 83-task ledger update.

Original product review F06/F08/F09/F11 remains useful risk context, but blanket “not implemented” findings were corrected where current code now has expiry/PKCE helpers, artifact versions, diagnostic export, trend scripts, workflow checkpoints, cleanup and feature settings. None of those primitives proves the entire product journey.

Important boundary: AgentZ Cargo.lock pins robotz-browser v0.1.13 commit cdff2c947f812648050a550b8e9d8a887bf01e01. The independently opened RobotZ working tree contains navigation-generation improvements, while the pinned cache lacks them. They cannot be credited to AgentZ until reviewed dependency adoption/wiring. Native UIA guards in RobotZ similarly do not establish AgentZ native Desktop tool registration.

ADR008 records a GUI-only waiver, not native GUI passing evidence. It does not waive external macOS/Linux environments, certificates, notarization or installer acceptance. Engineering harnesses, pure ownership/expiry/retry guards, documentation consistency and profiling instrumentation are local next actions even when final acceptance has an external gate. AZ077/078 also have local correctness work, not just missing hardware.

The latest reviewed workflows-installer receipt is unsigned, installed=false, release_ready=false and predates current source. Release YAML publishes non-draft outputs on v*; candidate workflow is artifacts-only. Offline signature evidence validation intentionally cannot authenticate native execution. No release readiness is inferred here.

## Per-task findings

### AZ-049 — 研究报告交付模板

Code: **not_started**; severity P2; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- Artifact Kind::Report exists, but no research question/source locator/fact versus inference template or validator was located. Delegate is a read-only investigator, not a report delivery schema.

Next executable slice: After artifact/delivery contracts stabilize, add report DTO/template and locator/acceptance validation locally; then review one fixed report.

Verification still missing: No generated report or locator/checklist acceptance receipt.

Inspected: `src-tauri/src/tools/delegate.rs`, `crates/agentz-task-state/src/artifacts.rs`, `src/workspaces/codez/TaskArtifactViewer.tsx`

### AZ-050 — 文档产物编辑预览闭环

Code: **partial**; severity P1; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- capture/list/read/delete and text version preview exist; no edit/restore-version command or durable edited-artifact continuation path. MarkdownPreview only accepts content; it is not an editing lifecycle.

Next executable slice: Add explicit human edit with new-version commit, restore-as-new-version and continuation attachment under fresh binding; avoid overwriting history.

Verification still missing: Saved-preview equality, restore history and continuation fixtures absent; GUI unrun/ADR008 waived.

Inspected: `src-tauri/src/commands/task_artifacts.rs`, `src/workspaces/codez/TaskArtifactViewer.tsx`, `src/workspaces/codez/MarkdownPreview.tsx`, `src/services/tauri/taskArtifacts.ts`

### AZ-051 — 连接器授权生命周期

Code: **partial**; severity P1; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- PKCE verifier/challenge, state matcher, HTTPS authorization URL and expiry helpers are now implemented, superseding old no-helper findings. They are not a registered start/callback/refresh flow.
- credentials.json remains plaintext; missing or malformed expires_at is treated as non-expired. get_credentials masks only manifest-declared secret fields, so undeclared OAuth token fields can reach renderer. Disable clears tokens locally but no per-call live MCP revocation gate was found.

Next executable slice: First finish local secret-reference storage/masking, validated callback state and bounded refresh/revocation gates using fake provider fixtures; reserve live provider acceptance as a separate account gate.

Verification still missing: Controlled provider/account live authorization and revoke not run; local helpers alone do not prove lifecycle.

Inspected: `src-tauri/src/commands/connectors.rs`, `src-tauri/src/tools/api_connector.rs`, `src/services/tauri/connectors.ts`, `src-tauri/src/commands/chat_turn.rs`

### AZ-052 — 外部发送与发布交付流程

Code: **partial**; severity P1; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- Outbound API tool/gateway exist and external_action domain exists, but inspected outbound code does not consume its immutable permit/idempotency/reconciliation contract. This is an existing unguarded surface, not absence of all send code.

Next executable slice: Connect POST/PUT/DELETE and gateway.send to prepare/frozen approval/start/completion/unknown reconciliation; create sandbox-local tests before live external account receipt.

Verification still missing: No frozen target/recipient UI, unknown-result reconciliation or deduped-send acceptance.

Inspected: `src-tauri/src/tools/api_connector.rs`, `src-tauri/src/commands/connectors.rs`, `src-tauri/src/commands/gateway.rs`, `crates/agentz-task-state/src/external_action.rs`

### AZ-053 — 浏览器上下文与抢占恢复

Code: **partial**; severity P1; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- AgentZ registers pinned robotz-browser v0.1.13 cdff2c947f812648050a550b8e9d8a887bf01e01. Its cached snapshot has no navigation generation and invalidate_tab spawns asynchronous removal. Newer generation-aware RobotZ working tree is not AgentZ pinned implementation.
- AgentZ BrowserActivity is global recent-use/chat-active close UX, not task/page ownership or user-takeover revocation. UI navigate shares manager without task token.

Next executable slice: Port/pin reviewed generation-aware ref handling and wire every GUI/tool navigation plus user takeover to same captured task/page generation; fixture stale-ref and delayed action refusal.

Verification still missing: Actual pinned-browser user takeover/stale-reference journey unrun; no task-bound lease/revoke evidence.

Inspected: `Cargo.lock`, `Cargo.toml`, `src-tauri/src/commands/chat_turn.rs`, `src-tauri/src/browser/activity.rs`, `src-tauri/src/browser/mod.rs`, `src-tauri/src/commands/browser.rs`, `C:/Users/zzz/.cargo/git/checkouts/robotz-869dfd69374135c7/cdff2c9/crates/robotz-browser/src/snapshot.rs`, `C:/Projects/RobotZ/crates/robotz-browser/src/snapshot.rs`

### AZ-054 — Windows 桌面操作最小闭环

Code: **not_started**; severity P1; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- AgentZ desktop registry explicitly installs robotz_browser, not robotz-automation UIA/DesktopAutomation. app_control manages AgentZ settings and does not establish native application focus control.
- RobotZ separate working tree has UIA foreground/non-password guards; that does not show AgentZ runtime exposure or a supported application contract.

Next executable slice: Define one supported Windows application/action set, add host focus identity and user takeover gate plus sensitive-region policy; test pure guards first and preserve native unverified label.

Verification still missing: No AgentZ native UIA integration or controlled app acceptance. GUI waiver does not fabricate OS operation evidence.

Inspected: `src-tauri/src/commands/chat_turn.rs`, `src-tauri/Cargo.toml`, `src-tauri/src/tools/app_control.rs`, `C:/Projects/RobotZ/crates/robotz-automation/src/uia.rs`, `C:/Projects/RobotZ/crates/robotz-automation/src/desktop_automation.rs`

### AZ-055 — 建立跨任务桌面资源租约

Code: **not_started**; severity P1; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- Run/environment owner leases exist, but no cross-task foreground/browser/shared-file resource lease was found. RobotZ tab lock is one global tab pointer, without task/attempt owner.

Next executable slice: After browser/native action binding, implement distinct task-attempt resource capability with queue/busy/revoke/drop and cancellation fixtures; do not repurpose process owner lease.

Verification still missing: No two-task front-resource contention/revocation fixture.

Inspected: `src-tauri/src/runtime/owner_lease.rs`, `src-tauri/src/state.rs`, `src-tauri/src/browser/activity.rs`, `C:/Users/zzz/.cargo/git/checkouts/robotz-869dfd69374135c7/cdff2c9/crates/robotz-browser/src/snapshot.rs`

### AZ-056 — 核心生命周期 E2E 门禁

Code: **partial**; severity P1; proposal `fix_then_done` (historical ledger status `blocked` is not changed).

- CI runs lint/typecheck/frontend tests/extension smoke/workspace Rust tests; candidate workflow builds artifacts. No single create→dual-task→continue→restart→review→archive/restore orchestration gate found.

Next executable slice: Implement local isolated lifecycle harness and log/failure gate first; then add it to candidate workflow. Keep native UI journey explicitly waived under ADR008, not passed.

Verification still missing: CI source is not a CI execution receipt; no candidate-specific integrated lifecycle log.

Inspected: `.github/workflows/ci.yml`, `.github/workflows/candidate.yml`, `scripts/upgrade-candidate-workflow.verify.mjs`, `src-tauri/src/commands/agent_task.rs`, `docs/upgrade/evidence/ADR-008-gui-blanket-waiver.md`

### AZ-057 — Windows 故障与长时运行基准

Code: **partial**; severity P1; proposal `fix_then_done` (historical ledger status `blocked` is not changed).

- perfCounters and bounded-process regressions are measurement primitives, not cold-start/two-hour/long-chat benchmark. No two-hour hardware/version/resource result was located.

Next executable slice: Freeze budgets and implement owned Windows sampler/fault harness locally; record hardware/build/attribution and actual soak run separately from waived visual checks.

Verification still missing: No baseline/repeated two-hour memory/handle/child residual measurements.

Inspected: `src/utils/perfCounters.ts`, `src/utils/perfCounters.test.ts`, `src-tauri/src/runtime/durable_stream.rs`, `src-tauri/src/commands/ide.rs`, `docs/upgrade/evidence/AZ-005-support-matrix.md`

### AZ-058 — macOS/Linux 核心旅程验证

Code: **partial**; severity P1; proposal `blocked_external` (historical ledger status `blocked` is not changed).

- Cross-platform CI/release YAML and macOS/Linux credential adapters exist; no installation-and-core-journey native receipts in reviewed scope. Build definitions are not runtime proof.

Next executable slice: Prepare reusable platform fixture checklist/scripts locally; obtain macOS/Linux actual sessions and run install/start/task/terminate/recover, recording differences before support upgrade.

Verification still missing: Actual macOS/Linux machines/sessions absent; ADR008 does not waive external platforms.

Inspected: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `docs/upgrade/evidence/AZ-005-support-matrix.md`, `docs/upgrade/evidence/ADR-008-gui-blanket-waiver.md`

### AZ-059 — 优化首屏与重型资源加载

Code: **partial**; severity P2; proposal `fix_then_done` (historical ledger status `blocked` is not changed).

- Vite manualChunks separates vendor/Monaco/Markdown; mermaidSafe loads Mermaid dynamically. CodeZ still statically imports Monaco and App entry is not comprehensively lazy. Chunk naming alone is not measured loading improvement.

Next executable slice: Create same-build cold-start/load profile baseline, isolate real heavyweight entry imports and compare offline assets and latency; split local profiling from GUI waiver.

Verification still missing: No matching before/after startup benchmark or frozen loading budget.

Inspected: `vite.config.ts`, `src/App.tsx`, `src/workspaces/codez/index.tsx`, `src/workspaces/codez/MarkdownPreview.tsx`, `src/workspaces/codez/mermaidSafe.ts`, `src/workspaces/codez/CodeMap/layout.ts`

### AZ-060 — 消除长任务资源增长

Code: **partial**; severity P1; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- CodeZ model URI cleanup, extension disposers and DAP session revocation now exist; this supersedes a blanket no-cleanup statement. In-process subagent inflight lifecycle also has outcome cleanup.
- No two-hour resource census demonstrates listener/model/PTY/index cleanup after task/panel close.

Next executable slice: Inventory owned listeners/models/processes/index jobs with allocation/release counters; target measured leaks and run close/restart soak against frozen budget.

Verification still missing: No measured long-run memory/resource slope and post-close census.

Inspected: `src/workspaces/codez/index.tsx`, `src/extensions/debug/dapClient.ts`, `src/extensions/extensionService.ts`, `src-tauri/src/runtime/koi.rs`, `src/utils/perfCounters.ts`

### AZ-061 — 统一诊断关联和脱敏导出

Code: **partial**; severity P1; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- Turn tracing includes app_version/task/environment fields; diagnostic export concatenates log files with known settings secret replacement.
- Export reads only Settings values, skips secrets shorter than eight chars and does not include connector credentials; no user range preview or complete task/run/tool/environment diagnostic manifest.

Next executable slice: Unify trace IDs and bounded export manifest; include all secret providers without exposing values, redact before preview/export, test short/connector/newly rotated secrets and task-failure locator.

Verification still missing: No all-source secret fixture or user range preview; current log-only export not complete support bundle.

Inspected: `src-tauri/src/diagnostics.rs`, `src-tauri/src/commands/diagnostics.rs`, `src-tauri/src/commands/chat_turn.rs`, `src-tauri/src/commands/connectors.rs`, `src-tauri/src/tools/settings_catalog.rs`

### AZ-062 — 建立评测趋势与质量门禁

Code: **partial**; severity P2; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- Trend script groups by model/endpoint and refuses spec/fixture mismatch; fixed-baseline reporter checks missing/contradictory samples. Existing AZ004 report records 33/36 success for kernel synthetic tools, not desktop/OS outcomes.
- Trend lacks intervention/cost/host-fault fields and CI quality threshold enforcement; sample count consistency is not fully checked by trend validateSummary.

Next executable slice: Extend compatible summary schema with known/unknown cost/intervention/fault attribution, assert count invariants, add regression threshold gate; retain synthetic versus actual desktop scopes.

Verification still missing: No multi-run host/product trend or gate execution; real-model additional samples need explicit provider budget.

Inspected: `scripts/upgrade-eval-trend.mjs`, `scripts/upgrade-eval-trend.verify.mjs`, `scripts/upgrade-model-baseline-report.mjs`, `scripts/upgrade-model-baseline.mjs`, `docs/upgrade/evidence/AZ-004-live-summary-v1.json`, `.github/workflows/ci.yml`

### AZ-063 — 版本化数据迁移与备份恢复

Code: **partial**; severity P1; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- Task-state schema version checks and kernel additive migrations exist, plus workspace hot-exit snapshot and secret migration code. They do not form a versioned protected whole-product backup/restore contract. ADR004 explicitly requires matched config/key backup before release.

Next executable slice: Implement inventory/versioned protected backup transaction covering task/session/environment/settings/artifact identities and compatible key references; failure retains original and recovery validates binding.

Verification still missing: No whole-product migration rollback or protected cross-version restore receipts.

Inspected: `crates/agentz-task-state/src/lib.rs`, `vendor/piscis-engine/piscis-kernel/src/store/db.rs`, `vendor/piscis-engine/piscis-kernel/src/security/secrets.rs`, `src-tauri/src/commands/workspace.rs`, `docs/upgrade/evidence/ADR-004-protected-downgrade.md`

### AZ-064 — 签名、公证与安装器验收

Code: **partial**; severity P1; proposal `blocked_external` (historical ledger status `blocked` is not changed).

- Offline signing evidence schema/validator, candidate preflight and actual local NSIS candidate receipts exist. Reviewed workflows installer records NotSigned, installed=false, release_ready=false; older known risk references do not automatically describe current source.
- Validator explicitly checks supplied-record consistency only and cannot authenticate native signing or installation. Release YAML still publishes non-draft outputs on v* independently of quality receipts.

Next executable slice: Finish local release readiness gate and installer fixture protocol first; obtain certificate/trust identity and clean test machines for actual signature/install/start/uninstall receipts. No publish implied.

Verification still missing: No certificate identity/native signature/notarization/install/uninstall acceptance; current source is newer than inspected installer.

Inspected: `scripts/upgrade-signing-evidence.mjs`, `docs/upgrade/contracts/signing-evidence.schema.json`, `scripts/upgrade-release-preflight.mjs`, `.github/workflows/candidate.yml`, `.github/workflows/release.yml`, `docs/upgrade/evidence/AZ-064-workflows-installer.json`

### AZ-065 — 可信更新与失败恢复

Code: **not_started**; severity P1; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- No updater plugin/config or in-app update flow found; RELEASE documents manual unsigned upgrades. ADR004 is a downgrade policy, not implementation.

Next executable slice: After protected backup and trusted signing, add authenticated channel/manifest/update checks plus interrupted download/install recovery and explicit schema compatibility refusal; test locally before service deployment.

Verification still missing: No bad-signature/recovery/compatible-data rollback fixture.

Inspected: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `RELEASE.md`, `docs/upgrade/evidence/ADR-004-protected-downgrade.md`

### AZ-066 — 首次成功任务引导

Code: **partial**; severity P2; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- SettingsPanel exposes provider/API-key configuration and DevEnv diagnostics exist. No dedicated clean-machine first-success wizard/sample journey was located.

Next executable slice: Build offline readiness checklist linking missing model credential/tool/environment to actionable settings/diagnostic actions and a fixed first-task example; keep actual clean-install/model run separate.

Verification still missing: No clean-machine first successful task receipt; no fake missing-key/tool onboarding fixture.

Inspected: `src/workspaces/codez/SettingsPanel.tsx`, `src/hooks/useAppSettings.ts`, `src-tauri/src/commands/devenv.rs`, `src/App.tsx`

### AZ-067 — 同步支持文档与发布说明

Code: **partial**; severity P2; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- Support matrix and candidate notes distinguish unverified scope; version injected from package and preflight checks identity. RELEASE still says current model v0.3.0 while package is0.9.7.
- Known-issues status index updated2026-10-13 still describes plan continuity as in-memory despite durable plan integration; its scope must be narrowed to remaining unsupported paths rather than blanket assertion.

Next executable slice: Reconcile docs against current exact path/feature coverage, synchronize version checks in CI and write implemented/verified/waived/planned sections without promoting build support.

Verification still missing: No complete docs version/claim consistency gate; old issue refs partly deliberately unverified.

Inspected: `README.md`, `RELEASE.md`, `docs/known-issues.md`, `docs/upgrade/evidence/AZ-005-support-matrix.md`, `docs/upgrade/release-notes-0.9.7.md`, `package.json`, `scripts/upgrade-release-preflight.mjs`, `src-tauri/src/tools/durable_plan_todo.rs`

### AZ-068 — 运行 Beta 与支持分流

Code: **not_started**; severity P2; proposal `blocked_external` (historical ledger status `blocked` is not changed).

- No actual participant roster/feedback ledger/cohort support window was located. Existing review templates are engineering records, not a Beta program.

Next executable slice: Prepare severity/attribution/rollback intake templates locally; named owner must select cohort and authorize distribution before actual external feedback.

Verification still missing: Actual Beta participants and feedback unavailable; cannot fabricate stakeholder or send outreach.

Inspected: `docs/upgrade/review-template.md`, `docs/upgrade/release-checklist-0.9.7.md`, `docs/upgrade/evidence/AZ-005-support-matrix.md`

### AZ-069 — 完成稳定版发布决策

Code: **not_started**; severity P1; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- Milestone ledger/candidate checklist exist, but no stable-release decision with complete gates, supported scope and accepted risk identities. Candidate hashes do not authorize release.

Next executable slice: Assemble explicit gate matrix after engineering/backup/signing/Beta dependencies; unresolvedP0/security/recovery gates cannot be accepted by missing evidence.

Verification still missing: Stable product/installation/recovery/signing/Beta receipts missing; final user release decision remains external.

Inspected: `docs/upgrade/plan.json`, `docs/upgrade/release-checklist-0.9.7.md`, `docs/upgrade/evidence/AZ-005-support-matrix.md`, `.github/workflows/release.yml`

### AZ-070 — 成员任务环境与集成契约

Code: **partial**; severity P1; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- Teams carry member IDs/org_spec/project pool and runtime requires workspace for pool turns; it no longer silently falls back to global workspace for such turns.
- Workflow dispatch assigns run.project_dir to nodes; inspected contract lacks authoritative per-member environment/output/integration-owner and conflict-resolution identity.

Next executable slice: Add member environment and artifact/output provenance contracts under real task binding, explicit integration owner and conflict policy; build concurrent same-file/environment fixtures before team defaults.

Verification still missing: No member-specific workspace isolation/integration artifact acceptance.

Inspected: `src-tauri/src/commands/teams.rs`, `src-tauri/src/runtime/koi.rs`, `src-tauri/src/runtime/workflow.rs`, `src-tauri/src/tools/call_fish.rs`

### AZ-071 — 团队任务预算与失败传播

Code: **partial**; severity P1; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- Subagent runtime has per-turn cancellation handles; teams configure task_timeout_secs; workflow has active-turn cancellation/retry/error outcome paths. These are implemented primitives.
- No shared authoritative team token/time/cost ledger across all members found in inspected team/runtime; existing tool budget hooks do not prove team aggregation/fanout acknowledgement.

Next executable slice: Connect one team-owned durable budget and member reservations, cancellation fanout with settlement acknowledgement and partial-failure attribution; avoid copying limits per member.

Verification still missing: No team total-budget or all-member cancel settlement fixture/actual team cost receipt.

Inspected: `src-tauri/src/runtime/koi.rs`, `src-tauri/src/commands/teams.rs`, `src-tauri/src/runtime/workflow.rs`, `src-tauri/src/runtime/task_budget.rs`, `src-tauri/src/tools/call_fish.rs`

### AZ-072 — 持久化自动化触发与去重

Code: **partial**; severity P1; proposal `fix_then_done` (historical ledger status `pending` is not changed).

- Kernel CronScheduler installs in-memory tokio jobs; desktop pool heartbeat uses background loops. No durable trigger occurrence identity/deduped dispatch/sleep restart compensation registry found.

Next executable slice: Implement trigger/occurrence/claim ledger with explicit missed-window policy, disable generation and no automatic repeat of unknown external actions; deterministic clock/restart fixtures first.

Verification still missing: No restart/sleep/dangerous trigger dedupe acceptance; heartbeat settings are not durable automation.

Inspected: `vendor/piscis-engine/piscis-kernel/src/scheduler/cron.rs`, `src-tauri/src/runtime/heartbeat.rs`, `src-tauri/src/tools/settings_catalog.rs`, `crates/agentz-task-state/src/external_action.rs`

### AZ-073 — 建立通知收件箱与静默策略

Code: **partial**; severity P2; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- Pool heartbeat avoids ack-only response output and durable Run event notice triggers replay; neither is a persistent user notification inbox with read state/task link/deduplication.

Next executable slice: After durable trigger/run identity, build persisted meaningful-change inbox and quiet policy, unread/seen/event dedupe fixtures and task navigation.

Verification still missing: No inbox/read-state/notification dedupe product acceptance.

Inspected: `src-tauri/src/runtime/heartbeat.rs`, `src/services/tauri/runEventNotice.ts`, `src/workspaces/codez/RunHistoryPanel.tsx`

### AZ-074 — 工作流检查点与人工续跑

Code: **partial**; severity P1; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- Workflow JSON persists cursor/blackboard/history and waiting_human state; in-flight cancellation and resume primitives exist. Thus old no-workflow-implementation finding is too broad.
- Separate JSON workflow lifecycle is not unified durable Run/Attempt/external-action receipts. Crash after side effect before save_run can repeat node; retry can reissue an unknown-effect action.

Next executable slice: Unify node attempt/checkpoint/approval identities and commit action receipts before cursor advance; explicit unknown-result reconciliation, restart no-repeat and exact-human-node fixtures.

Verification still missing: No dangerous-node crash-window dedupe or cross-restart approval-binding acceptance.

Inspected: `src-tauri/src/runtime/workflow.rs`, `src-tauri/src/commands/workflow.rs`, `src/services/tauri/workflow.ts`, `src/services/tauri/workflow.test.ts`, `crates/agentz-task-state/src/external_action.rs`

### AZ-075 — 验证多 Agent 相对收益

Code: **not_started**; severity P2; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- Single-model baseline exists, but no single-agent/read-only-parallel/implementation-team matched-condition experiment located. Generic team capabilities are not benefit evidence.

Next executable slice: Write fixed task/cohort/config/hash/cost/intervention comparison protocol locally; execute authorized actual models after team scope/budget constraints are ready.

Verification still missing: No relative-benefit/cost/latency/quality sample set; no model calls in this reassessment.

Inspected: `scripts/upgrade-model-baseline.mjs`, `scripts/upgrade-model-baseline-report.mjs`, `docs/upgrade/evidence/AZ-004-live-summary-v1.json`, `src-tauri/src/tools/delegate.rs`, `src-tauri/src/runtime/koi.rs`

### AZ-076 — 发布自动化与团队受控能力

Code: **partial**; severity P2; proposal `deferred_with_reason` (historical ledger status `deferred` is not changed).

- heartbeat_enabled and per-tool/agent enablement settings exist; blanket no-feature-switch statement is inaccurate. No evidence-linked automation/team release gate/rollback switch contract located.

Next executable slice: Define per-capability supported scenarios/defaults and authoritative disable/revoke switch after prerequisites; UI limits must reflect unverified platforms, and defaults require actual benefit/safety receipts.

Verification still missing: No controlled-capability release decision or disable-in-flight automation/team fixture.

Inspected: `src-tauri/src/tools/settings_catalog.rs`, `src-tauri/src/runtime/heartbeat.rs`, `src-tauri/src/commands/chat_turn.rs`, `docs/upgrade/evidence/AZ-005-support-matrix.md`

### AZ-077 — macOS 系统凭据保护适配

Code: **implemented_unverified**; severity P1; proposal `blocked_external` (historical ledger status `blocked` is not changed).

- Native SecItem adapter is wired under macOS with local non-sync generic-password master key and fail-closed OSStatus handling. Native tests are ignored; no actual macOS receipt.
- Current perform rejects every nonzero status, including missing item on delete; accessibility policy is not explicit. These local design/code tasks must not be blocked wholesale by missing Mac hardware.

Next executable slice: Locally specify missing/delete idempotency and explicit accessible/keychain/signing-identity recovery policy, build mock status fixtures; reserve actual read/write/lock/revoke/corruption/migration for real signed macOS session.

Verification still missing: Real macOS Keychain/signing identity unavailable; cross-target typecheck is not native execution.

Inspected: `vendor/piscis-engine/piscis-kernel/src/security/macos_keychain.rs`, `vendor/piscis-engine/piscis-kernel/src/security/secrets.rs`, `docs/upgrade/evidence/AZ-077-keychain-adapter.md`, `docs/upgrade/evidence/ADR-008-gui-blanket-waiver.md`

### AZ-078 — Linux 系统凭据保护适配

Code: **implemented_unverified**; severity P1; proposal `blocked_external` (historical ledger status `blocked` is not changed).

- Secret Service adapter uses DH encrypted sessions, refuses locked default collection/prompt fallback, cleans failed creation and is wired under Linux. Actual Linux ignored tests are unrun.
- Blocking service calls have no inspected deadline; secrets AlreadyExists path recursively calls Self::new without explicit bound. These are actionable local work before Linux native acceptance.

Next executable slice: Add bounded service boundary and bounded key-create retry with fault fixtures; then real Linux session tests for unavailable bus/locked/prompt/corrupt/revoke/migration and cleanup.

Verification still missing: Real Linux desktop Secret Service provider/session absent; cross-target compile does not prove runtime or cross-machine backup portability.

Inspected: `vendor/piscis-engine/piscis-kernel/src/security/linux_keyring.rs`, `vendor/piscis-engine/piscis-kernel/src/security/secrets.rs`, `vendor/secret-service/src/util.rs`, `docs/upgrade/evidence/AZ-078-secret-service-adapter.md`

