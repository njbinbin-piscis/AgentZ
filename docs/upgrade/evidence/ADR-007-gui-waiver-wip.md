# ADR-007：原生 GUI 验收豁免与 WIP 计数

2026-10-10。用户在会话中两次明确选择：

1. 接受 AZ-009、AZ-013 缺少原生 GUI 验收的风险，并据此签署 M1 Gate。
2. 将 `preparing` 计入 WIP 上限；先把 `review` 降到上限 2 以内。

本 ADR 只记录这两项决定。它不把任何未执行的原生验收记为通过。

**执行状态：决定已记录，台账尚未写入。** 2026-10-10 尝试更新台账时，`upgrade-plan.mjs` 的校验因 AZ-036 的实现复核指纹与 `src/workspaces/codez/index.tsx` 不一致而拒绝所有写入。该文件有未提交改动（相对 HEAD 为 136 行新增、40 行删除），属于 AZ-036 的进行中工作，不由本 ADR 处理。在 AZ-036 的复核被重新记录或正式撤回之前，下列台账变更不会生效，M1 Gate 仍为 pending。

## 一、GUI 豁免的范围

适用：AZ-009（WorkZ 按任务恢复环境）、AZ-013（完整变更评审界面）。

豁免的验证项，均未执行，也不会被记为通过：

- 真实 A/B 交叉续聊、应用重启后的目录与文件变更核对
- 评审文件列表与目录的原生核对
- 窗口内的视觉与交互确认

已经有的证据：前端类型检查、前端单测（AZ-009 44 项，AZ-013 含组件测试）、宿主 `cargo check`，以及对应的后端环境与 ChangeSet 测试。这些只支持服务级与组件级结论。

处理方式：两项按台账规则改为 done，`reviewer` 写为“用户豁免（ADR-007）”，`review_notes` 写明豁免与未验收内容。`actual_days` 按预计值记录，未经测量，不作为实际工时。

## 二、M1 Gate 的含义

M1 的退出条件原文是“任务不串环境，实际改动完整进入评审；归档可恢复”，并要求 A/B 切换、重启和未提交评审案例通过。这些原生案例未验收。签署 M1 Gate 的含义是：

- 接受风险，允许 M2 及后续候选在此前提下开工；
- 不等于产品桌面旅程通过；
- 若后续原生验收发现环境串用、未提交改动漏评审或归档不可恢复，M1 重新开启，相关任务回到 review，并新建缺陷任务。

## 三、WIP 计数

`preparing`、`doing`、`review` 合计计入 WIP。团队默认上限沿用 README 的数值，即合计上限 3，其中 review 单独上限 2。

决定时的实际计数超过上限：`preparing` 15 项，`review` 5 项，`doing` 0 项。执行第四节的处理后，`review` 为 1 项（AZ-032），AZ-009 与 AZ-013 转 done，AZ-077 与 AZ-078 转 blocked。合计仍远超上限，因此在降到上限以内之前，不新开 `preparing` 或 `doing`，只允许完成现有候选的复核、验收或阻塞登记。

## 四、降低 review 的处理

AZ-077（macOS 凭据）与 AZ-078（Linux 凭据）的下一步都需要真实 macOS/Linux 测试账户和测试会话。它们没有可执行的本地动作，因此从 review 转为 blocked，解除条件写明为取得对应平台的测试会话。这不是豁免：两项凭据保护在对应平台上仍未验收。

AZ-032（Webview 隔离）不在豁免之内。它仍为 review，等待真实 Tauri Webview 验收；其安全相关验收不因本 ADR 改变。

## 五、不改变的事实

- AZ-004 仍为 done；它是 synthetic 基线，不是 MB 真实任务基线。MB Gate 不因本 ADR 签署。
- M3、M2 及以后的 Gate 不变。
- 未发布、未签名、未对外分发。

## 五、AZ-036 草稿策略复审（2026-10-10）

- 已实施：Close All / Close Others 排除 agentz-recovery: 草稿（草稿保持打开、不被关闭、不阻断这两个操作）。独立复审（子代理，非 Codex）确认 Cancel 不丢数据、保存中标签不被关闭、Close 草稿需确认。
- 已回退：项目切换 / 关闭文件夹时“草稿不阻断”的导航规则。复审证实：workspace_save 为单个全局文件，sanitize_snapshot 只保留 open_paths 中的脏缓冲，项目切换会清空标签页并在下次保存时覆盖草稿，导致草稿静默丢失。回退后项目切换仍被草稿阻断（179 个测试通过，typecheck 通过）。
- 结论：AZ-036 复审仍为 BLOCK，ledger 未写入。需用户决定项目切换时草稿的处理方式后再重新复审并记录。

- Round 6 独立复审 PASS（子代理 23db6489，非 Codex）；清单 docs/upgrade/evidence/AZ-036-implementation-manifest-r2.json（22 个文件）。vitest 184/184，typecheck 通过。
- ledger 写入未完成：record-implementation-review AZ-036 被校验拦截，原因是 AZ-017（App.tsx、en.ts、zh.ts）、AZ-041（codez/index.tsx）、AZ-036 的候选复审指纹已与磁盘文件不一致。其中项目切换文案改动（App.tsx、en.ts、zh.ts 的 saveBeforeProject* 行）是本轮引入；zh.ts 原文未保留，无法精确还原。
- 待用户决定：A 保留新文案并刷新 AZ-017 / AZ-041 复审；B 提供 zh.ts 原始文案以精确还原；C 暂不写 ledger，保持 AZ-036 preparing。

### 更新（2026-10-10 23:55，本地时间）
- 上文“ledger 写入未完成”的原因已变化：另一会话于 10:29Z 为 AZ-036 与 AZ-041 重新记录了 current_source_review，其候选指纹与磁盘一致。
- 当前 validate 的失败来自 crates/agentz-task-state/src/lib.rs（AZ-017、AZ-018、AZ-019、AZ-020、AZ-025、AZ-035、AZ-038 的复审指纹失效）。该文件由另一会话于 23:31 修改，并非本次任务改动。
- 因该文件仍在变动，本次不重新复审 AZ-017，也不写入 ledger，以免与进行中的工作冲突。AZ-036 保持 preparing。
- AZ-036 记录的 reviewer 为 Codex，与 owner Codex 相同；独立性需由用户确认后再依赖该记录。
