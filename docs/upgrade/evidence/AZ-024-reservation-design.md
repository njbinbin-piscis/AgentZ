# AZ-024 第二核心候选：预留与单次执行许可设计

2026-10-08，只读基于当前观察式budget.rs与RunStore同事务权威helper；不修改已冻结源码、台账，不运行Cargo、模型或网络。第一阶段仍是usage累计和observed excess，本设计不追认它已经执行硬预算。

## 持久状态机与身份

保留总账键canonical project+rootstableTask。现阶段只允许root=self/member=self，不猜团队enrollment。每一**实际计量逻辑操作**有稳定operation_id；每一最终结算有稳定usage_id。operation_id不能使用重复model名、当前session累计数或每次函数重试新UUID来绕过原预留。明确host分配并捕获task/run/attempt/environment、boot_id、policy revision、kind与不可变intent digest；这些是来源维度，不是总预算reset键。

状态：`Reserved -> Started -> Settled`；`Reserved -> ReleasedUnstarted`；`Started -> Unknown -> SettledAfterReconciliation`。Unknown也可在显式可信对账证明“未发送/未计费”后终结，需独立审计事实与CAS，不能叫普通release。进程崩溃只读发现Started/Unknown不会自动修补；新预留看到未结算Started或Unknown必须failclosed，哪怕新Run/newAttempt。Reserved可显式未执行释放，不按TTL自动释放，不由队列取消推断Started没执行。

**幂等返回状态与执行许可必须分离。** `reserve`相同operation/intent返回Replay，不再次扣账，也不授权第二次网络发送。首次Reserved携带不可复用claim凭证；`start`在真正client调用前以Immediate CAS消耗claim并提交Started，只有首次成功的 `StartOutcome::Granted(StartedPermit)` 能发送。相同start重放只能返回AlreadyStarted，没有permit。写Started后进程崩溃、哪怕wire尚未发送，也是保守Unknown，不能恢复后再发送。permit仅预算许可，不能取代approval或OS权限。

禁止Clone的Rust类型可防普通误用，但不是持久幂等证明；必须有数据库状态CAS、可信宿主boot/nonce以及真实调用点接线。过期CapturedAttempt不能发起reserve或start，usage结算则允许原历史terminalAttempt真实归属，保证retry不丢账。

## 建议API与事务

```rust
initialize_phase2(expected_namespace_version: i64 /*必须1*/) -> Result<(), Error>
configure_reservation_policy(task, expected_usage_policy_revision, ReservationPolicy) -> Result<(), Error>
reserve(binding, root_task, member, operation_id, Intent, expected_revisions) -> Result<ReserveOutcome, Denial>
start(binding, operation_id, claim) -> Result<StartOutcome, Error>
release_unstarted(captured_origin, operation_id, expected_reserved_version, reason) -> Result<ReleaseOutcome, Error>
mark_unknown(captured_origin, operation_id, failure_facts) -> Result<(), Error>
settle(captured_origin, operation_id, usage_id, Measurement) -> Result<Snapshot, Error>
query(task) -> Result<Option<ReservationSnapshot>, Error>
```

ReservationPolicy独立带revision和显式mode。第一阶段immutable Policy的已有token/time/currency上限不被重写；新增max_in_flight与启用哪些维度需可信宿主显式配置。默认不收费enforce。若第一阶段已有unknown usage/price，启用对应严格维度前拒绝，须对账或明确保持observe，不能把迁移时旧未知历史当0。

所有写入为同runs.db Immediate事务：验证namespace/严格Run表结构、完整task历史、self成员、policy版本；reserve/start进一步要求captured=current且Running/NotReady；历史origin helper只能用于settle/release对账，不当新执行授权。余额用checked整数：`settledKnown + outstandingReserved + proposed <= configuredLimit`，currency固定与price snapshot匹配，in-flight persistent count同原子检查。候选第一实现可从逐条validated records求和，暂不增加容易失配的counter缓存；性能另验。

start不再增加预留或槽；ReleasedUnstarted才释放对应未执行项；Settled写usage、验证双ID幂等、累计实际并释放同一reservation，在一个事务commit。`settle`同usageID/operationID内容完全一致返回原结果；不同内容/币种/来源冲突拒绝，不重复累加。actual大于reservation仍如实记账、标overshoot并阻止后续操作；不得把超额usage拒写成没消费。policy更新或retry不清usage/reservation历史。

