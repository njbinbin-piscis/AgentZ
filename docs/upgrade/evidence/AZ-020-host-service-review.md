# AZ-020 宿主服务独立只读复核

范围：`src-tauri/src/runtime/durable_plan.rs` 与真实 PlanStore 接口。主线程提供 session 80882 exit 0、6 个测试通过；本代理阅读全部实现和夹具，不运行构建、不修改产品。不等同 tool 注册、Tauri 命令、GUI 或模型验收。

结论：当前服务核心未发现需要阻止候选接线的 P1；在生产使用真实 PlanStore、可信 fresh resolver 和宿主 lease 的前提下，提交/投影次序及旧回调 CAS 边界正确。以下前提不能由这 6 个夹具替代。

## 已确认边界

- recover 从 repository.current 读取权威记录，捕获稳定 ID/revision；不从旧内存恢复。write fresh() 的结果必须等于捕获 PlanBinding，且当前库 ID/revision 必须仍等于 captured。 fresh read 不推进 token，因此旧 Run 不能采用新 revision；库 CAS 再防 read→write 间的并发更新。
- PlanStore current/create/update 直接委托已测试的 continue_plan/create/update，没有不同参数语义。create/update 返回前 SQLite 已 commit；随后才更新 service token/projection，失败早退保持原值。工具适配层仍必须在服务成功后更新 kernel cache/emit。
- cancelled 条目映射无损；unknown 拒绝，normalized 后复用内核 validate_todos，多个 in_progress 拒绝。Completed 计划可显式 update 添加开放条目；整个 Cancelled/Replaced write 拒绝。closed projection 返回空仅用于运行上下文，不删除持久历史，不表示新目标已获准。
- managed/direct 数据校验由 PlanStore 完成：canonical project、登记 env、direct 指纹匹配；已有 plan 的 binding_version 变化拒绝。服务不授权工具，也不处理 Run 取消。

## 6 个夹具提供的实际范围

1. 真实 SQLite reopen、旧 revision 写拒绝、触发器拒写后投影/token及库不变。
2. 真实 SQLite 多 in_progress 在 recovery 拒绝，Fake 伪造不同绑定拒绝。
3. cancelled 映射与 unknown 拒绝。
4. Fake 提交失败及不同 fresh binding，无投影/写入推进。
5. Fake 两 session 旧 token拒绝，新恢复读取最新库版本。
6. Fake cancelled 计划不能经工具重开，投影为空但库保留。

Fake repository 不严格模拟真实 status 推导/形状校验；不能引用其通过来宣称所有损坏记录都已覆盖。真实 PlanStore 的对应 corruption/mixed cancelled/schema 测试是另一个验证集。

## 接线前必要约束与后续夹具

fresh() 目前是同步、调用方注入的 closure。真实 `resolve_binding` 是 async：工具必须先 await可信 resolver，在仍持有环境/执行 lease且每个session写入串行化的范围内，将新结果传入 closure；不能把 turn 初始 binding 简单 clone 当作 fresh。跨库环境快照不能事务锁定，避免在 resolve→write 期间 handoff 的措施仍由宿主 lease 提供。

现有 reopen 是成功 write 返回后的恢复，未实测进程在 SQLite commit 后、cache/emit 前崩溃。结构上服务恢复仅从库读，符合 crash-after-commit恢复预期；建议加真实 PlanStore直接commit、不调用service投影、销毁进程内cache后 recover 的故障注入夹具。不能称真实 crash 验收通过。

尚缺真实 registered managed env fixture、fresh resolver错误（而非不同成功绑定）、Cancelled/Completed真实SQLite→service恢复及写入夹具、两线程/连接同时在read后竞争写的服务整合夹具。最优先是工具层 recorder 验证 commit fail 不 cache、不 emit；其次 cancelled persisted history 与 query DTO不混淆空运行投影。

PlanRepository 是内部可信实现的抽象，不应允许 renderer/model自定义；service未重新校验 repository写入返回的全部Plan，真实PlanStore保证返回契约。若未来增加别的repository，须独立验证返回绑定、ID/revision及status或增强服务防御。当前不要扩大为跨库原子或权限保证。
