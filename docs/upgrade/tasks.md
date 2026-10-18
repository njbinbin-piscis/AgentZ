# AgentZ 原子任务执行台账

> 由 scripts/upgrade-plan.mjs render 从 plan.json 生成；状态更新请使用 CLI 或修改 JSON 后 validate/render。

共 83 项任务；理想工作量 123.5 有效工作日；不等于日历工期。

## M0 · 产品契约与可复现基线

Gate：passed；前置：无。

退出条件：产品旅程、身份契约、风险复现、支持策略和当前执行容量有证据；真实任务效果基线由MB单独验收。

### AZ-001 · 固定首批用户与三条核心旅程

- 状态：done；负责人：Codex；角色：产品；优先级：P1。
- 仓库：AgentZ；验收依赖：无；候选代码依赖：沿用验收依赖；预计：1d；实际：0.04d。
- 范围：确定开发者优先的定位；选定 bug 修复、代码研究、非代码报告旅程。
- 交付物：固定首批用户与三条核心旅程的版本化交付及验收记录。
- 验收：每条旅程写出输入、输出、授权和成功条件；明确本轮不承诺的能力。
- 验证：版本化产品契约与旅程示例。
- 关联审查：F10；风险：基线与团队容量未知；先确认口径再承诺日期。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：交付三条旅程与首期验收契约。
- 当前复评：document_complete；结论：preserve historical ledger status; qualify current verification separately；下一步：版本化契约与当前发布范围对齐。
- 复评剩余：版本化契约与当前发布范围对齐；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-001-contract.md。
- 完成摘要：冻结首批用户、三条旅程和首期范围；复盘：不把产品契约当作桌面实测；后续M4/M8必须用户旅程验收。

验收人：Codex自检（非独立用户验收）。

### AZ-002 · 固定任务与环境身份契约

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-001；候选代码依赖：沿用验收依赖；预计：1d；实际：0.04d。
- 范围：定义 Project/Task/Environment/Run/Artifact 标识及权威所在。
- 交付物：固定任务与环境身份契约的版本化交付及验收记录。
- 验收：新目标、续聊、视图切换和执行环境的语义无歧义；数据版本与兼容策略明确。
- 验证：ADR 与命令/事件样例审阅记录。
- 关联审查：F01, F09；风险：基线与团队容量未知；先确认口径再承诺日期。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：确认入口代码与基线，按scope交付并执行verification。
- 当前复评：document_complete；结论：preserve historical ledger status; qualify current verification separately；下一步：审阅实际迁移与重开边界。
- 复评剩余：审阅实际迁移与重开边界；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-002-identity-adr.md。
- 完成摘要：确定稳定Task和Environment身份与后端权威；复盘：不修改内核会话schema；旧任务绑定不能猜测。

验收人：Codex契约自检。

### AZ-003 · 冻结审查复现与回归夹具

- 状态：done；负责人：Codex；角色：质量；优先级：P1。
- 仓库：AgentZ；验收依赖：无；候选代码依赖：沿用验收依赖；预计：1d；实际：0.05d。
- 范围：用固定提交记录 A/B 环境切换、未提交评审、journal 根目录等复现。
- 交付物：冻结审查复现与回归夹具的版本化交付及验收记录。
- 验收：记录确认或未复现；每个案例包含版本、输入、预期、实际和日志。
- 验证：可运行临时 Git 仓库夹具与桌面复现记录。
- 关联审查：F01, F02, F03, F05；风险：基线与团队容量未知；先确认口径再承诺日期。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：运行临时仓库用例并记录源码路径证据。
- 当前复评：historical_baseline；结论：preserve historical ledger status; qualify current verification separately；下一步：按当前源码重新运行确认缺陷回归集。
- 复评剩余：按当前源码重新运行确认缺陷回归集；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-003-baseline.json。
- 完成摘要：固定基线与可重跑Git夹具，保存源码追踪证据；复盘：发现Windows自动CRLF转换，夹具采用换行归一化断言；桌面复现未执行。

验收人：Codex自检（非桌面E2E验收）。

### AZ-005 · 制定兼容支持与发布分层

- 状态：done；负责人：Codex；角色：产品；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-001；候选代码依赖：沿用验收依赖；预计：0.5d；实际：0.02d。
- 范围：定义预览、Beta、稳定版与已验证/部分支持/未验证能力。
- 交付物：制定兼容支持与发布分层的版本化交付及验收记录。
- 验收：平台、扩展、远程和 Desktop 能力均有状态定义；1.0 门槛写成可审阅条件。
- 验证：支持矩阵与发布策略评审。
- 关联审查：F12；风险：基线与团队容量未知；先确认口径再承诺日期。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：确认入口代码与基线，按scope交付并执行verification。
- 当前复评：document_complete；结论：preserve historical ledger status; qualify current verification separately；下一步：同步当前版本与实际支持范围。
- 复评剩余：同步当前版本与实际支持范围；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-005-support-matrix.md。
- 完成摘要：冻结预览Beta稳定版与已验证能力分类；复盘：编译和示例测试不冒充桌面或真实扩展验收。

验收人：Codex证据分类自检。

### AZ-006 · 确定容量预算和可执行排期

- 状态：done；负责人：Codex；角色：负责人；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-002, AZ-003, AZ-005；候选代码依赖：沿用验收依赖；预计：0.5d；实际：0.02d。
- 范围：确认实际人员、每周期可用工作日、角色覆盖和外部供应条件。
- 交付物：确定容量预算和可执行排期的版本化交付及验收记录。
- 验收：指定 gate 责任人；冻结首个执行周期任务；估算区间与风险缓冲有依据。
- 验证：容量表、责任分配和迭代选择记录。
- 关联审查：F10；风险：基线与团队容量未知；先确认口径再承诺日期。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：确认入口代码与基线，按scope交付并执行verification。
- 当前复评：document_complete；结论：preserve historical ledger status; qualify current verification separately；下一步：按当前剩余工作量重排依赖和容量。
- 复评剩余：按当前剩余工作量重排依赖和容量；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-006-current-cycle.md。
- 完成摘要：冻结本轮执行顺序，不虚构长期人力与日期；复盘：真实模型与发行资源未确认，单列Gate与阻塞。

验收人：Codex容量自检。

## M1 · 任务环境与可信变更

Gate：passed；前置：M0。

退出条件：任务不串环境，实际改动完整进入评审；归档可恢复。

### AZ-007 · 持久化任务环境登记

- 状态：done；负责人：Codex；角色：宿主；优先级：P0。
- 仓库：AgentZ；验收依赖：AZ-002, AZ-003；候选代码依赖：沿用验收依赖；预计：2d；实际：0.08d。
- 范围：实现 task_id→environment_id、路径、分支、base SHA 的版本化记录。
- 交付物：持久化任务环境登记的版本化交付及验收记录。
- 验收：创建后重载保持绑定；重复创建幂等；旧会话迁移或明确提示。
- 验证：数据库集成测试与旧数据迁移样例。
- 关联审查：F01；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：AZ-008接入每轮后端环境校验。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前精确版本登记/幂等/迁移测试。
- 复评剩余：当前精确版本登记/幂等/迁移测试；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-007-registry.md。
- 完成摘要：持久任务环境登记与冻结起点已实现，4项SQLite测试及宿主check通过；复盘：不以登记层验收冒充执行路由或桌面续聊验收；后续008/009接入。

验收人：Codex工程自检（非桌面E2E）。

### AZ-008 · 后端解析并验证执行环境

- 状态：done；负责人：Codex；角色：宿主；优先级：P0。
- 仓库：AgentZ；验收依赖：AZ-007；候选代码依赖：沿用验收依赖；预计：1.5d；实际：0.08d。
- 范围：chat_send 从任务登记解析路径并校验仓库、分支与 worktree。
- 交付物：后端解析并验证执行环境的版本化交付及验收记录。
- 验收：非法、失效、跨任务环境被拒绝；前端任意路径不覆盖登记绑定。
- 验证：环境解析正反例集成测试。
- 关联审查：F01, F06；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：校验worktree登记与公共Git目录，防止前端路径改绑。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：复核全部执行入口require_binding传播并测丢失绑定。
- 复评剩余：复核全部执行入口require_binding传播并测丢失绑定；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-008-execution.md。
- 完成摘要：每轮后端解析验证环境，排队后再次校验，路径重定向和失效不回退；复盘：工具执行期间的并发分支变化和系统沙箱仍待后续治理。

验收人：Codex后端契约自检（非桌面E2E）。

### AZ-009 · WorkZ 按任务恢复环境

- 状态：done；负责人：Codex；角色：前端；优先级：P0。
- 仓库：AgentZ；验收依赖：AZ-008；候选代码依赖：沿用验收依赖；预计：1d；实际：1d。
- 范围：移除单一 worktreeRef 的权威，任务切换读取任务环境。
- 交付物：WorkZ 按任务恢复环境的版本化交付及验收记录。
- 验收：A/B 切换与重启续聊保持目录正确；失效环境显示阻塞。
- 验证：双任务桌面旅程录像与文件变更证据。
- 关联审查：F01；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：用户暂跳过原生GUI验收，保留review与候选证据；后续桌面验收再补A/B和重启核对。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：补可执行宿主A/B恢复证据；保留GUI豁免声明。
- 复评剩余：补可执行宿主A/B恢复证据；保留GUI豁免声明；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-009-routing.md；docs/upgrade/evidence/AZ-009-native-capture-attempt.md；docs/upgrade/evidence/AZ-009-native-candidate-preparation.md；docs/upgrade/evidence/AZ-009-native-candidate-running.md；docs/upgrade/evidence/AZ-009-native-input-limitation.md；docs/upgrade/evidence/AZ-009-desktop-state-request.md；docs/upgrade/evidence/AZ-009-visible-candidate-retry.md；docs/upgrade/evidence/ADR-007-gui-waiver-wip.md。
- 完成摘要：代码与单元测试完成；按 ADR-007 用户豁免 GUI 验证。GUI 未在本机原生验证。；复盘：GUI 未验证，风险由用户在 ADR-007 中接受；实际工时为估计值（unmeasured）。。

验收人：用户豁免（ADR-007）。

### AZ-010 · 提供明确起点与非 Git 创建策略

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-007；候选代码依赖：沿用验收依赖；预计：1.5d；实际：0.15d。
- 范围：显式选择提交起点/含本地修改；禁止隐式提交暂存文件。
- 交付物：提供明确起点与非 Git 创建策略的版本化交付及验收记录。
- 验收：冻结起点 SHA；用户本地修改不被无声忽略；非 Git 默认不自动 init/commit。
- 验证：脏仓库、暂存仓库、空仓库集成案例。
- 关联审查：F05, F06；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：AZ-016补明确交付目标与事务预检。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：新版本dirty/specialpath/非Git回归。
- 复评剩余：新版本dirty/specialpath/非Git回归；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-010-explicit-start.md。
- 完成摘要：支持显式提交或当前本地源码起点，冻结SHA，不修改源HEAD/index；非Git及unborn不自动初始化提交；复盘：14真实领域回归、54前端测试、严格clippy、宿主check与生产构建通过；原生点击验收保持独立未验证。

验收人：Codex 自检。

### AZ-011 · 采集完整 worktree 变更集

- 状态：done；负责人：Codex；角色：宿主；优先级：P0。
- 仓库：AgentZ；验收依赖：AZ-008；候选代码依赖：沿用验收依赖；预计：2d；实际：0.3d。
- 范围：统一 committed/staged/unstaged/untracked 读取并保留错误类型。
- 交付物：采集完整 worktree 变更集的版本化交付及验收记录。
- 验收：未提交、新增、删除、改名、二进制和特殊路径均可列举；非法 ref 返回错误。
- 验证：基于临时仓库的 ChangeSet 集成测试。
- 关联审查：F02；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：本项完成；迁移评审UI到task id接口。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：完整工作树类型矩阵当前复跑。
- 复评剩余：完整工作树类型矩阵当前复跑；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-011-change-set.md。
- 完成摘要：冻结基点采集四类工作区变更，NUL路径、来源、改名、二进制和结构化错误已交付；复盘：改名夹具需独特blob；运行时数据库单列排除；旧评审UI留AZ-013，未声称原生验收。

验收人：Codex 自检。

### AZ-012 · 统一 journal 的环境路由

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-008；候选代码依赖：沿用验收依赖；预计：1d；实际：0.25d。
- 范围：journal 查询与 Undo 通过任务环境选择存储根。
- 交付物：统一 journal 的环境路由的版本化交付及验收记录。
- 验收：隔离与本地回合均读到正确 journal；Undo 不碰另一任务文件。
- 验证：隔离 journal 写读回滚集成测试。
- 关联审查：F03；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：本项完成；继续完整评审界面与checkpoint专项。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前隔离journal路由/undo跨任务用例。
- 复评剩余：当前隔离journal路由/undo跨任务用例；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-012-journal-routing.md。
- 完成摘要：journal查询与回滚统一任务根；主目录及A/B同名编辑、重开和独立Undo验证通过；复盘：首次宿主构建较重；保留真实hooks集成，不以领域规则替代回滚。原生UI和checkpoint验收另行跟踪。

验收人：Codex 自检。

### AZ-013 · 更新完整变更评审界面

- 状态：done；负责人：Codex；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-011, AZ-012；候选代码依赖：沿用验收依赖；预计：1.5d；实际：1.5d。
- 范围：区分变更来源、读取失败、二进制和未跟踪文件。
- 交付物：更新完整变更评审界面的版本化交付及验收记录。
- 验收：用户可核对实际文件列表；错误不渲染为没有改动；显示评审版本。
- 验证：评审组件案例与桌面文件核对。
- 关联审查：F02, F03；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：用户暂跳过原生GUI验收，保留review；后续补文件列表与目录核对。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前UI测试及端到端review版本检查。
- 复评剩余：当前UI测试及端到端review版本检查；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-013-review-ui.md；docs/upgrade/evidence/AZ-013-parallel-review.md；docs/upgrade/evidence/ADR-007-gui-waiver-wip.md。
- 完成摘要：代码与单元测试完成；按 ADR-007 用户豁免 GUI 验证。GUI 未在本机原生验证。；复盘：GUI 未验证，风险由用户在 ADR-007 中接受；实际工时为估计值（unmeasured）。。

验收人：用户豁免（ADR-007）。

### AZ-014 · 保存可恢复归档快照

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-011；候选代码依赖：沿用验收依赖；预计：2d；实际：0.05d。
- 范围：删除前记录提交、未提交与声明支持的新文件，保存恢复元数据。
- 交付物：保存可恢复归档快照的版本化交付及验收记录。
- 验收：归档还原内容与快照一致；忽略文件处理规则明确；快照失败阻止清理。
- 验证：包含新文件的归档/恢复完整性测试。
- 关联审查：F04；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：AZ-015接入身份/运行状态/快照与遗漏预检，再替换旧清理入口。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前恢复/故障/ignored文件矩阵。
- 复评剩余：当前恢复/故障/ignored文件矩阵；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-014-archive.md。
- 完成摘要：独立index与原始Git blob保存源码快照，版本化元数据和可达ref保留；新目录恢复逐文件核对，原HEAD/index不变；复盘：Windows verbatim index路径需转换；避免clean/smudge改变字节；ignored/runtime遗漏阻断清理；旧force discard未接入，不宣称完整清理安全。

验收人：Codex 自检。

### AZ-015 · 约束清理对象并报告分步失败

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-014；候选代码依赖：沿用验收依赖；预计：1d；实际：0.25d。
- 范围：验证任务路径与分支归属，移除错误吞噬；运行任务禁止直接清理。
- 交付物：约束清理对象并报告分步失败的版本化交付及验收记录。
- 验收：传入其他任务或任意分支拒绝；清理失败保持可恢复和真实状态。
- 验证：对象越界与删除失败故障案例。
- 关联审查：F04；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：后续恢复接手与原生旅程验收按路线图执行。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前身份错配/分步故障/恢复测试。
- 复评剩余：当前身份错配/分步故障/恢复测试；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-015-retained-cleanup.md。
- 完成摘要：旧强制删除替换为完整checkout保留，持久身份与运行租约约束，分步失败和恢复位置可见；复盘：12领域与53前端测试、真实journal迁移2项、最终宿主检查通过；永久清除和桌面恢复旅程尚未验收，不计为产品Gate。

