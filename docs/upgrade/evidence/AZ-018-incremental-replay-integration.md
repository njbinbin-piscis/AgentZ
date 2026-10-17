# AZ-018 已验证游标增量回放候选

2026-10-08。Codex 子代理独占collector fork与面板，主线程独立复核。同generation完整stream缓存已验证collector/Run；通知从已验证cursor补读，不把通知cursor当作确认。manual/focus/busy重新发现Run，同流复用，换Attempt新建collector。fork深拷贝events/IDs/sequence并重置单批页预算；draft分页、累计唯一事件5000/8MiB预算、投影和高水位全部成功后才swap缓存/UI。失败丢draft，原游标不变，显式重试从原verified cursor开始；不会回灌active聊天文字或执行工具。

增量仍复制/投影完整prefix，前端CPU仍O(N)且有JSON副本；仅数据库读取范围改为新增事件，未测桌面性能提升。同流刷新不重新验证旧payload内容，不宣称完整防篡改；宿主每页仍校验流metadata与权威Run/Attempt。外部破坏需明确恢复，未引入跨快照安全缓存。

独立只读热点审计确认append正常索引下无每笔全流扫描，不能把12.13秒写入直接归因O(N²)。replay每页全流聚合、全Task历史校验及逐row payload读取是可审查热点；下一包先在同事务保留整页metadata预算检查后一次范围取payload，不放宽durability。

实际主线程33679：39前端目标测试通过（collector7/notice7/projection6/panel19），最终typecheck/ESLint在途；代理26目标及typecheck/lint通过。未修改Rust故不重跑宿主。GUI、真实模型、跨平台、安装回滚及正式Gate仍未验收。

主线程33679最终exit0：39项目标测试/typecheck/限定ESLint通过；非桌面性能测量。
