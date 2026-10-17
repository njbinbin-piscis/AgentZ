# AZ-018 通知预算一致性与SQLite吞吐夹具

2026-10-08，Codex 子代理独占通知预算/测试与Rust opt-in夹具，主线程独立源码复核并执行验证。通知payload严格UTF8 JSON 1MiB，信封额外4KiB有界元数据；identity为256UTF8字节。初始未知stream缓存前先校验schema/task/identity/sequence，避免原1MiB信封限制拒绝合法边界payload。修正初始busy刷新覆盖错误的路径。边界测试覆盖精确1MiB多字节payload、+1byte拒绝、256/257byte identity。

新增 ignored 真SQLite夹具measure_thousand_delta_commits_and_reopened_paged_replay：1000小delta+20工具start/end=1040独立commit，drop writer后READONLY reopen，200条分页校验连续sequence及Event完整相等。耗时含真实提交/重开/校验，DB bytes是本地fixture数值，非桌面/model SLA。没有更改生产写入路径。

主线程56330已23项目标测试通过，最终typecheck/lint在途；代理相同23测试/typecheck/lint通过。主线程74556宿主编译与opt-in性能测试在途，尚无测量结果，不宣称吞吐达标。原生GUI、真实模型、跨平台与安装回滚未执行，正式状态和Gate不提升。

主线程74556最终exit0：opt-in真实SQLite夹具通过，1040 agent独立提交/1042总事件，write_elapsed_ms=12130.702，readonly重开与连续分页校验304.748ms，6页×最多200条，DB40960→552960bytes，Windows x86_64。约85.7 agent事件/秒只描述此次小payload单次本地样本，非真实模型/GUI吞吐SLA或p95。该结果说明全历史200ms反复重读有开销，下一候选优先同流已验证cursor增量回放，并审计每笔完整性扫描热点。

主线程56330最终exit0：23目标测试、typecheck、限定ESLint通过。生产提交策略没有放宽，原生/模型验收不变。