验收人：Codex 自检。

### AZ-016 · 实现应用与合并事务预检

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-010, AZ-011, AZ-014；候选代码依赖：沿用验收依赖；预计：2d；实际：0.1d。
- 范围：仓库级互斥、HEAD/脏状态/运行检查、冲突恢复与明确目标。
- 交付物：实现应用与合并事务预检的版本化交付及验收记录。
- 验收：主目录脏文件不被隐式改变；并发合并串行；失败后可解释与恢复。
- 验证：并发、脏目录、冲突和 HEAD 变化集成测试。
- 关联审查：F05；风险：涉及用户文件与Git；先临时仓库验证，失败保留恢复快照。
- 回退：失败保留任务环境/快照；回退实现前校验数据版本。
- 阻塞：无；下一步：原生旅程继续单独验收；后续运行状态机须M1通过。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前事务中断与恢复/并行冲突回归。
- 复评剩余：当前事务中断与恢复/并行冲突回归；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-016-merge-design.md；docs/upgrade/evidence/AZ-016-merge-preflight.md；docs/upgrade/evidence/AZ-016-merge-transaction.md。
- 完成摘要：稳定任务与冻结HEAD授权合并，仓库串行和运行租约预检，恢复ref/源码快照与真实分步失败已接宿主和评审；复盘：真实并发与漂移、dirty/冲突、hook修改/abort失败/收据故障和取消waiter测试通过；保留外部进程与原生恢复旅程未验收边界。

验收人：Codex 自检。

### AZ-079 · 修复同轮重复编辑的完整Undo

- 状态：done；负责人：Codex；角色：内核；优先级：P1。
- 仓库：piscis-engine；验收依赖：AZ-003；候选代码依赖：沿用验收依赖；预计：0.5d；实际：0.5d。
- 范围：Undo All按每条路径最早有效快照恢复；保留单步Undo语义与任务环境路由。
- 交付物：固定内核修复补丁、候选及固定依赖验收证据。
- 验收：同轮同文件多次编辑后Undo All恢复轮前字节；先新建后编辑恢复为不存在；失败/已撤销快照不覆盖有效源状态。
- 验证：实际固定内核重复编辑复现、修复后的多次编辑/新建/失败/任务隔离回归与版本接入核对。
- 关联审查：F13；风险：Undo涉及用户字节；仅临时夹具验证，保留现场与原版本配置。
- 回退：保留源码与journal数据库；不把版本切换当作已验证的数据回滚。
- 阻塞：无；下一步：接入可审查固定源码依赖到产品，并运行接入回归；保持GUI暂跳过不计验收。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前重复写/新建再改/失败undone回归。
- 复评剩余：当前重复写/新建再改/失败undone回归；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-079-repeat-undo-repro.md；docs/upgrade/evidence/AZ-079-undo-fix-started.md；docs/upgrade/evidence/AZ-079-undo-core.md；docs/upgrade/evidence/AZ-079-host-candidate-started.md；docs/upgrade/evidence/AZ-079-host-candidate-result.md；docs/upgrade/evidence/AZ-079-reconstructed-candidate.md；docs/upgrade/evidence/AZ-079-reconstructed-validation.md；docs/upgrade/evidence/AZ-079-reconstructed-validation.md；docs/upgrade/evidence/AZ-079-portable-bundle.md；docs/upgrade/evidence/AZ-079-product-integration.md；docs/upgrade/evidence/AZ-079-product-integration.md。
- 完成摘要：将审核后的v0.8.68基线与AZ-029/AZ-079补丁接入vendor固定源码依赖；产品核心4/4、宿主18/18、任务路由2/2通过，--locked复跑通过。；复盘：逐项复核最早快照、未存在文件、失败与已撤销排除、重复Undo不重放；产品Cargo.lock保留既有agentz-task-state记录且locked回归通过。未覆盖GUI、真实模型、跨平台或发布回滚。。

验收人：Codex。

## M2 · 耐久运行与恢复

Gate：pending；前置：M1。

退出条件：停止、重启、断网、权限等待均有真实状态与安全恢复。

### AZ-017 · 持久化 Run 与 Attempt 状态机

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-007；候选代码依赖：沿用验收依赖；预计：2d；实际：2d。
- 范围：定义执行状态与交付状态分离，持久化状态转移。
- 交付物：持久化 Run 与 Attempt 状态机的版本化交付及验收记录。
- 验收：无非法跳转；运行结束不自动发布；历史失败与新 attempt 分开。
- 验证：状态转换表与数据库测试。
- 关联审查：F09；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：限定身份/状态契约已独立复核供AZ018/020候选消费；正式验收仍等待M1及GUI/模型，事件原子接线/恢复另行实现。。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：评审当前共享变动并重冻manifest后执行状态矩阵。
- 复评剩余：评审当前共享变动并重冻manifest后执行状态矩阵；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/ADR-005-candidate-implementation.md；docs/upgrade/evidence/AZ-017-durable-state-candidate.md；docs/upgrade/evidence/AZ-017-implementation-manifest-scoped-r2.json；docs/upgrade/evidence/AZ-017-implementation-manifest-scoped-r2.json。
- 完成摘要：Run/Attempt 状态机：非法跳转被拒绝（穷举 7x7 集成测试）；运行结束不自动发布；历史失败与新 attempt 分开。cargo test 147 单元 + 2 集成通过。；复盘：独立复审 PASS；非阻断：retry 未校验上一 attempt 行状态；崩溃遗留 Running 的对账未接入启动流程（属 AZ-019）。工时为估计值（unmeasured）。。

验收人：独立复审子代理（26b1996e）。

### AZ-018 · 实现耐久事件与断流游标

- 状态：done；负责人：Codex (事件实现)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-017；候选代码依赖：AZ-017；预计：2d；实际：1d。
- 范围：事件携带 task/run/attempt/environment 标识及递增序号。
- 交付物：实现耐久事件与断流游标的版本化交付及验收记录。
- 验收：重复事件幂等；断流可补齐；事件不泄漏到其他任务。
- 验证：事件重放、去重、跨任务路由测试。
- 关联审查：F09, F10；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：本轮本地单Agent冻结候选已打包；后续另轮补范围/GUI模型验收，主实现转AZ023。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前断流去重游标/持久事件故障测试；不要扩到team。
- 复评剩余：当前断流去重游标/持久事件故障测试；不要扩到team；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-018-events-candidate.md；docs/upgrade/evidence/AZ-018-run-events-integration.md；docs/upgrade/evidence/AZ-018-event-replay-ipc.md；docs/upgrade/evidence/AZ-018-current-run-ui-integration.md；docs/upgrade/evidence/AZ-018-metadata-integrity-integration.md；docs/upgrade/evidence/AZ-018-environment-readonly-integration.md；docs/upgrade/evidence/AZ-018-chat-stream-integration.md；docs/upgrade/evidence/AZ-018-passive-preview-integration.md；docs/upgrade/evidence/AZ-018-committed-notice-integration.md；docs/upgrade/evidence/AZ-018-budget-throughput-integration.md；docs/upgrade/evidence/AZ-018-incremental-replay-integration.md；docs/upgrade/evidence/AZ-018-two-phase-replay-integration.md；docs/upgrade/evidence/AZ-018-local-candidate-freeze.md；docs/upgrade/evidence/AZ-018-completion.md。
- 完成摘要：本地单 Agent 耐久事件已交付：标识、递增序号、去重、断流补齐、按任务隔离；149+4+6 项相关测试通过；远程/团队/池化不在范围；复盘：独立实现审查 r2 PASS（非阻塞：生产幂等未覆盖提交后报错；失败生命周期先于终态写入；序号按 attempt 分配为有意设计）；GUI 重连与进程重启未验证，按 ADR-008 豁免。

验收人：independent-reviewer-135fa3b0。

### AZ-019 · 启动时对账中断任务

- 状态：done；负责人：Codex (恢复实现)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-017, AZ-018；候选代码依赖：AZ-017, AZ-018；预计：1.5d；实际：1.5d。
- 范围：进程启动检查未终结 Run、环境和存活进程。
- 交付物：启动时对账中断任务的版本化交付及验收记录。
- 验收：僵尸 running 标为 interrupted 或实际存活；不默默重执行工具。
- 验证：杀进程/重启故障注入证据。
- 关联审查：F09；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：后续清理：终态租约文件回收、残缺租约修复、非 Windows 编译验证。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前Windows kill/reopen试验，复核runtime/lock变动。
- 复评剩余：当前Windows kill/reopen试验，复核runtime/lock变动；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/current-release-scope.md；docs/upgrade/evidence/AZ-019-owner-integration-design.md；docs/upgrade/evidence/AZ-019-explicit-recovery-candidate.md；docs/upgrade/evidence/AZ-019-recovery-integration.md；docs/upgrade/evidence/AZ-019-owner-lease-implementation.md。
- 完成摘要：宿主 owner 租约（OS 独占锁，绑定 Binding 记录）+ 启动对账：杀进程故障注入下僵尸 running 标为 interrupted，活 owner 保留，不重放工具；无租约 Run 保留为 unverifiable；复盘：独立复核 PASS（independent-reviewer-az019，见 AZ-019-independent-review-r1.md）；限制：创建与加锁间崩溃窗口、残缺租约、非 Windows flock 未编译、NFS、无租约路径保留、未真实系统重启。

验收人：independent-reviewer-az019。

### AZ-020 · 持久计划并修正续聊清除语义

- 状态：done；负责人：Codex (计划实现)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-017；候选代码依赖：AZ-017；预计：1d；实际：1d。
- 范围：普通任务续聊保留开放计划；新目标显式结束或替换计划。
- 交付物：持久计划并修正续聊清除语义的版本化交付及验收记录。
- 验收：重启与续聊不清除未完成项；明确取消可以结束计划。
- 验证：clearPlan 与重启恢复集成案例。
- 关联审查：F09；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：AZ-026 修正内核 plan_todo schema 描述。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前续聊/merge/replacement持久计划用例。
- 复评剩余：当前续聊/merge/replacement持久计划用例；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-020-plan-candidate.md；docs/upgrade/evidence/AZ-020-host-service-candidate.md；docs/upgrade/evidence/AZ-020-host-service-candidate.md；docs/upgrade/evidence/AZ-020-host-service-review.md；docs/upgrade/evidence/AZ-020-durable-tool-candidate.md；docs/upgrade/evidence/AZ-020-durable-tool-review.md；docs/upgrade/evidence/AZ-020-heartbeat-environment-lease.md；docs/upgrade/evidence/AZ-020-chat-turn-wiring-review.md；docs/upgrade/evidence/AZ-020-plan-intent-actual-gate.md；docs/upgrade/evidence/AZ-020-codez-plan-controls.md；docs/upgrade/evidence/AZ-020-workz-plan-controls.md；docs/upgrade/evidence/AZ-020-closure-evidence.md。
- 完成摘要：续聊保留开放计划：省略 merge 默认合并，替换不丢未完成项；显式取消/替换经 chat_plan_apply；重启（同进程 SQLite 重开）保留计划；复盘：独立复核 r2 PASS（见 AZ-020-independent-review-r2.md）；残余：内核 schema 描述与宿主默认不一致，记入 AZ-026。

验收人：independent-reviewer-az020。

### AZ-021 · 覆盖排队与执行中的取消

- 状态：done；负责人：Codex (取消闭环)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-017, AZ-018；候选代码依赖：AZ-017, AZ-018；预计：1.5d；实际：1.5d。
- 范围：排队注册取消；执行取消等待工具和子进程终止确认。
- 交付物：覆盖排队与执行中的取消的版本化交付及验收记录。
- 验收：排队任务不在取消后启动；显示 cancelling→cancelled；无新工具调用。
- 验证：队列、长工具、子进程取消案例。
- 关联审查：F09；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：冻结已审本地取消候选并准备一次新Windows安装器，保留全资源与GUI验收缺口。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前queue/run/grace/uncooperative回归。
- 复评剩余：当前queue/run/grace/uncooperative回归；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-021-cancel-integration-baseline.md；docs/upgrade/evidence/AZ-021-owned-completion-candidate.md；docs/upgrade/evidence/AZ-021-host-cancellation-review.md；docs/upgrade/review-execution-map.md；docs/upgrade/evidence/AZ-021-cancellation-integration.md；docs/upgrade/evidence/AZ-021-cancel-grace-fix-r4.md。
- 完成摘要：排队与执行中取消闭环：宿主 cancelling→cancelled、宽限收尾、真实 shell 与 code_run 取消夹具、无协作工具宽限后中断；GUI Stop 由 ADR-008 豁免；复盘：独立复核 r2 BLOCK→r3 PASS→r4 PASS→r5 PASS；见 AZ-021-independent-review-r5.md；非阻塞残余风险见 AZ-021-cancel-grace-fix-r4.md。

验收人：independent-reviewer-135fa3b0。

### AZ-022 · 设置进程与工具恢复契约

- 状态：done；负责人：Codex (工具恢复)；角色：内核；优先级：P1。
- 仓库：piscis-engine；验收依赖：AZ-021；候选代码依赖：AZ-021；预计：2d；实际：2d。
- 范围：定义工具超时、进程终止、UAC 未响应及输出截断结果。
- 交付物：设置进程与工具恢复契约的版本化交付及验收记录。
- 验收：可恢复工具失败返回结构化结果；Windows 子进程树可核对终止。
- 验证：内核契约测试与 Windows 工具故障证据。
- 关联审查：F09；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：固定版本回退；涉及持久数据先备份并验证旧版本兼容。
- 阻塞：无；下一步：实现 kernel 进程与工具恢复状态，补故障注入测试；复核者须与 owner 不同名。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前native shell/code timeout/cancel清理与恢复。
- 复评剩余：当前native shell/code timeout/cancel清理与恢复；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-022-execution.md；docs/upgrade/evidence/AZ-022-windows-e2e.md。
- 完成摘要：Kernel recovery contract (recovery_v1, Job Object tree termination) verified end-to-end in AgentZ on Windows: a real two-level cmd tree started via the kernel shell tool is terminated on timeout and on cancel, with structured outcome and tree_terminated termination and no surviving marker-carrying processes. UAC/elevation and GUI are excluded and covered by ADR-008.；复盘：Retrospective: first attempt used ping, denied by AppContainer (fixed to local cmd loop); first survivor query used -Filter, which silently returned 0 on error (caught by positive control; fixed; failures now fail closed); liveness observer and two-level assertion added per review. Residuals: UAC/elevation not triggered here; recovery_v1 JSON contract documentation not updated in this task; kernel unit tests cover the remaining fault paths.。

验收人：independent-reviewer-az022-r2。

### AZ-023 · 固化模型重试分类与预算

