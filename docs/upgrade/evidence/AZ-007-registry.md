# AZ-007 环境登记实现与验证

基线a787b6d，当前工作树增量实现。新增framework-independent `agentz-task-state` crate；桌面通过薄封装访问独立SQLite登记，数据库schema=1。

实现：task_id唯一绑定environment_id/project/workspace/branch/base ref/base SHA；登记事务幂等；不同绑定拒绝；跨项目记录拒绝；更高schema拒绝读取；创建环境起点使用冻结SHA；新增agent_task_get供任务重开查询。旧未登记worktree不按路径冒认任务，返回显式恢复提示。

验证：`cargo test -p agentz-task-state --locked` 4项通过（持久重开/A-B隔离、幂等与base不漂移、跨项目与未来schema、环境唯一性与损坏记录拒绝）；`npm run typecheck`通过；提取后的`cargo check -p agentz-desktop --locked`通过（2m28s）。

测试说明：曾启动完整desktop lib测试，因需首次编译全部Tauri依赖主动停止；随后将实际登记代码抽到独立crate，并运行其真实SQLite测试，没有将中断的desktop测试算通过。无需额外第三方运行时依赖，锁文件仅增加本地crate。

未验证：真实桌面重开、工作区执行校验（AZ-008）、前端切换路由（AZ-009）。本项不能单独宣称修复全部环境串用问题。

复盘：纯领域代码从桌面框架中隔离，可以低成本运行真实存储测试；API与UI绑定继续按后续任务验收。回退保留task-environments.db及已创建worktree，不删除用户数据。
