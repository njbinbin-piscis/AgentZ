# Release-first 候选范围与实施排序复核

2026-10-09。只读计划/证据审查；不修改plan.json、生产源码，不运行构建或发布。目标是尽快完成一个明确版本的桌面候选交付闭环，而非把83项成熟化全部做完再交付。

## 当前已经存在的具体交付

`AZ-064-current-local-build.md`记录0.9.7 Windows x64未签名候选：

- 安装器 `target/release/candidate-0.9.7-20261008/AgentZ_0.9.7_x64-setup.exe`，98,648,395bytes，SHA256 `623ed5710ec0ea954c63976aa9c323564bb5ed06ad7098a679387c6d19e3ad11`。
- 实际前端tsc/Vite、extension-host build、Tauri NSIS build和该新产物offline preflight均exit0。Authenticode NotSigned；candidate_inputs_ready=true、release_ready=false。
- 该安装器冻结包含本轮AZ018本地单Agent候选之前的已集成范围。**不包含随后AZ023重试及AZ024核心账务改动**，不能把最新源码测试加到旧包功能承诺。

可以立即给用户提供这个具体本地工程候选和已知限制；这不需要等待AZ024全收费、团队、macOS/Linux或83项全部done。若需“包含AZ023”的下一候选，则先冻结最小补丁集合与snapshot，构建一次并生成新独立产物/manifest；不重复扩大范围或覆盖旧包。版本命名由主线程按既有release元数据管理，不能把不同源码同名包混为一份证据。

当前可承诺范围：Windows x64工程候选、本地普通单Agent的环境绑定/持久Run与计划/只读执行历史候选能力、已有审批/凭据保护/受限执行及Undo改进；远程、Webview、LSP为源码候选及自动回归范围，真实旅程待验。不承诺稳定版、跨平台、完整恢复、全Task费用、团队或真实provider成功率。

## 发布类型与实际阻挡

| 目标 | 当前状态/实际前置 | 不应强加的前置 |
|---|---|---|
| 本地工程候选可审阅交付 | 具体包已经存在；核对manifest/snapshot/功能范围，给链接及限制即可 | AZ024整体、全部团队能力、全部路线图Gate |
| 下一本地候选含AZ023 | 最新限定源码冻结、host编译/测试、前端资源匹配、一次NSIS构建与新产物hash/清单；AZ024修复测试若带入也须冻结 | AZ024 LLM adapter、所有费用wire、跨平台、真实模型基准 |
| 可分发Windows预览/Beta | 明确用户分发授权、可信资源来源/摘要、安装卸载/关键数据升级与恢复验证、适用签名/渠道策略、具体核心旅程验证。当前无此发布授权 | 未提供或不宣称支持的平台/团队功能 |
| 正式稳定版 | 原AZ069/Beta质量与正式Gate按真实证据；未执行GUI/模型/安装/回滚不能通过 | 不能用“候选已打包”替代该层验收 |

现有offline preflight只检查输入及字节hash，始终release_ready=false；不能把exit0解释为安全发行批准。candidate workflow仅手动artifact-only，受批准resource-integrity.json等供应链前置约束；本地bundle与CIworkflow能力是不同证据。签名、公证、可信更新、平台测试、真实模型配置预算未具备时记录缺口，不因此停止可逆的候选代码/本地包交付。

已发现的实际数据/权限正确性缺陷必须修好后纳入候选；用户允许后移GUI资源不等于允许已复现错误。AZ024 observer operation冲突P2已有独立修复候选和追加fixture，统一最新测试尚需主线程记证据；无宿主接线的core不能成为当前产品收费承诺。

## 83项与ready为空的含义

本次读取plan：83任务，done24、review5、preparing8、blocked1、pending45；共59未正式验收。通过Gate只有M0。这个计数是验收台账，不是当前安装器完成度或发版所需任务数量。