- 状态：done；负责人：Codex (内核)；角色：内核；优先级：P1。
- 仓库：piscis-engine；验收依赖：AZ-017；候选代码依赖：AZ-017；预计：1.5d；实际：2d。
- 范围：审查已有退避；定义429/5xx/鉴权/余额/取消分类与重试上限。
- 交付物：固化模型重试分类与预算的版本化交付及验收记录。
- 验收：暂时错误有界重试；永久错误不循环；取消打断等待。
- 验证：假模型端点故障矩阵测试。
- 关联审查：F09；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：固定版本回退；涉及持久数据先备份并验证旧版本兼容。
- 阻塞：无；下一步：固化模型重试分类与上限预算，补分类表测试；现有复核者与 owner 同名，需换人复核。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前HTTP/cancelbackoff用例及分类边界。
- 复评剩余：当前HTTP/cancelbackoff用例及分类边界；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-023-retry-baseline.md；docs/upgrade/evidence/AZ-023-error-class-candidate.md；docs/upgrade/evidence/AZ-023-retry-integration.md；docs/upgrade/evidence/AZ-023-reconstruction.md；docs/upgrade/evidence/AZ-023-http-reconstruction.md；docs/upgrade/evidence/AZ-023-review-fix-r3.md。
- 完成摘要：Retry classification (401/403 permanent, quota permanent, 5xx transient, 429 rate-limited) and bounded budget verified by classification/loop cases and 4 real local HTTP fault cases; cancel during backoff interrupts within bounds (lower and upper bound); candidate patch chain reproduces on pinned baseline. Kernel-only, no GUI surface.；复盘：Retrospective: two review rounds. Round 1 BLOCK fixed (timing assertions lacked bounds; manifest note and hash chain wrong). Round 2 found only-upper-bound passes without backoff, fixed with lower bound. Round 3 independent re-review PASS (independent-reviewer-az023-r4). Non-blocking residuals: same_model_transient_retries naming, error_class 5xx vs model-not-found ordering; cargo log shows v0.8.63 vs patch v0.8.68.。

验收人：independent-reviewer-az023-r4。

### AZ-024 · 实现任务整体资源与费用预算

- 状态：done；负责人：Codex (宿主)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-017, AZ-023；候选代码依赖：AZ-017, AZ-023；预计：2d；实际：2d。
- 范围：对全部 attempt/成员累计 token、费用、时长和并发配额。
- 交付物：实现任务整体资源与费用预算的版本化交付及验收记录。
- 验收：预算耗尽进入解释性停止；续跑不重置全任务预算；未知价格明示。
- 验证：预算边界与多 attempt 累计案例。
- 关联审查：F09；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：等待 owner 决定：接受未纳入预算的模型调用点残留（自动标题、记忆/技能复盘、curator、consolidate、inline edit、Gateway/Koi）后 done，或另起任务覆盖。
- 当前复评：local_fail_closed_fix_unreviewed；结论：preserve historical ledger status; qualify current verification separately；下一步：独立复核fail-closed改动；覆盖已配置文件丢失、所有模型入口与真实费用语义。。
- 复评剩余：审查已配置文件删除后的历史限额保持；盘点所有模型入口与费用语义；独立实现复核及用户任务验收；验证缺口：没有真实模型费用与长时任务验收；本轮仅完成主调用路径故障测试。。
- 证据：docs/upgrade/evidence/AZ-024-budget-integration-baseline.md；docs/upgrade/evidence/AZ-024-budget-core-review.md；docs/upgrade/evidence/AZ-024-accounting-core-integration.md；docs/upgrade/evidence/AZ-024-reservation-core-integration.md；docs/upgrade/evidence/AZ-024-reservation-core-review.md；docs/upgrade/evidence/current-release-scope.md；docs/upgrade/evidence/AZ-024-closure-evidence.md；docs/upgrade/evidence/AZ-024-closure-evidence.md。
- 完成摘要：按用量预算收口：token、active 时长、并发配额按任务累计，超限拒绝并给出 budget denied 说明，续跑不重置，未知价格明示；金额成本不在本任务范围（另立任务）；复盘：独立复审 r1-r7：r6 与 r7 PASS；残留项见收口证据第4节。

验收人：independent-reviewer-az024-r7。

### AZ-025 · 定义外部动作幂等与恢复对账

- 状态：done；负责人：Codex (外部动作对账)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-018, AZ-019；候选代码依赖：AZ-018, AZ-019；预计：2d；实际：1d。
- 范围：对发送、发布、PR 等意图记录动作键、结果与待对账状态。
- 交付物：定义外部动作幂等与恢复对账的版本化交付及验收记录。
- 验收：执行成功但结果丢失时不盲重放；先查询或要求人工处理。
- 验证：可控外部服务重复/丢结果故障案例。
- 关联审查：F09；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：实现外部动作 intent、单次 permit 与结果丢失显式对账，受控本地服务 SQLite 测试（AZ-019 依赖已满足）。
- 当前复评：domain_only；结论：preserve historical ledger status; qualify current verification separately；下一步：明确接入宿主/实际外部动作并测丢响应，禁止自动重发。
- 复评剩余：明确接入宿主/实际外部动作并测丢响应，禁止自动重发；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-025-contract.md；docs/upgrade/evidence/AZ-025-execution.md；docs/upgrade/evidence/AZ-025-closure-evidence.md。
- 完成摘要：External action intent, single-dispatch permit, lost-result reconciliation without blind replay, and atomic secret-free audit implemented in agentz-task-state (external_action.rs); 12 real SQLite fixtures including a two-connection permit race and an in-process controlled service that loses its result, then reopens with no replay. Domain only; no host provider adapter or real external service.；复盘：Retrospective: domain built in earlier sessions with an earlier review; closure review BLOCKED on run.rs provenance (hash changed by AZ-019 active_runs); fixed by correcting manifest and evidence, current bytes covered by this closure review; second review PASS. Limits: caller-asserted Evidence, unsalted action digest, start origin not compared to run_id, no host/IPC/provider.。

验收人：independent-reviewer-az025。

### AZ-026 · 发布并接入内核契约版本

- 状态：preparing；负责人：Codex (内核固定交付)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-022, AZ-023；候选代码依赖：AZ-022, AZ-023；预计：1d；实际：待记录d。
- 范围：固定包含已验收契约的内核版本，更新锁文件与兼容说明。
- 交付物：发布并接入内核契约版本的版本化交付及验收记录。
- 验收：AgentZ 与固定版本契约测试通过；无未提交 sibling override 依赖。
- 验证：锁文件、版本兼容记录与集成检查。
- 关联审查：F09；风险：跨宿主/内核契约；先验证固定版本，副作用恢复先对账。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：Review five changed files semantically, then independent current-candidate review and formal M2 decision.。
- 当前复评：new_local_freeze_candidate_unreviewed；结论：Keep preparing; do not promote old independent review or M2 to the changed candidate.；下一步：Review five changed files semantically, then independent current-candidate review and formal M2 decision.。
- 复评剩余：Changed-file semantic review；Independent candidate review；Formal M2 decision；验证缺口：Old reviewer fingerprints do not cover current source；Real-DB evaluation ignored; no cross-platform or release acceptance。
- 证据：docs/upgrade/evidence/AZ-026-contract.md；docs/upgrade/evidence/AZ-026-execution.md；docs/upgrade/evidence/AZ-026-independent-review.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

## M3 · 权限、秘密与扩展边界

Gate：pending；前置：M0。

退出条件：秘密保存失败关闭；危险动作授权明确；Webview 隔离实测。

### AZ-027 · 冻结权限分级与授权有效范围

- 状态：done；负责人：Codex；角色：产品；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-002；候选代码依赖：沿用验收依赖；预计：1d；实际：0.03d。
- 范围：定义文件、进程、网络、浏览器、桌面、连接器权限与危险动作。
- 交付物：冻结权限分级与授权有效范围的版本化交付及验收记录。
- 验收：每类权限包含授权对象、有效期、可撤销性和升级路径。
- 验证：权限矩阵与具体动作示例评审。
- 关联审查：F06, F08；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：按Gate推进AZ-028持久化与资源执行校验；AZ-029独立安全修复可先推进。
- 当前复评：current_source_read_only_no_tests_rerun；结论：retain_contract_done；下一步：保留文档范围完成，实际资源执行逐项由028/034/后续任务验收。。
- 复评剩余：保留文档范围完成，实际资源执行逐项由028/034/后续任务验收。；本轮仅阅读矩阵，未重新运行动作例评。；验证缺口：本轮仅阅读矩阵，未重新运行动作例评。。
- 证据：docs/upgrade/evidence/AZ-027-permission-contract.md。
- 完成摘要：冻结四级授权、六类资源范围/期限/撤销/升级、持久意图状态与受保护审批surface；完成12动作例评；复盘：核对现有一次性审批源码；逐类检查授权对象有效期撤销升级齐全，明确模型不能自批，区分契约与未实现边界；不计作M3 Gate通过。

验收人：Codex 自检。

### AZ-028 · 实现持久审批意图与范围校验

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-027；候选代码依赖：沿用验收依赖；预计：2d；实际：0.05d。
- 范围：审批绑定动作参数摘要、任务、环境与有效期。递归预检之后的路径替换、固定内核排除遍历、LSP 结构化返回过滤和操作系统隔离分别属于 AZ-080、AZ-081、AZ-082、AZ-034。
- 交付物：实现持久审批意图与范围校验的版本化交付及验收记录。
- 验收：参数变化/过期/换任务重新审批；重启不保留已失效授权。
- 验证：批准后参数变更与重启案例。
- 关联审查：F06, F09；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：审批契约完成；继续 AZ-034 操作系统边界策略，使用真实正反文件/进程/网络验证并清楚区分不支持的平台。。
- 当前复评：current_source_read_only_no_tests_rerun；结论：retain_scope_revalidate；下一步：重跑当前approval领域/host/PermissionCard并对齐声明入口清单。。
- 复评剩余：重跑当前approval领域/host/PermissionCard并对齐声明入口清单。；当前源码未重测；真实受保护UI点击未运行。；验证缺口：当前源码未重测；真实受保护UI点击未运行。。
- 证据：docs/upgrade/evidence/AZ-028-durable-core.md；docs/upgrade/evidence/AZ-028-broker-surface.md；docs/upgrade/evidence/AZ-028-execution-hooks.md；docs/upgrade/evidence/AZ-028-existing-schema.md；docs/upgrade/evidence/AZ-028-lifecycle-pairing.md；docs/upgrade/evidence/AZ-028-binding-during-approval.md；docs/upgrade/evidence/AZ-028-direct-context.md；docs/upgrade/evidence/AZ-028-harness-coverage.md；docs/upgrade/evidence/AZ-028-background-read-guard.md；docs/upgrade/evidence/AZ-028-search-start.md；docs/upgrade/evidence/AZ-028-cached-search.md；docs/upgrade/evidence/AZ-028-lsp-diagnostics.md；docs/upgrade/evidence/AZ-028-diagnostic-preview.md；docs/upgrade/evidence/AZ-028-symbol-resources.md；docs/upgrade/evidence/AZ-028-recursive-preflight.md；docs/upgrade/evidence/AZ-028-expiry-card.md；docs/upgrade/evidence/AZ-028-lsp-execution-gate.md；docs/upgrade/evidence/AZ-028-protected-search-scope.md；docs/upgrade/evidence/AZ-028-single-submit-card.md；docs/upgrade/evidence/AZ-028-stale-handler.md；docs/upgrade/evidence/AZ-028-preview-retry.md；docs/upgrade/evidence/AZ-028-unavailable-dismiss.md；docs/upgrade/evidence/AZ-028-closed-preview.md；docs/upgrade/evidence/AZ-028-acceptance-review-2026-10-06.md。
- 完成摘要：原始持久审批验收按当前 host 集成与 SQLite 自动化通过：参数/任务/环境/期限绑定，重启作废旧授权；17项宿主执行集成与18项审批领域测试通过。；复盘：已逐项核对原始验收标准及自动化测试；原生GUI点击和生产打包未运行，保持未验证；AZ-034 OS 隔离单列，不将工具裁剪称作沙箱。。

验收人：Codex。

### AZ-029 · 让秘密加密失败阻止保存

- 状态：done；负责人：Codex；角色：内核；优先级：P1。
- 仓库：piscis-engine；验收依赖：AZ-003；候选代码依赖：沿用验收依赖；预计：1d；实际：0.03d。
- 范围：移除 SecretStore/加密失败时的明文 fallback，改原子写入。
- 交付物：让秘密加密失败阻止保存的版本化交付及验收记录。
- 验收：密钥损坏、随机源失败、不可写时旧配置保留且不产生明文。
- 验证：加密失败注入和文件内容检查。
- 关联审查：F08；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：固定版本回退；涉及持久数据先备份并验证旧版本兼容。
- 阻塞：无；下一步：需确定视觉API key加密后的降级契约：允许旧版直接读取、要求用户恢复加密备份后再回退，或暂不支持跨此schema降级；决定后更新验收与发布门禁。
- 当前复评：current_source_read_only_no_tests_rerun；结论：retain_scope_revalidate；下一步：重跑损坏密钥/加密错误/写入失败与格式downgrade夹具并核对kernel source provenance。。
- 复评剩余：重跑损坏密钥/加密错误/写入失败与格式downgrade夹具并核对kernel source provenance。；未注入秘密/失败；未重跑rollback。；验证缺口：未注入秘密/失败；未重跑rollback。。
- 证据：docs/upgrade/evidence/AZ-029-secret-save.md；docs/upgrade/evidence/AZ-029-rollback-review-started.md；docs/upgrade/evidence/AZ-029-rollback-review-result.md；docs/upgrade/evidence/AZ-079-product-integration.md；docs/upgrade/evidence/ADR-004-protected-downgrade.md。
- 完成摘要：加密失败关闭与原子保存已进入固定vendor依赖；跨格式回退按ADR-004移交受保护备份恢复发行门禁；复盘：保存失败注入及固定依赖证据通过；用户确认推荐安全恢复契约。旧版视觉key直接降级不兼容保留为已知事实，AZ-063/AZ-065发行前必须实现保护备份与版本拒绝，未声称回滚验收通过。。

验收人：Codex自检（用户确认回退契约）。

### AZ-030 · 接入系统凭据保护与迁移

- 状态：done；负责人：Codex；角色：内核；优先级：P1。
- 仓库：piscis-engine；验收依赖：AZ-029；候选代码依赖：沿用验收依赖；预计：2d；实际：0.25d。
- 范围：先以 Windows 为参考实现系统主密钥保护及旧密钥迁移接口。
- 交付物：接入系统凭据保护与迁移的版本化交付及验收记录。
- 验收：旧加密配置可迁移；系统保护不可用有明确错误；不静默降级。
- 验证：迁移、凭据不可用与恢复证据。
- 关联审查：F08；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：固定版本回退；涉及持久数据先备份并验证旧版本兼容。
- 阻塞：无；下一步：AZ077/AZ078推进平台适配；AZ063/AZ065落实ADR004恢复与降级门禁，AZ083处理回归探针。
- 当前复评：current_source_read_only_no_tests_rerun；结论：retain_windows_scope_revalidate；下一步：分离Windows已测范围与其他平台候选；检查provider新增源码来源，重跑迁移/保护失败。。
- 复评剩余：分离Windows已测范围与其他平台候选；检查provider新增源码来源，重跑迁移/保护失败。；本轮未调用DPAPI/Keychain/keyring；跨用户及跨平台恢复未验。；验证缺口：本轮未调用DPAPI/Keychain/keyring；跨用户及跨平台恢复未验。。
- 证据：docs/upgrade/evidence/AZ-030-windows-key-protection.md。
- 完成摘要：Windows DPAPI用户级主密钥保护、原子旧密钥迁移及新版配置严格加载已接入固定vendor；保护不可用与损坏恢复明确失败关闭。；复盘：产品及固定候选凭据专项各13通过、宿主18通过1忽略、产品严格Clippy与宿主既有例外Clippy通过；完整回归358通过1失败1忽略，AppContainer夹具不稳定独立AZ083跟踪。未运行多用户、GUI、跨平台、发行和旧版回滚验收。。

验收人：Codex自检（无独立用户验收）。

### AZ-031 · 贯通秘密引用与脱敏出口

