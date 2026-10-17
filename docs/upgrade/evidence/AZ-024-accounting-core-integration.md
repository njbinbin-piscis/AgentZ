# AZ-024 观察式预算核心整体验证

2026-10-08。统一进程52758实际执行 cargo test -p agentz-task-state --offline：97 passed、0 failed、0 ignored，112.69秒；随后 cargo clippy -p agentz-task-state --all-targets --offline -- -D warnings 成功，9.04秒。

新增修复覆盖零usage时损坏Run/Attempt仍拒绝configure与snapshot，以及缺PK/重复历史Attempt的数据库拒绝open与所有预算写读。写入前同事务校验，失败不插入policy、不修复数据库。独立子代理仅源码审查，实际测试由主线程统一运行。

第一阶段交付不可变policy、真实来源与双ID幂等usage、checked汇总、未知计量与币种分离。没有请求执行预留、硬费用限制、LLM宿主接线、成员登记或桌面验收。AZ-024和里程碑保持未验收，既有安装包未包含本批源码。

下一步按AZ-024-reservation-design.md实现持久Reserved/Started/Unknown状态机与原子结算；执行许可与幂等重放分离，崩溃不自动释放未知额度。