失败usage未知时不能用0调用settle：保留token/金额/槽预留，记录Unknown与failure source。若只知道token而cost未知，允许保存部分观测，但不释放相应未知cost风险或把整体reservation标完全Settled；最小包可先要求所有启用维度完整后才settle，部分观测单独事实记录。它不声称强制杀死工具或撤销已有消费。

## schema升级与旧版本兼容

建议显式将budget namespace从1升级至2，同时增加reservation、reservation policy与审计事实表；Run user_version仍1。仅 `initialize_phase2(expected=1)` 可执行，Immediate锁后重读版本、验证完整v1布局与全部历史账，然后创建新表并最后更新budget版本，同一事务回滚保证不出现partial升级。不使用CREATE IF NOT EXISTS修复缺表，不自动迁移、清账、释放风险或初始化缺失runs.db。

新query支持完整v1的观察式snapshot与完整v2的reservation snapshot，返回显式schema/mode，仍READ_ONLY且不创建。旧phase1写入器遇namespace2应拒绝，不能继续绕过reservation写所谓严格账。新版本 `record_usage`也不能绕过启用的reservation策略：v2启用任务必须走settle；历史v1记录保留只读原样。若需要继续v2 observe-only追加，必须是明确独立路径，不能被视为硬执行覆盖。未支持的旧二进制回退不能直接写v2库，升级前一致性备份及实际恢复演练仍须另验。

v2验证需确切列/type/NOTNULL/主键/独立task-operation及task-usage唯一性、版本单行，以及record列与JSON身份/状态一致。namespace部分存在、坏revision、负数/real型预算、记录缺origin或reservation、Unknown漏预留、Settled缺usage和Released仍计槽等均拒绝，不自动repair。migration并发者锁后见v2可精确验证并幂等返回，见future版本拒绝。

## 价格、计量与执行覆盖限制

未知价格不能承诺硬金额上限。严格费用预留需要可信price snapshot及可信资源上界；缺price或image/token上界时明确Denied::PriceUnknown/InputBoundUnknown，observe模式也显示Unknown。不创建默认0价格。provider缺usage、stream中断、cancel、客户端内部reasoning重发均不能通过本接口自动免费结算。真正wire/auxiliary/member覆盖由后续adapter处理；仅预留逻辑主请求不等于整个Run费用有界。

active_ms预留不等同墙钟deadline或硬实时停止。若需要时间enforce，adapter必须消费剩余duration并竞争取消，实际超时仍结算。多成员累计active时间与wall clock不同，时钟回拨/崩溃时长未知保持风险；先只支持可校验整数/未知语义，不宣称已有完整实时限制。

## 下一原子包的真实fixture

1. 两连接同时reserve最后余额/并发槽仅一个成功；等于上限允许、+1拒绝，整数overflow拒绝不写。
2. operation重放只Replay，intent/来源/revision冲突拒绝；start首次Granted、重放AlreadyStarted，没有第二个permit；更新Attempt后旧Reserved不能start。
3. Reserved队列取消显式release成功，Started/Unknown普通release拒绝；drop/reopen、新boot/newAttempt保留Unknown预留和槽，不能TTL清零。
4. settle精确幂等一次，旧terminalAttempt真实usage入账，跨域拒绝；actual超预留如实入账及overshoot阻止新reserve。
5. 缺usage/price或部分usage不把Unknown改0，已知currency不能混合；严格费用缺price/inputbound拒绝且不执行。
6. v1完整升级保留所有Policy/Usage字节与累计；并发显式升级安全；缺namespace/future/partial/corrupt无修复且bytes不变；readonlyquery不触发upgrade。
7. Commit-before-start故障注入、start后崩溃与settle commit失败，均无虚构许可或成功；实际provider与GUI资源验收另列。

核心通过后才能接宿主真实client wrapper，且须禁用或绑定尚无captured identity的post-turn记忆/skill/title及delegate/call_fish旁路。该文档不放行任何默认收费限制或正式Gate。