- 状态：blocked；负责人：未分配；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-030, AZ-077, AZ-078；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：设置与模型只暴露 reference/掩码；检查日志、诊断、导出和 app_control。
- 交付物：贯通秘密引用与脱敏出口的版本化交付及验收记录。
- 验收：真实密钥不进入模型或诊断；替换、删除、存在状态独立操作。
- 验证：植入唯一测试秘密后扫描所有声明出口。
- 关联审查：F08；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：waiting external macOS/Linux accounts via AZ-077/078 (ADR-007)；下一步：Review cross-process settings writes, historical plaintext snapshots and real v1 migration/rollback, then scan remaining outlets and confirm per-key deletion in native UI.。
- 当前复评：model_mcp_batched_reference_and_v2_persistence_candidate_unreviewed；结论：Keep blocked until reference DTO, replace/delete/exists operations, all-outlet scan and platform credential adapters are accepted.；下一步：Review cross-process settings writes, historical plaintext snapshots and real v1 migration/rollback, then scan remaining outlets and confirm per-key deletion in native UI.。
- 复评剩余：Cross-process reference writes and native per-key deletion behavior；Existing v1 files, backups and plaintext snapshots; real rollback migration；All declared outlet scan；Native UI and AZ-077/AZ-078 platform credential acceptance；验证缺口：Settings response tests cover model API keys and MCP command/URL/argument/env/header values, not native UI；No complete outlet scan; rollback snapshot intentionally retains original configuration and old audit files were not rewritten；macOS/Linux adapters not accepted；Connector mutation corruption fixture is source-level only; full credential encryption and recovery are unverified。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-032 · 修复 Webview 功能与隔离策略

- 状态：review；负责人：Codex；角色：扩展；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-027；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：受控 origin/nonce CSP/资源映射/消息协议和面板 state 隔离。
- 交付物：修复 Webview 功能与隔离策略的版本化交付及验收记录。
- 验收：示例能渲染；脚本不能访问宿主 DOM/IPC/其他面板；不开放宽泛 unsafe-inline。
- 验证：真实 Tauri Webview 正反测试。
- 关联审查：F07；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：保留真实Tauri正反验收缺口；验证打包nonce供给及跨panel资源泄漏。。
- 当前复评：current_source_read_only_no_tests_rerun；结论：acceptance_pending；下一步：保留真实Tauri正反验收缺口；验证打包nonce供给及跨panel资源泄漏。。
- 复评剩余：保留真实Tauri正反验收缺口；验证打包nonce供给及跨panel资源泄漏。；真实Webview DOM/IPC隔离与资源handler联动未测；13旧测试不算当前pass。；验证缺口：真实Webview DOM/IPC隔离与资源handler联动未测；13旧测试不算当前pass。。
- 证据：docs/upgrade/evidence/AZ-032-webview-isolation-started.md；docs/upgrade/evidence/AZ-032-webview-isolation-started.md；docs/upgrade/evidence/AZ-032-webview-isolation-started.md；docs/upgrade/evidence/AZ-032-parallel-review.md。
- 完成摘要：待填写；复盘：每面板UUID令牌、localResourceRoots、Rust canonical路径白名单/大小限制、释放时注销已实现；Rust协议处理器1项测试通过，前端13项测试及两侧typecheck、ESLint、rustfmt和配置解析通过。真实Tauri运行行为未验证。。

验收人：未分配。

### AZ-033 · 记录技能扩展来源与授权变更

- 状态：done；负责人：Codex；角色：扩展；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-027；候选代码依赖：沿用验收依赖；预计：1.5d；实际：0.2d。
- 范围：安装/升级记录来源、版本、校验与新增能力。
- 交付物：记录技能扩展来源与授权变更的版本化交付及验收记录。
- 验收：更新新增权限时显式处理；卸载撤销相关授权；来源可追溯。
- 验证：安装→升级→撤销→卸载案例。
- 关联审查：F06, F11；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：实现与定向验证齐备；复核能力只作为本轮技能声明，并核对SHA-256边界已在证据中明确。
- 当前复评：current_source_read_only_no_tests_rerun；结论：retain_scope_revalidate；下一步：重跑service install-update-uninstall真实临时DB/目录夹具并核查扩展范围措辞。。
- 复评剩余：重跑service install-update-uninstall真实临时DB/目录夹具并核查扩展范围措辞。；当前skills/service及provenance未重测；真实市场安装/GUI未验。；验证缺口：当前skills/service及provenance未重测；真实市场安装/GUI未验。。
- 证据：docs/upgrade/evidence/AZ-033-skill-provenance-authorization.md；docs/upgrade/evidence/AZ-033-skill-provenance-authorization.md。
- 完成摘要：技能安装与升级记录来源、版本、SHA-256及请求工具能力历史；升级新增工具前失败关闭并保留旧版；卸载记录撤权意图、删技能和Hub锁；复盘：验收标准由本地SQLite/临时文件服务集成覆盖，5项 skills tests 与3项安装路径 tests 通过；TS typecheck、ESLint和Rust格式检查通过。密码学上游签名校验与真实注册表/桌面验证未覆盖，已在证据中列明。。

验收人：Codex。

### AZ-034 · 实现受限执行的首个策略

- 状态：done；负责人：Codex；角色：内核；优先级：P1。
- 仓库：piscis-engine；验收依赖：AZ-027, AZ-028；候选代码依赖：沿用验收依赖；预计：2d；实际：2d。
- 范围：以 Windows 参考策略实现约束目录/工具/网络的可验证最小范围。
- 交付物：实现受限执行的首个策略的版本化交付及验收记录。
- 验收：越界操作被实际拒绝；不支持的平台明确显示能力边界；worktree 不宣称沙箱。
- 验证：越界文件、进程与网络正反案例。
- 关联审查：F06；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：固定版本回退；涉及持久数据先备份并验证旧版本兼容。
- 阻塞：无；下一步：自检任务验收标准、能力披露、越界文件/移动/网络与取消收割证据；确认平台/GUI/LSP边界没有被写成通过后决定关闭或继续。
- 当前复评：current_source_read_only_no_tests_rerun；结论：retain_windows_scope_revalidate；下一步：重跑当前Windows正反文件/网络/取消/ACL案例，审新增kernel差异。。
- 复评剩余：重跑当前Windows正反文件/网络/取消/ACL案例，审新增kernel差异。；本轮不执行AppContainer；跨平台、低权限、跨卷/多用户与发行未验。；验证缺口：本轮不执行AppContainer；跨平台、低权限、跨卷/多用户与发行未验。。
- 证据：docs/upgrade/evidence/AZ-034-capability-disclosure-started.md；docs/upgrade/evidence/AZ-034-capability-disclosure-started.md；docs/upgrade/evidence/AZ-034-capability-disclosure-started.md；docs/upgrade/evidence/AZ-034-capability-disclosure-started.md；docs/upgrade/evidence/AZ-034-capability-disclosure-started.md；docs/upgrade/evidence/AZ-034-capability-disclosure-started.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-production-runner.md；docs/upgrade/evidence/AZ-034-acceptance-review-2026-10-06.md。
- 完成摘要：交付并自检Windows AppContainer命令参考策略，接入code_run/shell；工作区ACL生命周期、无网络/越界拒绝、Job超时取消收割、失败关闭和能力披露有Windows自动化覆盖。内核353 passed、pool25 passed、Clippy/rustfmt通过。；复盘：任务Acceptance criteria通过并由Codex自检记录：真实OS拒绝越界文件/网络操作；非Windows能力状态明确且不执行；managed worktree/local不宣称为OS沙箱。未覆盖的GUI、Linux运行、LSP、release、模型及回滚保持未验证/单独跟踪；真实DB评估1项ignored。用户明确要求跳过原生GUI，这没有被当作通过证据。。

验收人：Codex (self-review)。

### AZ-077 · macOS 系统凭据保护适配

- 状态：blocked；负责人：Codex；角色：内核；优先级：P1。
- 仓库：piscis-engine；验收依赖：AZ-030；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：适配macOS系统凭据接口、可用性检测与旧配置迁移。
- 交付物：接入系统凭据保护与迁移的版本化交付及验收记录。
- 验收：macOS系统凭据可用时正常读写；不可用时明确失败且不保存明文；旧加密配置迁移可恢复。
- 验证：macOS真实运行的读写、撤销、损坏和迁移证据。
- 关联审查：F08；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：固定版本回退；涉及持久数据先备份并验证旧版本兼容。
- 阻塞：外部 macOS/Linux 账号未就绪（ADR-007，非豁免）；下一步：Locally specify missing/delete idempotency and explicit accessible/keychain/signing-identity recovery policy, build mock status fixtures; reserve actual read/write/lock/revoke/corruption/migration for real signed macOS session.。
- 当前复评：keychain_delete_idempotency_cross_target_checked_native_unverified；结论：Keep blocked for actual signed macOS acceptance; do not infer platform readiness from cross-target typechecking.；下一步：Choose and verify Keychain accessibility and signed-identity recovery policy in a real macOS test account; run temporary-item read/write, lock, revoke, corruption and migration tests with cleanup receipts.。
- 复评剩余：Explicit accessible/login-versus-data-protection policy validated against signed app identity；Native macOS read/write, lock, revoke, corruption and migration receipts；Independent unified review and rollback compatibility；验证缺口：No native macOS run or Security.framework link；Ignored temporary-credential test has only been typechecked；Cross-identity recovery and Keychain accessibility not established。
- 证据：docs/upgrade/evidence/AZ-077-keychain-design-started.md；docs/upgrade/evidence/AZ-077-keychain-adapter.md；docs/upgrade/evidence/ADR-007-gui-waiver-wip.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-078 · Linux 系统凭据保护适配

- 状态：blocked；负责人：Codex；角色：内核；优先级：P1。
- 仓库：piscis-engine；验收依赖：AZ-030；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：适配Linux系统凭据接口、可用性检测与旧配置迁移。
- 交付物：接入系统凭据保护与迁移的版本化交付及验收记录。
- 验收：Linux系统凭据可用时正常读写；不可用时明确失败且不保存明文；旧加密配置迁移可恢复。
- 验证：Linux真实运行的读写、撤销、损坏和迁移证据。
- 关联审查：F08；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：固定版本回退；涉及持久数据先备份并验证旧版本兼容。
- 阻塞：外部 macOS/Linux 账号未就绪（ADR-007，非豁免）；下一步：Add bounded service boundary and bounded key-create retry with fault fixtures; then real Linux session tests for unavailable bus/locked/prompt/corrupt/revoke/migration and cleanup.。
- 当前复评：linux_secret_service_cleanup_retry_fixture_cross_target_checked_native_unverified；结论：Keep blocked for native Linux acceptance; typechecking is not runtime evidence.；下一步：Add a bounded service boundary and controlled key-create retry with fault fixtures; then run the ignored temporary-item test and unavailable-bus, lock, prompt, corruption, revoke and migration cases in a real Linux user session.。
- 复评剩余：Bounded D-Bus service boundary and controlled create retry；Native Linux Secret Service read/write, lock, prompt, corruption, revoke and migration receipts；Independent unified review and rollback compatibility；验证缺口：No native Linux service run or cleanup receipt；Ignored temporary-credential test only typechecked；D-Bus waiting and lost-response create behavior remain unbounded。
- 证据：docs/upgrade/evidence/AZ-078-secret-service-adapter.md；docs/upgrade/evidence/ADR-007-gui-waiver-wip.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-080 · 复查递归预检之后的路径替换

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-028；候选代码依赖：沿用验收依赖；预计：1d；实际：1d。
- 范围：file_search 与 file_list 的有界链接预检之后、实际读取之前，再次确认路径仍在已批准根内。
- 交付物：预检通过后路径被替换时拒绝读取的版本化交付及验收记录。
- 验收：预检通过后把目录换成工作区外 junction 或符号链接时，搜索和列举被拒绝，且不返回区外内容。
- 验证：Windows junction 时序夹具，记录仍由 AZ-081 覆盖的内核遍历排除。
- 关联审查：F06；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：复核额外授权根回归、预检后替换测试是否拦在provider前，并核对路径名读取留下的末端TOCTOU边界；验证严格Clippy基线诊断已准确记录。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前junction时序fixture复跑；句柄no-follow另立范围。
- 复评剩余：当前junction时序fixture复跑；句柄no-follow另立范围；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-080-junction-revalidation.md；docs/upgrade/evidence/AZ-080-junction-revalidation.md。
- 完成摘要：递归 file_search/file_list 在有界链接预检后、以及下游hook返回后、provider调用前再次验证搜索起始路径仍在授权根中。Windows junction 被注入到预检后和下游hook后两个时序点，均拒绝工具调用；approval_execution 18 passed、1 ignored。严谨记录严格Clippy既有4 lint阻断及allow-list检查通过。；复盘：AZ-080验收矩阵满足：预检完成后外部junction替换会在provider之前拒绝，file_search和file_list均被覆盖；原有nested-junction和审批provider回归通过。实现不声称句柄绑定的原子无TOCTOU，末端窄窗口已列明。用户要求跳过的native GUI没有被记作已验收。。

验收人：Codex (self-review)。

### AZ-081 · 让固定内核遍历遵守排除目录

- 状态：done；负责人：Codex；角色：内核；优先级：P1。
- 仓库：piscis-engine；验收依赖：AZ-028；候选代码依赖：沿用验收依赖；预计：1.5d；实际：0.2d。
- 范围：固定内核 file_search 遍历遵守与宿主预检相同的排除目录，而不是只在外层跳过噪声目录。
- 交付物：固定内核补丁、候选及固定依赖验收证据。
- 验收：排除目录不会被内核遍历读入结果；宿主预检与内核遍历结果一致。
- 验证：临时仓库对比预检与内核遍历；固定依赖版本核对。
- 关联审查：F06；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：保留源码与原版本配置；不把版本切换当作已验证的数据回滚。
- 阻塞：无；下一步：对齐宿主 check_search_links 与 FileSearchTool glob/grep 遍历过滤语义，先添加对比夹具暴露任何越界读取。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前忽略canary与预检一致性测试。
- 复评剩余：当前忽略canary与预检一致性测试；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-081-excluded-search-directories.md。
- 完成摘要：固定内核与宿主共享目录排除规则，实际遍历在访问前剪枝，并将补丁应用至不可变依赖候选验证。；复盘：复核内核/宿主列表来源一致；候选补丁和归档SHA与manifest核对；内核、宿主集成、lint、格式与台账验证均通过。限制明确：Windows自动化而非GUI/发布或跨平台验收；排除目录是遍历策略而非安全边界。。

验收人：Codex自检（无独立用户验收）。

### AZ-082 · 过滤 LSP 结构化返回中的资源路径

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-028；候选代码依赖：沿用验收依赖；预计：1.5d；实际：0.25d。
- 范围：lsp 已要求一次性审批。审批通过后仍过滤或包装返回中的工作区外路径，文本截断不算路径隔离。
- 交付物：结构化返回路径过滤的版本化交付及验收记录。
- 验收：返回中的工作区外文件路径被拒绝或改写为不可跟随引用；审批提示与真实返回一致。
- 验证：含区外 URI 的诊断或符号替身测试；真实语言服务器另记未验证。
- 关联审查：F06；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：等 AZ-028 done 后再做。语言服务器沙箱仍属于 AZ-034。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前嵌套URI/rename/edit/异常scheme fixture；live LSP另测。
- 复评剩余：当前嵌套URI/rename/edit/异常scheme fixture；live LSP另测；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-082-lsp-response-resource-scope.md。
- 完成摘要：LSP在渲染前将结构化资源URI限制到任务工作区，对越界/未知路径失败关闭并标记省略数。；复盘：核验固定依赖重建与补丁指纹；8项IDE工具候选测试、宿主18项审批执行集成、Clippy allow-list、格式和台账检查通过。候选live LSP、GUI和release未运行，不作为产品验收。。

验收人：Codex自检（无独立用户验收）。

### AZ-083 · 修复 AppContainer 后代回收验收的不稳定性