`upgrade-plan.mjs:166/193/201` ready只返回 `pending && !unmet(task)`，unmet同时要求任务依赖done与里程碑前置Gate passed。因此M1 GUI待验收使M2以后ready列表空；13个review/preparing也不在ready内。这不表示没有可行动工作。

已有ADR-005、后续code_dependencies/implementation_review规则（脚本candidateDependencies）已把**候选开工**与**正式验收**分开：上游preparing/review的限定契约只有独立复核、最新文件指纹、真实验证才能消费；doing/review/done和Gate依然使用正式依赖。next-cycle.md旧草案声称该规则尚未采用，已落后于当前脚本，不能据此反复讨论或重新设计同一制度。

建议最小跟踪改进：另外输出candidate-ready/候选依赖缺口（保留ready原意），并明确“本轮发布范围”独立清单。它只改善排序视图，不新增假done、不绕过Gate。主线程可按现脚本为满足限定契约的AZ019/AZ021/AZ035登记code_dependencies；指纹过期先刷新审查，不直接开启。

## 下一单一主实现与并行安排

优先主实现 **AZ-019本地普通任务重启对账**：消费AZ017/018已复核Run/Event接口，仅启动时读取权威活跃Run、确认本进程无法恢复执行后标Interrupted/解释可人工继续，禁止自动重跑模型或工具。先冻结“可确定失活”的本地单进程范围，旧库/未知环境失败关闭，测试受控重开。这个用户可感知闭环比继续AZ024完整费用adapter更直接提高候选可靠性。若本轮需先打包，则先完成发布冻结包，再进入AZ019，避免构建源持续变化。

| 并行包 | 独占范围建议 | 具体完成结果 |
|---|---|---|
| 主线程候选冻结/发行清单 | snapshot、candidate manifest、发布说明/构建配置，由主线程串行 | 选择现0.9.7交付或含AZ023新候选；一份明确源码→资源→安装器证据链；不发布 |
| 子代理AZ019新恢复模块 | 新runtime恢复模块+独立fixture/证据；chat/startup/Run共享接线主线程 | Interrupted对账与只读恢复提示，不自动resume；受控SQLite/重开测试 |
| 子代理AZ035限定Task DTO | 新只读DTO/service/tests；不同时改chat_turn/App | task/environment/currentRun/plan/当前事件引用聚合，预算/产物缺失明确unknown；为CodeZ/WorkZ统一展示铺路 |
| 子代理发行文档/候选校验 | 发布范围、已知问题、升级风险清单、静态artifact检查；不产品源码 | 修正文档旧草案，注明旧包不含AZ023/024；缺资源列待验，不造通过 |

同时最多一个Rust全量构建；构建前冻结生产源，代理可只读审查/独立文档与fixture设计。AZ019与AZ035原task依赖均是preparing上游，先经现code_dependencies记录和最新契约复核再实施。GUI/真实模型/SSH/跨平台验收单独排队，不挤占每个原子代码包的收尾。

后移本版：AZ024全部收费/时间硬上限/成员、AZ070–076团队与自动化、AZ058跨平台、AZ065自动更新、真实provider成本基准。它们保持正式台账状态，不能写done，但不是限定Windows工程候选的默认前置。AZ021取消闭环是下一可靠性优先项，可与AZ019先设计，避免同时编辑shared runtime。

## 最小交付闭环

1. 立刻冻结本轮候选范围：已有0.9.7包可交付审阅；若加入AZ023只再做一次新包，不让AZ024继续扩范围。
2. 记录源snapshot、资源及包hash、实际测试、未安装/未签名/未模型GUI/未恢复验收；发布说明与具体包同范围。
3. 展示本地具体包和已知限制，保留对外发布未授权。候选交付完成与路线图done分开汇报。
4. 转入AZ019单一主实现，AZ035/发行文档独占并行；每包收尾即产出可消费契约，不等全Gate才产生产品进展。

此建议无需新用户产品选择即可开展常规可逆工程准备；真正分发、收费默认策略和稳定版发布仍需对应明确授权/验收。
