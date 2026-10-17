# AZ-024 第一核心候选：观察式持久账务

2026-10-08，仅新增 `crates/agentz-task-state/src/budget.rs` 及7个内嵌真实SQLite夹具。主线程负责最小模块导出及RunStore同事务权威校验helper，本代理不修改run.rs/lib.rs、不运行Cargo或模型。

API为 `BudgetStore::open_existing(project,readonly)`、显式 `initialize()`、`configure(task,Policy)`、`record_usage(Usage)->Snapshot`、`query(task)->Option<Snapshot>`。只打开既有runs.db，READ_ONLY或READ_WRITE均无CREATE，不新建.agentz，不改Run user_version；独立3表budgetnamespace版本1，Immediate首次初始化锁后重查、partial/空metadata/future/错误layout/缺operation unique拒绝，不CREATE IF NOT EXISTS修补。query无namespace返回None，不初始化。

累计键canonical project+rootstableTask，usage同时保存来源capturedBinding/run/attempt/environment。只支持member==rootTask且binding.task==rootTask，拒绝外部成员；跨retry、新Run和环境仍累计同任务。`validate_budget_origin(tx,binding)`由主线程在同一SQLite事务验证现有Run与真实Attempt，包括历史terminalAttempt结算，不使用另库/另连接预检代替权威快照。

Policy第一次配置不可变，精确重放配置幂等，其他revision或值拒绝；上调/重置需要后续显式CASAPI。usageID和operationID均task内unique，同record幂等返回原累计，不同内容报错。usage/累计在单事务写入并校验，checked整数溢出时整笔回滚；不存在用极值截断或丢真实usage的成功路径。

未知input/output/active_ms/cost为Option，snapshot同时给已知小计与unknown记录数；价格未知不是已知免费。已知金额必须有price_source，采用非负micro-units和3位大写currency，按currency分桶不混加。来源由可信调用方传入，只表示记录来源，不保证providerusage/tokenizer/账单真实性。snapshot限额仅是observed_limits_exceeded，不防止执行，**没有hard费用上限、网络permit、reservation、concurrency槽或崩溃Started对账实现**；这些保留下一原子包，也没有自动启用默认收费限制。

7个夹具覆盖：跨retry/历史Attempt迟到/新Run换环境累计与重开；双ID幂等冲突/跨域/成员拒绝/immutablepolicy；unknown字段与不同币种已知小计；负数和overflow回滚；missing库及namespace只读不create/future不repairbytes不变；坏record/key、空metadata、partialnamespace拒绝不修复；双初始化与双usage写入原子去重。

rustfmt及限定git diff --check退出码0。夹具未由本代理运行；主线程统一编译/回归与独立负面复核后追加真实结果。当前文件依赖主线程提供run::validate_budget_store及validate_budget_origin接口，尚不能用静态检查冒称完整验收。