- 状态：done；负责人：Codex；角色：内核；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-034；候选代码依赖：沿用验收依赖；预计：1d；实际：0.1d。
- 范围：定位完整内核回归中后代存活标记偶发失败；区分非交互 timeout 夹具失效与真实 Job 生命周期缺陷。
- 交付物：可重复的后代存活与Job关闭回收验收及必要修复。
- 验收：延时探针在非交互AppContainer下可靠等待；持有Job时后代存活，关闭后后代退出；失败证据保留且不靠放宽断言。
- 验证：针对性重复运行及完整内核回归；记录首次失败和重跑结果。
- 关联审查：F06；风险：秘密与权限边界；在测试账户/测试凭据环境验证，失败关闭。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：继续AZ077/AZ078平台凭据适配；保持GUI与发行门禁未验收。
- 当前复评：bounded_source_review；结论：preserve historical ledger status; qualify current verification separately；下一步：当前Windows handshake连续回归，保留失败与超时断言。
- 复评剩余：当前Windows handshake连续回归，保留失败与超时断言；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-083-descendant-handshake.md。
- 完成摘要：以ready/release握手及正向对照替换依赖控制台输入的timeout后代探针；重复验证与完整内核回归通过。；复盘：新专项与三次重复全部通过；完整内核359 passed/1 ignored，pool25 passed，doc test1 ignored；格式与台账检查通过。仅测试夹具修改，生产runner未修改；首次失败与入口修正记录保留，未做GUI/跨平台/压力验收。。

验收人：Codex自检（当前Windows自动化）。

## M4 · CodeZ 与 WorkZ 同任务交接

Gate：pending；前置：M1, M2。

退出条件：任务环境可在人与 Agent 之间连续交接，产物可检查与验收。

### AZ-035 · 提供统一任务详情读取接口

- 状态：preparing；负责人：Codex (任务详情)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-018, AZ-020；候选代码依赖：AZ-018, AZ-020；预计：1d；实际：待记录d。
- 范围：Task DTO 聚合环境、运行、预算、计划和产物引用。
- 交付物：提供统一任务详情读取接口的版本化交付及验收记录。
- 验收：两个视图读同一权威记录；失效引用返回明确状态。
- 验证：任务详情 API 契约案例。
- 关联审查：F01, F09, F10；风险：人与Agent并发改动；交接先冻结活动操作并保留未保存内容。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：独立复核当前源码；原生跨视图导航验证。
- 当前复评：implementation_candidate_unreviewed；结论：Keep preparing until independent review and unified cross-view/desktop acceptance.；下一步：Independently review exact source content; run cross-view task/environment navigation and desktop read acceptance in the unified validation phase.。
- 复评剩余：Pin a current source manifest and independent implementation review；Exercise cross-view navigation under the same task/environment binding；Run native desktop read with an environment lease and assess non-atomic revisions；验证缺口：No native desktop GUI run in this batch；No independent source review after the current edits。
- 证据：docs/upgrade/evidence/current-release-scope.md；docs/upgrade/evidence/AZ-035-plan-readonly.md；docs/upgrade/evidence/AZ-035-host-readonly-candidate.md；docs/upgrade/evidence/AZ-035-host-readonly-integration.md；docs/upgrade/evidence/AZ-035-two-workspace-integration.md。
- 完成摘要：两个本地视图共用授权只读详情服务；领域5/plan16/host4/frontend11及tsc实际通过；GUI未验；复盘：待填写。

验收人：未分配。

### AZ-036 · CodeZ 打开当前任务环境

- 状态：preparing；负责人：Codex；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-035；候选代码依赖：AZ-035；预计：1.5d；实际：待记录d。
- 范围：编辑器、终端、调试和预览显式绑定 environment。
- 交付物：CodeZ 打开当前任务环境的版本化交付及验收记录。
- 验收：从 WorkZ 打开后所有面板使用任务根；主项目与任务目录可分辨。
- 验证：同任务跨视图路径与操作核对。
- 关联审查：F01, F10；风险：人与Agent并发改动；交接先冻结活动操作并保留未保存内容。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：等待用户确定权限根与脏缓冲策略后实现。
- 当前复评：read_only_codez_route_candidate_unreviewed；结论：Keep preparing; this is a bounded read-only route, not full task-environment operation binding.；下一步：Implement task-bound host authorization for CodeZ editor, terminal, debug and preview operations and same-source session continuation; then perform unified native handoff acceptance.。
- 复评剩余：Authorize all four CodeZ operation classes against current task/environment identity；Preserve WorkZ session/source identity in CodeZ continuation；Independent source review and native dirty-buffer/active-process handoff；验证缺口：No writable CodeZ task environment route in this batch；No native desktop or cross-platform handoff run；No independent review after the current route edit。
- 证据：docs/upgrade/evidence/AZ-036-routing-design.md；docs/upgrade/evidence/AZ-036-navigation-review.md；docs/upgrade/evidence/AZ-036-execution.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-037 · 实现交接与人类编辑反馈

- 状态：deferred；负责人：未分配；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-036, AZ-016；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：在 Agent 与人交接时同步环境和修改上下文，处理活动执行。
- 交付物：实现交接与人类编辑反馈的版本化交付及验收记录。
- 验收：接手可暂停；人的编辑进入下一轮上下文；不覆盖脏缓冲。
- 验证：暂停→人工编辑→继续→评审完整旅程。
- 关联审查：F01, F09；风险：人与Agent并发改动；交接先冻结活动操作并保留未保存内容。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：Nothing is implemented yet and the task depends on AZ-036, which is still preparing, so it should remain pending behind that prerequisite.；下一步：设计最小handoff状态与人类修改上下文消费，不能把stop/newRun称pause resume。。
- 当前复评：current_source_read_only_no_tests_rerun；结论：implement_after_task_route；下一步：设计最小handoff状态与人类修改上下文消费，不能把stop/newRun称pause resume。。
- 复评剩余：设计最小handoff状态与人类修改上下文消费，不能把stop/newRun称pause resume。；全旅程未实现/未测。；验证缺口：全旅程未实现/未测。。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-038 · 统一产物清单与版本

- 状态：preparing；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-035, AZ-011；候选代码依赖：AZ-035；预计：1.5d；实际：待记录d。
- 范围：Artifact 支持代码 ChangeSet、文档、报告、网页与验证记录。
- 交付物：统一产物清单与版本的版本化交付及验收记录。
- 验收：文件定位到环境与版本；旧产物可打开；删除不留下错误成功状态。
- 验证：代码和非代码产物 API 集成案例。
- 关联审查：F02, F03；风险：人与Agent并发改动；交接先冻结活动操作并保留未保存内容。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：复核artifacts/host新漂移并重跑domain/host/client，明确文本候选支持范围。。
- 当前复评：current_source_read_only_no_tests_rerun；结论：revalidate_and_bound_scope；下一步：复核artifacts/host新漂移并重跑domain/host/client，明确文本候选支持范围。。
- 复评剩余：复核artifacts/host新漂移并重跑domain/host/client，明确文本候选支持范围。；当前新diff未重新验；真实旧版本GUI/删除unknowncommit未验。；验证缺口：当前新diff未重新验；真实旧版本GUI/删除unknowncommit未验。。
- 证据：docs/upgrade/evidence/AZ-038-execution.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-039 · 改造交付评审与验收动作

- 状态：blocked；负责人：Codex；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-038, AZ-013；候选代码依赖：AZ-038, AZ-013；预计：1.5d；实际：待记录d。
- 范围：首屏呈现变更摘要、产物、验证、未验证、用户动作。
- 交付物：改造交付评审与验收动作的版本化交付及验收记录。
- 验收：执行完成与验收/应用/发布分开；无证据不能展示已验证。
- 验证：三条旅程评审可用性记录。
- 关联审查：F02, F11；风险：人与Agent并发改动；交接先冻结活动操作并保留未保存内容。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：Labelling without receipts is implemented and tested in code, but acceptance requires three recorded native usability journeys that have not been run.；下一步：按三旅程补回执来源/评审动作；先复核AssistantPanel新pin漂移。。
- 当前复评：current_source_read_only_no_tests_rerun；结论：finish_acceptance_receipts；下一步：按三旅程补回执来源/评审动作；先复核AssistantPanel新pin漂移。。
- 复评剩余：按三旅程补回执来源/评审动作；先复核AssistantPanel新pin漂移。；三条旅程可用性未跑，当前wrapper/summary未重测。；验证缺口：三条旅程可用性未跑，当前wrapper/summary未重测。；历史阻塞仍在，解锁条件未获得新的实测证据。。
- 证据：docs/upgrade/evidence/AZ-039-execution.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-040 · 简化首用输入与运行状态

- 状态：preparing；负责人：Codex；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-035, AZ-028；候选代码依赖：AZ-035；预计：1.5d；实际：待记录d。
- 范围：输入显示目标、环境、模型、权限摘要，高级项按需展开。
- 交付物：简化首用输入与运行状态的版本化交付及验收记录。
- 验收：首次任务无需猜路径；执行中能看到阻塞、最近动作和预算。
- 验证：至少5次代表用户走查及问题记录。
- 关联审查：F06, F10；风险：人与Agent并发改动；交接先冻结活动操作并保留未保存内容。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：Run five representative walkthroughs and native approval/first-run journeys, then independently review current source; latest-tool candidate passes 18 focused tests.。
- 当前复评：current_session_approval_and_latest_tool_candidate_unreviewed；结论：Keep preparing until representative walkthroughs and native GUI acceptance are completed.；下一步：Perform five representative walkthroughs and native approval/first-run journeys; independently review the current source.。
- 复评剩余：Five representative user walkthroughs；Native GUI approval and first-run acceptance；Independent current-source review；验证缺口：No native approval journey；No five-user walkthrough record；Current AssistantPanel change not independently reviewed。
- 证据：docs/upgrade/evidence/AZ-040-input-design.md；docs/upgrade/evidence/AZ-040-execution.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

## M5 · CodeZ 日常工程体验

Gate：pending；前置：M1。

退出条件：脏缓冲、语言服务、调试、终端和远程核心旅程通过。

### AZ-041 · 建立脏缓冲与外部改动冲突流程

- 状态：preparing；负责人：Codex (编辑器保护)；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-036；候选代码依赖：AZ-036；预计：1.5d；实际：待记录d。
- 范围：编辑器比较磁盘版本，冲突提供查看、保留、合并选择。
- 交付物：建立脏缓冲与外部改动冲突流程的版本化交付及验收记录。
- 验收：Agent/外部改动不覆盖未保存内容；恢复后版本可解释。
- 验证：未保存编辑与 Agent 并发写入案例。
- 关联审查：F11；风险：平台及扩展行为差异；声明具体已验证环境。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：原生验证脏缓冲与外部改动冲突；提交独立复核（指纹已变，旧复核失效）。
- 当前复评：current_source_read_only_no_tests_rerun；结论：finish_conflict_workflow；下一步：审index新修改并复现typing/save/watcher/恢复交错；明确保留/查看/合并与磁盘base。。
- 复评剩余：审index新修改并复现typing/save/watcher/恢复交错；明确保留/查看/合并与磁盘base。；当前并发实际文件/GUI未跑；历史helper pass不代表新index。；验证缺口：当前并发实际文件/GUI未跑；历史helper pass不代表新index。。
- 证据：docs/upgrade/evidence/AZ-041-next-scope.md；docs/upgrade/evidence/AZ-041-next-scope-review.md；docs/upgrade/evidence/AZ-041-next-scope-review.md；docs/upgrade/evidence/AZ-041-root-checkpoint.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-042 · 语言服务崩溃恢复

- 状态：done；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-003；候选代码依赖：沿用验收依赖；预计：1.5d；实际：1.5d。
- 范围：LSP 健康状态、重启、文档重新同步和过期诊断清理。
- 交付物：语言服务崩溃恢复的版本化交付及验收记录。
- 验收：语言服务重启后补全/诊断恢复；陈旧结果不污染新模型。
- 验证：至少 TS/Rust 服务异常旅程。
- 关联审查：F11；风险：平台及扩展行为差异；声明具体已验证环境。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：无依赖：补 Monaco 恢复/提示/切换代码与测试；桌面实测按 ADR-008 豁免 GUI。
- 当前复评：current_source_read_only_no_tests_rerun；结论：revalidate_scope_completion；下一步：核对ADR008豁免适用与原verification，修文档窗口描述并保留真实TS/Rust异常演练项。。
- 复评剩余：核对ADR008豁免适用与原verification，修文档窗口描述并保留真实TS/Rust异常演练项。；本轮不重跑10fixtures；native服务/Monaco未验即使done。；验证缺口：本轮不重跑10fixtures；native服务/Monaco未验即使done。。
- 证据：docs/upgrade/evidence/AZ-042-lsp-recovery-candidate.md；docs/upgrade/evidence/AZ-042-closure-evidence.md。
- 完成摘要：恢复预算在成功重连后重置；回归测试覆盖多轮崩溃恢复；10/10 通过；复盘：reeval 2026-10-10 指出成功后未重置 recoveryAttempts，2026-10-11 复核确认代码已修复（reset 位于 reconnect 成功之后），并以临时副本变异测试证明测试可区分。桌面 kill-drill 与 Monaco 行为未验证，按 ADR-008 豁免。。

验收人：independent-reviewer-az042。

### AZ-043 · 调试会话生命周期回归

- 状态：preparing；负责人：Codex (调试生命周期)；角色：扩展；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-036；候选代码依赖：AZ-036；预计：1.5d；实际：待记录d。
- 范围：明确 launch/attach、断点、异常结束和环境映射。
- 交付物：调试会话生命周期回归的版本化交付及验收记录。
- 验收：停止后无残留调试会话；文件跳转到正确环境。
- 验证：本地示例调试录屏与异常退出证据。
- 关联审查：F11；风险：平台及扩展行为差异；声明具体已验证环境。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：重跑当前host/client/controller后本地adapter launch/attach与异常退出记录。。
- 当前复评：current_source_read_only_no_tests_rerun；结论：native_acceptance_pending；下一步：重跑当前host/client/controller后本地adapter launch/attach与异常退出记录。。
- 复评剩余：重跑当前host/client/controller后本地adapter launch/attach与异常退出记录。；无native调试录屏、进程树OS证明及同task环境跳转验收。；验证缺口：无native调试录屏、进程树OS证明及同task环境跳转验收。。
- 证据：docs/upgrade/evidence/AZ043-next-scope.md；docs/upgrade/evidence/AZ-043-root-checkpoint.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-044 · 终端生命周期与路径回归

- 状态：preparing；负责人：Codex (终端生命周期)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-036, AZ-021；候选代码依赖：AZ-036, AZ-021；预计：1d；实际：待记录d。
- 范围：终端 cwd、恢复语义、关闭和 Agent 进程关联。
- 交付物：终端生命周期与路径回归的版本化交付及验收记录。
- 验收：切任务不串终端根；关闭时相关进程与提示符合策略。
- 验证：多终端、任务切换和进程终止案例。
- 关联审查：F11；风险：平台及扩展行为差异；声明具体已验证环境。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：独立审当前Job/terminal新增改动并重跑multiPTY/close/error，完成Task scope路由后验。。
- 当前复评：current_source_read_only_no_tests_rerun；结论：revalidate_native_lifecycle；下一步：独立审当前Job/terminal新增改动并重跑multiPTY/close/error，完成Task scope路由后验。。
- 复评剩余：独立审当前Job/terminal新增改动并重跑multiPTY/close/error，完成Task scope路由后验。；当前native测试未跑；其他OS与真实任务切换未验。；验证缺口：当前native测试未跑；其他OS与真实任务切换未验。。
- 证据：docs/upgrade/evidence/AZ-044-terminal-checkpoint.md；docs/upgrade/evidence/AZ-044-terminal-checkpoint.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-045 · 远程断连与环境恢复

