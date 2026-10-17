# AZ-018 同事务两阶段分页候选

2026-10-08。仅events.rs更改，生产写入/pragma/迁移未改。第一查询收集整页metadata、SQLite storage type、连续sequence、identity/time与payload字节长度，并在加载JSON前拒绝超过4MiB页预算；第二查询在同一事务快照一次有界sequence范围读取payload，逐条比对已验证metadata/类型/长度且拒绝缺失或多余行。保留原全流聚合、版本/布局/cursor和腐坏校验，不跨快照缓存。

新增2回归：整页超预算优先于第一行坏JSON，缩页明确JSON错误/修复成功；BLOB payload和real created_ms storage class拒绝/修复成功。主线程已独立读两阶段查询及ownership/drop顺序，未宣称性能提升。统一73190 full task-state/strictClippy、56385宿主同1040真实commit性能夹具正在执行，结果未收取。

审计记录更正：上轮前端source manifest刷新与子代理events.rs首次写入存在竞态，指纹更新不等于该宿主改动已复核/测试；此次才完成源审查，测试仍在途。前端39测试结果不覆盖该Rust变更，正式任务/Gates从未提升。最终候选必须以本包实际测试与再冻结指纹为依据。

GUI、真实模型、跨平台及安装/回滚未验收。单次本地性能对比仅fixture观测，不能作SLA结论。

主线程73190最终exit0：full task-state88项全部通过，strict all-target Clippy通过。主线程56385最终exit0：同opt-in夹具1040真实agent独立commit、1042总events/6页完整比对通过；write11704.960ms，readonly reopen/replay99.696ms，DB40960→552960bytes，Windows x86_64。旧夹具单样本replay304.748ms，本次99.696ms是本地观测，样本/缓存/机器负载不受控，不能推出p95或桌面SLA；write生产策略不变。源码最终已独立复核及验证，覆盖此前指纹写入竞态的不完整证据。下一阶段冻结AZ018候选并做稳定范围收尾，不继续无界性能扩展。
