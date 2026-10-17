# AZ019 领域恢复集成补验

首次38615六项夹具在初始化失败，原因是fixture未登记环境；随后仅改为合法direct binding/create_direct，生产校验不变。

实际99087：recovery六项SQLite测试全部通过，0 failed、0 ignored，0.08秒。证明Alive/Unverifiable不写、确证失活原子状态事件、重开无自动retry、双连接仅一成功、事件SQL失败状态回滚、错scope/坏历史/缺库拒绝。不是宿主真实进程失活证明或杀进程重启端到端通过。

宿主自动启动扫描与持久跨进程归属尚未接线；不提升正式AZ019/Gate。