- 状态：blocked；负责人：Codex；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-008；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：SSH 主机断连后文件/LSP/终端/Agent 状态统一并可重连。
- 交付物：远程断连与环境恢复的版本化交付及验收记录。
- 验收：断连不当成本地路径；主机密钥改变拒绝；重连不重复写入。
- 验证：受控 SSH 断网/重连/主机密钥案例。
- 关联审查：F11；风险：平台及扩展行为差异；声明具体已验证环境。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：waiting real SSH remote environment (external)；下一步：准备可认证owned POSIX fixture验证文件/LSP/terminal/Agent断连重连且零重复写。。
- 当前复评：current_source_read_only_no_tests_rerun；结论：authenticated_fixture_pending；下一步：准备可认证owned POSIX fixture验证文件/LSP/terminal/Agent断连重连且零重复写。。
- 复评剩余：准备可认证owned POSIX fixture验证文件/LSP/terminal/Agent断连重连且零重复写。；无真实认证远端写/黑洞/重连旅程，本轮未重跑。；验证缺口：无真实认证远端写/黑洞/重连旅程，本轮未重跑。；历史阻塞仍在，解锁条件未获得新的实测证据。。
- 证据：docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-source-manifest.json；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-remote-recovery-candidate.md；docs/upgrade/evidence/AZ-045-isolated-ssh.md；docs/upgrade/evidence/AZ-045-source-manifest.json。
- 完成摘要：远程URI失败关闭、绑定session、镜像/LSP/转发恢复及受控自动化候选已交付；隔离SSH仅主机密钥握手通过，完整旅程未验收。；复盘：待填写。

验收人：未分配。

### AZ-046 · 建立首批真实扩展兼容档案

- 状态：deferred；负责人：未分配；角色：扩展；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-032；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：选定首批语言/格式化/Git/调试扩展和具体版本。
- 交付物：建立首批真实扩展兼容档案的版本化交付及验收记录。
- 验收：每个档案有实测能力、缺口和升级回归说明；未验证不标兼容。
- 验证：至少6个扩展的安装/运行证据。
- 关联审查：F07, F11；风险：平台及扩展行为差异；声明具体已验证环境。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：Nothing of the deliverable exists and the dependency AZ-032 is still in review, so it should wait for AZ-032 and then be executed as a fresh evidence task.；下一步：固定6扩展ID/version/来源/能力清单，做安装运行升级正反矩阵，不标未知兼容。。
- 当前复评：current_source_read_only_no_tests_rerun；结论：implement_matrix；下一步：固定6扩展ID/version/来源/能力清单，做安装运行升级正反矩阵，不标未知兼容。。
- 复评剩余：固定6扩展ID/version/来源/能力清单，做安装运行升级正反矩阵，不标未知兼容。；6扩展真实安装/运行/回归全部缺。；验证缺口：6扩展真实安装/运行/回归全部缺。。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-047 · 整理日常工程交互一致性

- 状态：preparing；负责人：Codex (交互一致性)；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-040, AZ-041；候选代码依赖：AZ-040, AZ-041；预计：1d；实际：待记录d。
- 范围：快捷键、焦点、键盘操作、空态和错误恢复覆盖核心旅程。
- 交付物：整理日常工程交互一致性的版本化交付及验收记录。
- 验收：键盘可完成主要路径；错误给出下一动作；中英文语义一致。
- 验证：交互与基本可访问性走查清单。
- 关联审查：F10, F11；风险：平台及扩展行为差异；声明具体已验证环境。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：原生验证快捷键不与 Monaco/终端冲突；独立复核。
- 当前复评：current_source_read_only_no_tests_rerun；结论：expand_journey_a11y；下一步：复核当前dock/terminal修改，扩展到输入审批评审产物/空态error中英走查。。
- 复评剩余：复核当前dock/terminal修改，扩展到输入审批评审产物/空态error中英走查。；原生keyboard/accessibility全旅程未验；历史15tests不是当前pass。；验证缺口：原生keyboard/accessibility全旅程未验；历史15tests不是当前pass。。
- 证据：docs/upgrade/evidence/AZ-047-keyboard-checkpoint.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

## M6 · Desktop 工作与集成

Gate：pending；前置：M2, M3, M4。

退出条件：无仓库任务、研究文档、连接器与桌面操作具有受控交付闭环。

### AZ-048 · 无仓库任务独立工作目录

- 状态：preparing；负责人：Codex (无仓库目录)；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-035, AZ-038；候选代码依赖：AZ-035, AZ-038；预计：1.5d；实际：待记录d。
- 范围：独立项目less任务环境与数据路径，不隐式创建 Git。
- 交付物：无仓库任务独立工作目录的版本化交付及验收记录。
- 验收：重开仍在原目录；产物可归档恢复；清理不删除用户指定目录。
- 验证：无项目研究任务完整旅程。
- 关联审查：F05, F09；风险：外部副作用和前台资源；先测试账户与受控应用，独立授权。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：Run a real first turn and continuation in separate projectless sessions, reopen after restart, verify catalog-loss refusal and legacy free-session compatibility; review final source and rollback evidence before acceptance.。
- 当前复评：host_trusted_projectless_turn_and_observation_wired_local_checks_passed；结论：Keep preparing pending unified native and recovery acceptance.；下一步：Run a real first turn and continuation in separate projectless sessions, reopen after restart, verify catalog-loss refusal and legacy free-session compatibility; review final source and rollback evidence before acceptance.。
- 复评剩余：Native desktop first-turn and continuation journey；Reopen and legacy free-session compatibility journey；Catalog-loss recovery and rollback acceptance；Unified source review and evidence freeze；验证缺口：No real desktop/model turn executed；No native reopen/legacy/rollback journey executed。
- 证据：docs/upgrade/evidence/AZ-048-directory-checkpoint.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-049 · 研究报告交付模板

- 状态：deferred；负责人：未分配；角色：产品；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-038, AZ-039；候选代码依赖：沿用验收依赖；预计：1d；实际：待记录d。
- 范围：定义问题、来源、结论、局限与可打开报告产物。
- 交付物：研究报告交付模板的版本化交付及验收记录。
- 验收：来源可定位；事实与推断分开；任务验收标准可核对。
- 验证：固定研究任务结果评审。
- 关联审查：F11；风险：外部副作用和前台资源；先测试账户与受控应用，独立授权。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：No code exists and it hard-depends on AZ-038 and AZ-039, so it should be scheduled after those are done.；下一步：Review one fixed research report and open each source locator, exercise native task-bound capture/read/edit/continue, then independently review the final source and acceptance criteria.。
- 当前复评：structured_report_render_wired_to_task_bound_artifact_capture_local_checks_passed；结论：Keep deferred until upstream artifact/delivery contracts and unified review are accepted.；下一步：Review one fixed research report and open each source locator, exercise native task-bound capture/read/edit/continue, then independently review the final source and acceptance criteria.。
- 复评剩余：Fixed research report and opened source locators；Native capture/read/edit/continue journey；Independent final review and upstream acceptance；验证缺口：No real research task or source fact-check；No native desktop artifact journey。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-050 · 文档产物编辑预览闭环

- 状态：deferred；负责人：未分配；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-048；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：先支持 Markdown/HTML 文档的生成、预览、人工编辑和版本。
- 交付物：文档产物编辑预览闭环的版本化交付及验收记录。
- 验收：预览与保存一致；改后可继续 Agent 任务；旧版本可恢复。
- 验证：文档创建→编辑→续聊→归档旅程。
- 关联审查：F11；风险：外部副作用和前台资源；先测试账户与受控应用，独立授权。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：Only read-only preview and version capture/delete exist; the edit, restore and continuation pieces are missing and the dependency AZ-048 is not closed, so it should follow AZ-048.；下一步：Run native create/edit/restore/continue/archive journey, verify HTML/Markdown preview and failed-send recovery, then independently review final source and upstream contracts.。
- 当前复评：host_resolved_versioned_continuation_local_checks_passed；结论：Keep deferred until complete native document journey and independent review.；下一步：Run native create/edit/restore/continue/archive journey, verify HTML/Markdown preview and failed-send recovery, then independently review final source and upstream contracts.。
- 复评剩余：Native complete document journey；Preview and failed-send recovery；Independent final review and upstream acceptance；验证缺口：No live native send or model turn；No full document lifecycle or preview exercise。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-051 · 连接器授权生命周期

- 状态：pending；负责人：未分配；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-028, AZ-031；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：以一个连接器完成连接、范围显示、过期、刷新与撤销。
- 交付物：连接器授权生命周期的版本化交付及验收记录。
- 验收：撤销立即阻止新调用；过期不循环重试；凭据不写入对话。
- 验证：受控测试账户授权/过期/撤销案例。
- 关联审查：F06, F08；风险：外部副作用和前台资源；先测试账户与受控应用，独立授权。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：补OAuth start/callback/refresh及受保护凭据存储；保留外部账号端到端验收。。
- 当前复评：local_oauth_expiry_callback_mcp_revalidation_and_lifecycle_guard_candidate_unreviewed；结论：Keep pending until the current end-to-end authorization and revocation path is independently reviewed and exercised with a real provider account.；下一步：Review the current PKCE callback, refresh and per-call MCP guard; decide provider-side revocation support, then run one real provider authorization/expiry/revoke journey and in-flight cancellation case during unified acceptance.。
- 复评剩余：Independent review of the current OAuth callback, refresh and storage candidate；Concurrent native lifecycle race fixture, provider revoke policy and already in-flight MCP call handling；Actual provider authorization, expiry, refresh and revoke receipts；验证缺口：No external provider account or real OAuth endpoint was exercised；No native desktop callback journey, live MCP call or provider-side revoke receipt；No unified independent implementation review。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-052 · 外部发送与发布交付流程

- 状态：deferred；负责人：未分配；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-051, AZ-025；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：呈现目标账户、接收方/发布位置、内容预览与执行结果。
- 交付物：外部发送与发布交付流程的版本化交付及验收记录。
- 验收：审批后参数不可偷偷改变；失败可对账；不重复发送。
- 验证：发送与发布沙盒案例及动作日志。
- 关联审查：F06, F09；风险：外部副作用和前台资源；先测试账户与受控应用，独立授权。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：Neither the flow nor the frozen-parameter path exists, and both prerequisites AZ-025 and AZ-051 are unfinished, so it cannot be verified yet.；下一步：Connect POST/PUT/DELETE and gateway.send to prepare/frozen approval/start/completion/unknown reconciliation; create sandbox-local tests before live external account receipt.。
- 当前复评：partial；结论：deferred_with_reason；下一步：Connect POST/PUT/DELETE and gateway.send to prepare/frozen approval/start/completion/unknown reconciliation; create sandbox-local tests before live external account receipt.。
- 复评剩余：Connect POST/PUT/DELETE and gateway.send to prepare/frozen approval/start/completion/unknown reconciliation; create sandbox-local tests before live external account receipt.；验证缺口：No frozen target/recipient UI, unknown-result reconciliation or deduped-send acceptance.。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-053 · 浏览器上下文与抢占恢复

- 状态：pending；负责人：未分配；角色：宿主；优先级：P1。
- 仓库：RobotZ；验收依赖：AZ-028, AZ-018；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：页面引用、导航变化、用户接管和任务工具结果绑定。
- 交付物：浏览器上下文与抢占恢复的版本化交付及验收记录。
- 验收：旧页面引用明确失效；用户接管停止 Agent 后续动作。
- 验证：浏览器导航/抢占/恢复案例。
- 关联审查：F11；风险：外部副作用和前台资源；先测试账户与受控应用，独立授权。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：Review and pin an immutable RobotZ revision with generation-aware refs, then run AgentZ's pinned stale-ref and delayed-action takeover fixtures.。
- 当前复评：unshipped_dependency_fix_local_tests_pass；结论：Keep pending until a reviewed immutable RobotZ revision is pinned and AgentZ's browser navigation/takeover path is verified.；下一步：Review and pin an immutable RobotZ revision with generation-aware refs, then run AgentZ's pinned stale-ref and delayed-action takeover fixtures.。
- 复评剩余：Reviewed immutable RobotZ dependency revision；AgentZ pinned dependency test；Actual task/page takeover revocation and delayed-action refusal；Unified review；验证缺口：Local tests used dirty RobotZ working tree, not AgentZ pinned dependency；No GUI takeover or delayed action test。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-054 · Windows 桌面操作最小闭环

- 状态：pending；负责人：未分配；角色：宿主；优先级：P1。
- 仓库：RobotZ；验收依赖：AZ-034, AZ-053；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：仅声明支持的应用与动作；实现焦点校验、用户抢占与权限区处理。
- 交付物：Windows 桌面操作最小闭环的版本化交付及验收记录。
- 验收：焦点丢失停止；敏感区按策略处理；不支持动作有明确说明。
- 验证：受控应用跨窗口与用户抢占旅程。
- 关联审查：F06, F11；风险：外部副作用和前台资源；先测试账户与受控应用，独立授权。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：AZ-053 完成后：挂载 RobotZ UIA 能力到 AgentZ 宿主；补焦点身份、用户抢占门控与敏感区策略；受控应用验收仍需原生证据。
- 当前复评：not_started；结论：fix_then_done；下一步：Define one supported Windows application/action set, add host focus identity and user takeover gate plus sensitive-region policy; test pure guards first and preserve native unverified label.。
- 复评剩余：Define one supported Windows application/action set, add host focus identity and user takeover gate plus sensitive-region policy; test pure guards first and preserve native unverified label.；验证缺口：No AgentZ native UIA integration or controlled app acceptance. GUI waiver does not fabricate OS operation evidence.。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-055 · 建立跨任务桌面资源租约

- 状态：deferred；负责人：未分配；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-054；候选代码依赖：沿用验收依赖；预计：1d；实际：待记录d。
- 范围：前台焦点、浏览器控制与共享文件资源受任务租约管理。
- 交付物：建立跨任务桌面资源租约的版本化交付及验收记录。
- 验收：两个任务不能同时争抢同一前台资源；取消释放租约。
- 验证：双任务抢占/取消/崩溃租约案例。
- 关联审查：F01, F09；风险：外部副作用和前台资源；先测试账户与受控应用，独立授权。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：The foreground resource it would lease does not exist until AZ-054 lands, although the global-turn-flag close-guard defect should be fixed now.；下一步：After browser/native action binding, implement distinct task-attempt resource capability with queue/busy/revoke/drop and cancellation fixtures; do not repurpose process owner lease.。
- 当前复评：not_started；结论：deferred_with_reason；下一步：After browser/native action binding, implement distinct task-attempt resource capability with queue/busy/revoke/drop and cancellation fixtures; do not repurpose process owner lease.。
- 复评剩余：After browser/native action binding, implement distinct task-attempt resource capability with queue/busy/revoke/drop and cancellation fixtures; do not repurpose process owner lease.；验证缺口：No two-task front-resource contention/revocation fixture.。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

## M7 · 性能、评测与跨平台质量

Gate：pending；前置：M0。

退出条件：指标有基线、资源有预算、核心旅程自动化和三平台证据齐全。

### AZ-056 · 核心生命周期 E2E 门禁

- 状态：blocked；负责人：未分配；角色：质量；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-003, AZ-009, AZ-013, AZ-015, AZ-019, AZ-021；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：自动化隔离创建→双任务→续聊→重启→评审→归档恢复。
- 交付物：核心生命周期 E2E 门禁的版本化交付及验收记录。
- 验收：每次候选发布运行并保存日志；失败阻断发布。
- 验证：固定仓库E2E结果与失败产物。
- 关联审查：F11；风险：硬件/模型影响测量；固定环境并报告缺失证据。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：依赖 AZ-003、009、013、015、019、021 均已 done。仍缺原生 A/B 与文件列表证据，E2E 门禁未执行，不能以域内证据签收。；下一步：Run the candidate workflow after resource preflight is satisfied; add host/native A/B, review/file-list and recovery receipts before acceptance.。
- 当前复评：isolated_kernel_lifecycle_gate_candidate_unreviewed；结论：Keep blocked until native A/B and review UI journey, actual workflow run and unified acceptance are evidenced.；下一步：Run the candidate workflow after resource preflight is satisfied; add host/native A/B, review/file-list and recovery receipts before acceptance.。
- 复评剩余：Actual candidate workflow run；Native A/B and review/file-list journey；Host restart and recovery validation；Unified acceptance；验证缺口：No workflow dispatch；No native GUI；No model or host process restart。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-057 · Windows 故障与长时运行基准

