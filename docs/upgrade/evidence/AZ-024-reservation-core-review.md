# AZ-024 原子预留核心独立审查

2026-10-09。只读审查 `budget_reservation.rs` SHA256 `e4e14afa6ac662308a2fe32792bb0faff1c60b2ca3567994bec38ac6a440998b` 及既有observer/Run事务helper。主线程运行专项55073，本代理未运行Cargo或提供商/GUI。未修改生产源码/台账。

## 必修 P2：已有 observer operation 可被 reserve 写成矛盾记录

`reserve`（531起）读取已有reservation后，仅汇总observer Snapshot并check_budget，新INSERT没有检查 `resource_budget_usage(task_id,operation_id)` 已占用。`records`（274起）要求observer usage_id与reservation.usage_id完全一致，Reserved的usage_id必须None。

复现fixture：先合法BudgetStore.record_usage一条operation='op'、known token1；再以有充足余额的reservation policy reserve同operation='op'、bound1。当前路径可以返回Created并commit；随后query/start/settle都在records遇到Some(observerusageid)!=None而失败。原本正常账本被合法API写成矛盾状态，不能靠下一步校验补救。

建议新reservation INSERT之前在同Immediate事务拒绝已有observer operation；不自动拼接、释放或修改历史。新增真实SQLite fixture证明reserve拒绝、reservation行数保持0、observer账与query仍正常且旧usage不改。同时已有reservation的observer旁路record_usage仍可能造成矛盾，当前不宣称宿主硬预算；应明确错误而不自动repair。

## 限定成立的核心路径

- 独立三表schema1：恰好version1、integer storage、精确columns/PK/NOT NULL；partial/空metadata/未来version拒绝。依赖observer namespace及Run约束，不改Run或observer schema版本。readonly open无CREATE，query不初始化。
- reserve/start/settle均Immediate锁；余额使用observer已知值+所有Reserved/Started/Unknown预留；未知限定维度拒绝，混currency拒绝，checked加法。slots与余额在同事务检查。
- reserve精确重放返回Replay而不新授权；start核captured Binding/claim digest/boot，只有Reserved状态commit后返回非Clone StartedPermit。重复Started返回Replay。多连接操作由SQLite写锁线性化，预算许可不授OS/tool能力。
- reserve/start要求权威current Running/not_ready Attempt；settle与mark_unknown允许真实历史Attempt来源，归属/所有history仍验证，不使旧Attempt重新执行。
- Unknown和跨boot旧Started仍占额度/slot并阻后续；Started不能release_unstarted。无TTL自动释放。query的unknown_operations只计显式Unknown，旧bootStarted可从outstanding.boot_id识别，尚无宿主恢复UI。
- settle在同事务写observerusage及reservation Settled；未知被限制维度不能释放，实际超预留如实记账并reason flag阻未来reserve，records重算overshoot及capturedidentity。精确重复usage幂等，冲突拒绝；原子性仍需故障注入fixture证明。

## 当前真实fixture覆盖与缺口

源码7fixture覆盖reserve重放/start并发单Granted、两个连接争额度与slot（一个组合案例）、历史settle/retry累计、旧boot/Unknown保留、超预留和无价拒绝、只读/坏schema不repair、settledidentity/overshootflag篡改拒绝。实跑结果由主线程记录，不冒称本代理测试通过。

除上述must-fix，建议补：observer operation冲突及usageID/operationID各自指向不同旧账的双ID冲突；触发器或SQL失败发生在observerINSERT后/reservationUPDATE时的全rollback；只tokens限制/宽slot与只slot/宽tokens分别竞争；init双连接、缺PK/NOT NULL/部分namespace；start之前observer消费增加导致余额不足而仍Reserved。这些是当前fixture覆盖缺口，不等于已发现另一个源码错误。

## 产品限制

本包没有LLM adapter/全部wire请求接线；observer record_usage独立入口仍能旁路reservation限制，因此不能宣称宿主硬费用/请求上限。可信price upper bound由caller提供但无本包模型费率验证；时长只是预留账务，未实现实时取消；成员仅self，完整成员/remote/模型/GUI/崩溃真实进程验收未完成。

结论：修复observer operation冲突后再重跑并复核；当前不予无保留候选通过，也不标记AZ024验收完成。