- 状态：blocked；负责人：未分配；角色：质量；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-004, AZ-026, AZ-032；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：在Windows测冷启动、两小时资源增长、长对话和故障恢复。
- 交付物：Windows 故障与长时运行基准的版本化交付及验收记录。
- 验收：数据含硬件/版本；内存和取消残留有阈值；失败有任务归因。
- 验证：Windows基准与故障报告。
- 关联审查：F11；风险：硬件/模型影响测量；固定环境并报告缺失证据。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：The benchmark needs a stable Windows machine, a real model endpoint and frozen builds from AZ-026 and AZ-032, none of which exist yet.；下一步：Freeze budgets and implement owned Windows sampler/fault harness locally; record hardware/build/attribution and actual soak run separately from waived visual checks.。
- 当前复评：windows_resource_sampler_orphan_tracking_candidate_unreviewed；结论：Keep blocked until frozen candidate build, hardware, thresholds and actual workload results are available.；下一步：Freeze hardware/build/budget and run AgentZ cold-start, fault and two-hour workload with the sampler; preserve raw receipts.。
- 复评剩余：Reviewed resource and timing budgets；Frozen build and hardware profile；AgentZ cold-start/fault/two-hour workload；Real model attribution and unified review；验证缺口：Only PowerShell sampler fixtures；No AgentZ benchmark；No real model run；AZ-026/AZ-032 not accepted。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-058 · macOS/Linux 核心旅程验证

- 状态：blocked；负责人：未分配；角色：质量；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-056, AZ-057；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：在两平台各跑安装后核心旅程和工具终止检查。
- 交付物：macOS/Linux 核心旅程验证的版本化交付及验收记录。
- 验收：两平台均有证据；差异进入支持矩阵；严重缺口不标稳定支持。
- 验证：macOS/Linux运行记录与缺口清单。
- 关联审查：F11, F12；风险：硬件/模型影响测量；固定环境并报告缺失证据。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：waiting macOS/Linux test accounts (external, ADR-007)；下一步：Prepare reusable platform fixture checklist/scripts locally; obtain macOS/Linux actual sessions and run install/start/task/terminate/recover, recording differences before support upgrade.。
- 当前复评：platform_journey_protocol_prepared_native_unverified；结论：Keep blocked until owned macOS and Linux sessions are available and actual journeys complete.；下一步：After AZ-056/AZ-057 candidate freeze, execute separate macOS and Linux native cards with model and credential fixtures; record differences in the support matrix.。
- 复评剩余：Owned macOS and Linux test sessions；Frozen candidate and model endpoint；Actual install/start/task/terminate/recover receipts；Independent review and support matrix update；验证缺口：No macOS or Linux runtime session；No native journey, process-group or rollback result。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-059 · 优化首屏与重型资源加载

- 状态：blocked；负责人：未分配；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-004, AZ-040；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：按profile拆分主入口、Monaco、图布局和Markdown加载。
- 交付物：优化首屏与重型资源加载的版本化交付及验收记录。
- 验收：相同基准有前后对照；不损坏离线加载和功能；预算由基线评审确定。
- 验证：构建体积与冷启动对比数据。
- 关联审查：F10, F11；风险：硬件/模型影响测量；固定环境并报告缺失证据。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：Minified production build now passes; controlled before/after cold-start and graph-open measurements, native offline checks, and AZ-040 acceptance are still missing.；下一步：Measure controlled initial and graph-open load against a frozen baseline; then run native offline journeys after AZ-040 review.。
- 当前复评：code_map_lazy_chunk_minified_build_unreviewed；结论：Keep blocked until controlled before/after startup and offline/native functional acceptance are resolved.；下一步：Measure controlled initial load and graph-open load, then run native offline journey after AZ-040 review.。
- 复评剩余：Controlled before/after startup and graph-open measurement；Native offline feature check；AZ-040 dependency and unified review；验证缺口：No controlled before/after startup measurement；No native desktop run；Earlier minifier failure root cause unproven。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-060 · 消除长任务资源增长

- 状态：pending；负责人：未分配；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-057；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：定位监听器、编辑器模型、PTY、扩展与索引的主要泄漏来源。
- 交付物：消除长任务资源增长的版本化交付及验收记录。
- 验收：关闭任务/面板后资源回收；两小时增长符合冻结预算。
- 验证：资源profile前后对比与回归案例。
- 关联审查：F11；风险：硬件/模型影响测量；固定环境并报告缺失证据。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：Exercise concurrent Tauri terminal-create command invocations and a real repository graph-build close, then run the two-hour close/restart soak against a frozen AZ-057 budget.。
- 当前复评：owned_pty_model_listener_watcher_extension_graph_snapshot_candidate；结论：Keep pending until whole-resource census and frozen-budget soak acceptance.；下一步：Exercise concurrent Tauri terminal-create command invocations and a real repository graph-build close, then run the two-hour close/restart soak against a frozen AZ-057 budget.。
- 复评剩余：Concurrent Tauri create fixture；Real repository graph-build close；Two-hour close/restart soak against frozen AZ-057 budget；验证缺口：No native-window snapshot invocation；No concurrent Tauri create invocation fixture；No long resource soak。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-061 · 统一诊断关联和脱敏导出

- 状态：pending；负责人：未分配；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-018, AZ-031；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：日志关联 task/run/tool/environment，导出含版本与必要诊断。
- 交付物：统一诊断关联和脱敏导出的版本化交付及验收记录。
- 验收：可从失败任务定位路径；测试秘密不出现在导出；用户可预览范围。
- 验证：诊断导出样例与脱敏扫描。
- 关联审查：F08, F11；风险：硬件/模型影响测量；固定环境并报告缺失证据。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：独立复核 span 字段与导出脱敏；决定无任务绑定时的记录策略。
- 当前复评：bounded_redacted_log_manifest_candidate_unreviewed；结论：Keep pending until AZ-031, reliable task/run/tool/environment correlation, full-source secret review and native journey are complete.；下一步：Verify task/run/environment/tool fields with a real failing tool and native export, then run all-source secret scan after AZ-031.。
- 复评剩余：AZ-031 secret-reference dependency；Real failing-tool correlation；All-source secret inventory and scan；Native preview/export journey；Unified review；验证缺口：Current settings including MCP and connector files only; other secret stores not fully inventoried；Correlation tested only with synthetic spans; tool field may be absent in real logs；No native desktop journey。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-062 · 建立评测趋势与质量门禁

- 状态：pending；负责人：未分配；角色：质量；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-004, AZ-056；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：按固定模型/配置记录成功率、干预、成本与宿主故障趋势。
- 交付物：建立评测趋势与质量门禁的版本化交付及验收记录。
- 验收：重复运行可比较；归因模型与产品；缺失样本不当成功。
- 验证：趋势报告和回归门禁规则。
- 关联审查：F11；风险：硬件/模型影响测量；固定环境并报告缺失证据。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：Wire the gate to the stable lifecycle harness, collect repeated actual desktop/model summaries, then perform unified review.。
- 当前复评：local_trend_gate_and_count_reconciliation_candidate_unreviewed；结论：Keep pending until the lifecycle harness and repeated actual target-scope measurements support a quality decision.；下一步：Wire the gate to the stable lifecycle harness, collect repeated actual desktop/model summaries, then perform unified review.。
- 复评剩余：Stable lifecycle harness dependency AZ-056；Repeated actual target-scope measurements；Unified acceptance review；验证缺口：Only synthetic-kernel baseline gate exercised；No desktop or new live model trend。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

## M8 · 发行、迁移与支持

Gate：pending；前置：M2, M3, M4, M5, M6, M7, MB。

退出条件：安装、签名、更新、迁移和恢复经过候选版验证。

### AZ-063 · 版本化数据迁移与备份恢复

- 状态：pending；负责人：未分配；角色：宿主；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-007, AZ-017, AZ-030；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：覆盖任务、会话、环境、设置和产物schema升级。
- 交付物：版本化数据迁移与备份恢复的版本化交付及验收记录。
- 验收：升级失败保留备份；旧版本读写策略明确；恢复后任务绑定正确。
- 验证：至少两历史版本迁移/损坏恢复夹具。
- 关联审查：F09, F12；风险：需要证书、机器和Beta用户；明确外部阻塞，保留可恢复安装。
- 回退：停止发行或更新渠道；按验证过的安装与数据恢复流程回退。
- 阻塞：无；下一步：强制写入者静默与一致的 SQLite 快照（应用内接入）；原子 in-place 恢复暂不做（用户选择 restore-to-new-location）；真实旧版本数据夹具待提供；原生恢复验收。
- 当前复评：offline_encrypted_backup_preview_candidate_unreviewed；结论：Keep pending until protected versioned backup/restore and historical migration fixtures are accepted.；下一步：Integrate enforced writer quiescence and coherent SQLite snapshot capture, then implement guarded atomic in-place recovery with original preservation and credential/task binding validation using historical fixtures.。
- 复评剩余：Enforced writer quiescence and coherent live SQLite snapshot of config/key and project state；Atomic restore with original preservation and identity binding validation；At least two historical-version migration/corruption fixtures；Native recovery and rollback acceptance；验证缺口：Encrypted bundle creation depends on an operator quiescence assertion and does not guarantee a coherent live SQLite snapshot；Only synthetic layout and restore-preview were exercised; no real user data or in-place recovery was tested。
- 证据：docs/upgrade/evidence/ADR-004-protected-downgrade.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-064 · 签名、公证与安装器验收

- 状态：blocked；负责人：Codex；角色：发行；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-005；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：准备证书/签名服务并验证声明支持平台安装包。
- 交付物：签名、公证与安装器验收的版本化交付及验收记录。
- 验收：签名身份与完整性可验证；干净机器安装、启动和卸载记录齐全。
- 验证：各平台签名输出与安装录像。
- 关联审查：F12；风险：需要证书、机器和Beta用户；明确外部阻塞，保留可恢复安装。
- 回退：停止发行或更新渠道；按验证过的安装与数据恢复流程回退。
- 阻塞：waiting code-signing certificate and notarization accounts (external)；下一步：Finish local release readiness gate and installer fixture protocol first; obtain certificate/trust identity and clean test machines for actual signature/install/start/uninstall receipts. No publish implied.。
- 当前复评：local_publication_guard_candidate_native_unverified；结论：Keep blocked until certificate and trust identity, clean-machine install/start/uninstall and rollback receipts are reviewed.；下一步：Obtain external signing/notarization identities and clean test machines; build a frozen candidate through the nonpublishing path, complete native receipts, then make the explicit AZ-069 release decision.。
- 复评剩余：Signing and notarization identities；Frozen signed artifact hashes；Clean-machine install/start/uninstall；Rollback and independent acceptance；验证缺口：GitHub publishing workflow not dispatched；No trusted signature or native installer journey。
- 证据：docs/upgrade/evidence/AZ-064-preflight.md；docs/upgrade/evidence/AZ-064-preflight.md；docs/upgrade/evidence/AZ-064-preflight.md；docs/upgrade/evidence/AZ-064-preflight.md；docs/upgrade/evidence/RC-extension-host.md；docs/upgrade/release-candidate-scope.md；docs/upgrade/evidence/RC-windows-build.log；docs/upgrade/evidence/RC-windows-artifact.json；docs/upgrade/evidence/AZ-064-preflight.md；docs/upgrade/evidence/RC-version-identity.md；docs/upgrade/evidence/AZ-064-preflight.md；docs/upgrade/evidence/RC-0.9.7-windows-artifact.json；docs/upgrade/evidence/RC-source-closure-followup.md；docs/upgrade/evidence/RC-source-provenance-review.md；docs/upgrade/evidence/AZ-064-current-local-build.md；docs/upgrade/evidence/AZ-064-current-artifact-preflight.json；docs/upgrade/evidence/current-release-scope.md；docs/upgrade/evidence/AZ-064-next-local-build.md；docs/upgrade/evidence/AZ-064-next-build-inputs.json；docs/upgrade/evidence/AZ-064-next-installer.json；docs/upgrade/evidence/AZ-064-next-artifact-preflight.json；docs/upgrade/evidence/AZ-064-contract-local-build.md；docs/upgrade/evidence/AZ-064-contract-local-build.md；docs/upgrade/evidence/AZ-064-contract-installer.json；docs/upgrade/evidence/AZ-064-workflows-local-build.md；docs/upgrade/evidence/AZ-064-workflows-local-build.md；docs/upgrade/evidence/AZ-064-workflows-installer.json。
- 完成摘要：新20261009 Windows NSIS候选构建成功且独立归档，源码314项无变化；未签名/安装/回滚/发布；复盘：待填写。

验收人：未分配。

### AZ-065 · 可信更新与失败恢复

- 状态：deferred；负责人：未分配；角色：发行；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-063, AZ-064；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：签名更新、渠道、下载失败、安装失败与数据兼容恢复。
- 交付物：可信更新与失败恢复的版本化交付及验收记录。
- 验收：拒绝坏签名；中断不损坏可运行安装；回滚有数据兼容策略。
- 验证：更新正常/坏签名/断网/失败场景报告。
- 关联审查：F12；风险：需要证书、机器和Beta用户；明确外部阻塞，保留可恢复安装。
- 回退：停止发行或更新渠道；按验证过的安装与数据恢复流程回退。
- 阻塞：No updater exists and the task depends on unfinished AZ-063 and AZ-064; defer implementation, but the downgrade guard must still be enforced before any release.；下一步：After protected backup and trusted signing, add authenticated channel/manifest/update checks plus interrupted download/install recovery and explicit schema compatibility refusal; test locally before service deployment.。
- 当前复评：not_started；结论：deferred_with_reason；下一步：After protected backup and trusted signing, add authenticated channel/manifest/update checks plus interrupted download/install recovery and explicit schema compatibility refusal; test locally before service deployment.。
- 复评剩余：After protected backup and trusted signing, add authenticated channel/manifest/update checks plus interrupted download/install recovery and explicit schema compatibility refusal; test locally before service deployment.；验证缺口：No bad-signature/recovery/compatible-data rollback fixture.。
- 证据：docs/upgrade/evidence/ADR-004-protected-downgrade.md。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-066 · 首次成功任务引导

- 状态：pending；负责人：未分配；角色：前端；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-040, AZ-048, AZ-061；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：模型配置检测、环境诊断、授权解释和首任务样例。
- 交付物：首次成功任务引导的版本化交付及验收记录。
- 验收：干净机器可走完首次任务；缺key/工具错误有可执行下一步。
- 验证：至少5次首次使用走查记录。
- 关联审查：F12；风险：需要证书、机器和Beta用户；明确外部阻塞，保留可恢复安装。
- 回退：停止发行或更新渠道；按验证过的安装与数据恢复流程回退。
- 阻塞：无；下一步：原生跑首次任务旅程；独立复核。
- 当前复评：first_task_project_tool_connection_guidance_local_checks_passed；结论：Keep pending until unified acceptance and clean signed build walkthrough.；下一步：Review first-task UI with real tool/environment failures; run five clean signed-build first-use journeys using an authorized model after upstream dependencies are accepted.。
- 复评剩余：Native tool/environment failure journey；Five clean signed-build first-use journeys；Independent UI review and unified acceptance；验证缺口：No native desktop or model journey run；No clean signed-build first-use walkthrough run。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-067 · 同步支持文档与发布说明

- 状态：pending；负责人：未分配；角色：产品；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-005, AZ-046, AZ-058；候选代码依赖：沿用验收依赖；预计：1d；实际：待记录d。
- 范围：更新README/RELEASE/known-issues与支持矩阵，版本自动一致校验。
- 交付物：同步支持文档与发布说明的版本化交付及验收记录。
- 验收：已实现/已验证/规划分开；旧问题给关闭版本或复现依据。
- 验证：文档链接与版本一致性检查。
- 关联审查：F12；风险：需要证书、机器和Beta用户；明确外部阻塞，保留可恢复安装。
- 回退：停止发行或更新渠道；按验证过的安装与数据恢复流程回退。
- 阻塞：无；下一步：Reconcile the support matrix with actual extension and macOS/Linux journeys, then review all current docs and run the CI version check on the final candidate.。
- 当前复评：local_release_document_sync_candidate_unreviewed；结论：Keep pending until extension/platform support evidence and unified documentation review are complete.；下一步：Reconcile the support matrix with actual extension and macOS/Linux journeys, then review all current docs and run the CI version check on the final candidate.。
- 复评剩余：Extension compatibility evidence；macOS/Linux support evidence；Unified documentation review；验证缺口：CI workflow not dispatched；No installation, signature or cross-platform acceptance。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-068 · 运行 Beta 与支持分流

- 状态：blocked；负责人：未分配；角色：产品；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-056, AZ-058, AZ-061, AZ-065, AZ-066；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：限定Beta用户与反馈窗口，按严重度记录问题和回滚条件。
- 交付物：运行 Beta 与支持分流的版本化交付及验收记录。
- 验收：反馈均有复现/影响/归因；严重丢失与串环境触发停止扩大发行。
- 验证：Beta问题台账与周期复盘。
- 关联审查：F11, F12；风险：需要证书、机器和Beta用户；明确外部阻塞，保留可恢复安装。
- 回退：停止发行或更新渠道；按验证过的安装与数据恢复流程回退。
- 阻塞：human decision: needs real Beta users and support triage；下一步：Prepare severity/attribution/rollback intake templates locally; named owner must select cohort and authorize distribution before actual external feedback.。
- 当前复评：local_beta_intake_prepared；结论：Keep blocked on prerequisite evidence and human cohort/distribution decision.；下一步：After engineering prerequisites, obtain named owner, cohort and window; run actual authorized Beta and record triage outcomes.。
- 复评剩余：Named owner, consented cohort and time window；Prerequisite gate evidence；Actual Beta intake and support response；验证缺口：No external participants or Beta run；No support response evidence。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-069 · 完成稳定版发布决策

- 状态：deferred；负责人：未分配；角色：负责人；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-068, AZ-062, AZ-067；候选代码依赖：沿用验收依赖；预计：0.5d；实际：待记录d。
- 范围：核对M0–M7 gate、候选提交、支持范围和遗留风险。
- 交付物：完成稳定版发布决策的版本化交付及验收记录。
- 验收：无未关闭P0；关键恢复/安全旅程通过；风险接受有姓名与证据。
- 验证：发布决策记录及冻结候选版本。
- 关联审查：F11, F12；风险：需要证书、机器和Beta用户；明确外部阻塞，保留可恢复安装。
- 回退：停止发行或更新渠道；按验证过的安装与数据恢复流程回退。
- 阻塞：It is a governance decision gated by M0-M7 and external signing/Beta evidence that do not exist yet, and there is no code to fix.；下一步：Assemble explicit gate matrix after engineering/backup/signing/Beta dependencies; unresolvedP0/security/recovery gates cannot be accepted by missing evidence.。
- 当前复评：not_started；结论：deferred_with_reason；下一步：Assemble explicit gate matrix after engineering/backup/signing/Beta dependencies; unresolvedP0/security/recovery gates cannot be accepted by missing evidence.。
- 复评剩余：Assemble explicit gate matrix after engineering/backup/signing/Beta dependencies; unresolvedP0/security/recovery gates cannot be accepted by missing evidence.；验证缺口：Stable product/installation/recovery/signing/Beta receipts missing; final user release decision remains external.。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

## M9 · 受控自动化与团队扩展

Gate：pending；前置：M2, M3, M4, M7。

退出条件：团队与自动化受预算和授权约束，收益有单 Agent 对照证据。

### AZ-070 · 成员任务环境与集成契约

- 状态：deferred；负责人：未分配；角色：宿主；优先级：P2。
- 仓库：AgentZ；验收依赖：AZ-008, AZ-024, AZ-038；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：成员环境、输出与集成负责人绑定，明确冲突策略。
- 交付物：成员任务环境与集成契约的版本化交付及验收记录。
- 验收：成员不串目录；汇总不是直接宣布成功；集成可检查。
- 验证：双成员实现与冲突集成案例。
- 关联审查：F01, F09；风险：并发成本和副作用扩大；任务整体预算、去重和灰度开关。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：Not started and blocked by unfinished AZ-024 and AZ-038; the existing swarm path does share one directory across members, so it should be scheduled as a P1 before any team feature is exposed.；下一步：Add member environment and artifact/output provenance contracts under real task binding, explicit integration owner and conflict policy; build concurrent same-file/environment fixtures before team defaults.。
- 当前复评：partial；结论：deferred_with_reason；下一步：Add member environment and artifact/output provenance contracts under real task binding, explicit integration owner and conflict policy; build concurrent same-file/environment fixtures before team defaults.。
- 复评剩余：Add member environment and artifact/output provenance contracts under real task binding, explicit integration owner and conflict policy; build concurrent same-file/environment fixtures before team defaults.；验证缺口：No member-specific workspace isolation/integration artifact acceptance.。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-071 · 团队任务预算与失败传播

- 状态：deferred；负责人：未分配；角色：宿主；优先级：P2。
- 仓库：AgentZ；验收依赖：AZ-070, AZ-024；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：团队共享预算、取消、超时和成员失败归因。
- 交付物：团队任务预算与失败传播的版本化交付及验收记录。
- 验收：预算不按成员复制；取消到全部成员；部分成功明确报告。
- 验证：多成员预算和取消故障案例。
- 关联审查：F09；风险：并发成本和副作用扩大；任务整体预算、去重和灰度开关。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：The budget layer explicitly rejects multi-member usage and the team layer does not exist yet; work should wait for AZ-070 and AZ-024.；下一步：Connect one team-owned durable budget and member reservations, cancellation fanout with settlement acknowledgement and partial-failure attribution; avoid copying limits per member.。
- 当前复评：partial；结论：deferred_with_reason；下一步：Connect one team-owned durable budget and member reservations, cancellation fanout with settlement acknowledgement and partial-failure attribution; avoid copying limits per member.。
- 复评剩余：Connect one team-owned durable budget and member reservations, cancellation fanout with settlement acknowledgement and partial-failure attribution; avoid copying limits per member.；验证缺口：No team total-budget or all-member cancel settlement fixture/actual team cost receipt.。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-072 · 持久化自动化触发与去重

- 状态：pending；负责人：未分配；角色：宿主；优先级：P2。
- 仓库：AgentZ；验收依赖：AZ-019, AZ-025；候选代码依赖：沿用验收依赖；预计：2d；实际：待记录d。
- 范围：定时/事件触发持久化，睡眠/重启补偿与去重策略明确。
- 交付物：持久化自动化触发与去重的版本化交付及验收记录。
- 验收：重启不重复触发危险动作；错过调度有解释；禁用立即生效。
- 验证：重启/睡眠/重复触发案例。
- 关联审查：F09；风险：并发成本和副作用扩大；任务整体预算、去重和灰度开关。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：Extend the verified-scope reminder source to controlled task execution with AZ-025 permits and budgets; exercise real sleep/resume and native disable/click journeys.。
- 当前复评：durable_trigger_claim_domain_candidate_unreviewed；结论：Keep pending until a controlled task execution adapter, AZ-025 permits for external actions, actual sleep/resume and native product acceptance are integrated.；下一步：Extend the verified-scope reminder source to controlled task execution with AZ-025 permits and budgets; exercise real sleep/resume and native disable/click journeys.。
- 复评剩余：Controlled task execution adapter；AZ-025 permit binding for external actions；Actual OS sleep/resume fixture；Native UI and unified review；验证缺口：Only local reminder dispatch, no automated task execution；No actual OS sleep/resume or native journey test；M2/M3/M4/M7 gates unmet。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-073 · 建立通知收件箱与静默策略

- 状态：deferred；负责人：未分配；角色：前端；优先级：P2。
- 仓库：AgentZ；验收依赖：AZ-072, AZ-018；候选代码依赖：沿用验收依赖；预计：1d；实际：待记录d。
- 范围：仅有意义变化/完成/失败/用户动作触发通知并可追踪。
- 交付物：建立通知收件箱与静默策略的版本化交付及验收记录。
- 验收：不变状态不刷屏；点击定位任务；读状态与去重保持。
- 验证：自动化无变化/变化/失败通知案例。
- 关联审查：F10；风险：并发成本和副作用扩大；任务整体预算、去重和灰度开关。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：Inbox, Run/approval notices and local reminder trigger integration exist; native click-to-task/quietness, approval restart reconciliation and full AZ-072 automation remain unaccepted；下一步：Verify native reminder/approval click-to-task, unchanged-state quietness and multiple-read backlog behavior after AZ-072 trigger integration.。
- 当前复评：persisted_inbox_with_rotating_approval_reconciliation_candidate_unreviewed；结论：Keep deferred until AZ-072 durable trigger integration and product inbox wiring are complete.；下一步：Exercise native reminder/approval click-to-task and unchanged-state quietness; verify multiple-read backlog behavior and complete AZ-072 trigger integration.。
- 复评剩余：Native click-to-task and quietness journey；Multiple-read backlog product verification；Full AZ-072 automation trigger integration；验证缺口：No native product-level unchanged-state or navigation fixture；Approval expiry/restart covered by a local fixture only；AZ-072 not accepted。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-074 · 工作流检查点与人工续跑

- 状态：deferred；负责人：未分配；角色：宿主；优先级：P2。
- 仓库：AgentZ；验收依赖：AZ-018, AZ-028, AZ-072；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：工作流节点 checkpoint、审批等待与恢复对齐统一运行模型。
- 交付物：工作流检查点与人工续跑的版本化交付及验收记录。
- 验收：重启后不重跑已完成危险节点；人工回答恢复正确节点。
- 验证：分支/重试/审批/重启案例。
- 关联审查：F09；风险：并发成本和副作用扩大；任务整体预算、去重和灰度开关。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：The legacy workflow runtime has per-node persistence and human resume, but the upgrade checkpoint scope depends on unfinished AZ-018 and AZ-072, and the restart reconciliation and dangerous-node re-execution defects are P1 and need fixing before any done claim.；下一步：Unify node attempt/checkpoint/approval identities and commit action receipts before cursor advance; explicit unknown-result reconciliation, restart no-repeat and exact-human-node fixtures.。
- 当前复评：partial；结论：deferred_with_reason；下一步：Unify node attempt/checkpoint/approval identities and commit action receipts before cursor advance; explicit unknown-result reconciliation, restart no-repeat and exact-human-node fixtures.。
- 复评剩余：Unify node attempt/checkpoint/approval identities and commit action receipts before cursor advance; explicit unknown-result reconciliation, restart no-repeat and exact-human-node fixtures.；验证缺口：No dangerous-node crash-window dedupe or cross-restart approval-binding acceptance.。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-075 · 验证多 Agent 相对收益

- 状态：deferred；负责人：未分配；角色：质量；优先级：P2。
- 仓库：AgentZ；验收依赖：AZ-071, AZ-062；候选代码依赖：沿用验收依赖；预计：1.5d；实际：待记录d。
- 范围：固定任务与配置，对比单Agent/只读并行/实现团队。
- 交付物：验证多 Agent 相对收益的版本化交付及验收记录。
- 验收：记录成功率、耗时、费用与干预；无收益的场景不默认团队。
- 验证：重复对照评测与默认策略建议。
- 关联审查：F11；风险：并发成本和副作用扩大；任务整体预算、去重和灰度开关。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：This is a measurement task with no harness, no data, and pending upstream dependencies, so it cannot be verified or closed until AZ-062 and AZ-071 exist.；下一步：After team scope and budget contracts, freeze a real experiment card and run paired single/read-only-parallel/implementation-team repeats with actual model, cost, intervention, and oracle receipts.。
- 当前复评：matched_comparison_protocol_prepared_unreviewed；结论：Keep deferred until AZ-070/AZ-071 contracts, AZ-062 collection, authorized model runs, and unified review.；下一步：After team scope and budget contracts, freeze a real experiment card and run paired single/read-only-parallel/implementation-team repeats with actual model, cost, intervention, and oracle receipts.。
- 复评剩余：Executable fixture and frozen card；Three-arm authorized actual-model sample set；Paired analysis and independent default-policy review；验证缺口：No model calls or cost receipts；No team-run comparative measurements；Upstream task/environment and team budget contracts not accepted。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

### AZ-076 · 发布自动化与团队受控能力

- 状态：deferred；负责人：未分配；角色：产品；优先级：P2。
- 仓库：AgentZ；验收依赖：AZ-073, AZ-074, AZ-075；候选代码依赖：沿用验收依赖；预计：0.5d；实际：待记录d。
- 范围：按验证结果更新支持范围、默认开关与用量说明。
- 交付物：发布自动化与团队受控能力的版本化交付及验收记录。
- 验收：每项默认启用有证据；未验证平台/场景显示限制；回滚开关可用。
- 验证：能力发布决策与使用说明。
- 关联审查：F12；风险：并发成本和副作用扩大；任务整体预算、去重和灰度开关。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：It is a release-decision task whose inputs (AZ-073 to AZ-075) are all unimplemented, so there is nothing yet to gate or record.；下一步：Define per-capability supported scenarios/defaults and authoritative disable/revoke switch after prerequisites; UI limits must reflect unverified platforms, and defaults require actual benefit/safety receipts.。
- 当前复评：partial；结论：deferred_with_reason；下一步：Define per-capability supported scenarios/defaults and authoritative disable/revoke switch after prerequisites; UI limits must reflect unverified platforms, and defaults require actual benefit/safety receipts.。
- 复评剩余：Define per-capability supported scenarios/defaults and authoritative disable/revoke switch after prerequisites; UI limits must reflect unverified platforms, and defaults require actual benefit/safety receipts.；验证缺口：No controlled-capability release decision or disable-in-flight automation/team fixture.。
- 证据：待附提交/PR/测试日志/截图或报告。
- 完成摘要：待填写；复盘：待填写。

验收人：未分配。

## MB · 真实任务效果基线

Gate：pending；前置：M0。

退出条件：固定12项任务与配置的真实模型结果、成本、人工干预与性能基线有实际证据。

### AZ-004 · 建立成功率成本与性能基线

- 状态：done；负责人：Codex (真实模型基线)；角色：质量；优先级：P1。
- 仓库：AgentZ；验收依赖：AZ-001；候选代码依赖：沿用验收依赖；预计：1.5d；实际：1.2305d。
- 范围：固定模型配置和至少 12 个代表性任务；定义成本与人工干预口径。
- 交付物：建立成功率成本与性能基线的版本化交付及验收记录。
- 验收：每个样例记录结果、环境、时间、成本和未验证项；缺失数据不填零。
- 验证：基线数据 JSON 与采集说明。
- 关联审查：F11；风险：基线与团队容量未知；先确认口径再承诺日期。
- 回退：优先功能开关/回退独立变更；数据schema变化先执行备份恢复检查。
- 阻塞：无；下一步：后续评估完善oracle和代表性；桌面真实旅程与性能另验。
- 当前复评：historical_limited_measurement；结论：preserve historical ledger status; qualify current verification separately；下一步：新版本重采样；真实桌面/复杂仓库任务另验收。
- 复评剩余：新版本重采样；真实桌面/复杂仓库任务另验收；验证缺口：该分项只做源码/历史证据阅读；本轮没有逐项重跑专属验收。。
- 证据：docs/upgrade/evidence/AZ-004-live-design.md；docs/upgrade/evidence/AZ-004-live-execution.md；docs/upgrade/evidence/AZ-004-live-results-v1.json；docs/upgrade/evidence/AZ-004-live-results-review.md。
- 完成摘要：首次固定12项synthetic真实kernel基线×3，36样例/108调用；自动33/36、独立语义复核36/36分列，记录耗时tokens价格估算与未验项，不提升MB/桌面Gate；复盘：独立全36产物及9JS/3JSON检查通过限定要求；固定v1保留oracle三条误判，后续版本修正。actual_days为本轮started_at至完成UTC耗时除8h的追踪代理，非人日/累计历史投入；成本实账未知null。

验收人：Codex (独立模型基线审查)。

