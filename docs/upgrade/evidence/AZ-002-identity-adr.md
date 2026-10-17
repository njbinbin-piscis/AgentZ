# AZ-002 · ADR-001：任务与执行环境身份契约

基线a787b6d，2026-10-04；状态：实施契约。现有session数据库继续作为交互记录，不重写共享内核。

Project是canonical根路径标识；Task首期使用WorkZ预生成的session UUID，避免前端另生成不关联的worktree id。Environment由宿主登记，包含稳定id、task_id、project根、运行根、local/managed-worktree类型、分支、创建时base ref与base SHA。相对路径只用于显示，不是执行权威。

一项Task只有一个活动Environment；后续交接必须显式变更登记，不能通过传入workspace_dir覆盖。一个Run属于一个Task与Environment；Attempt是该Run的一次执行尝试，后续M2持久化。交付与运行状态分开，Artifact引用环境与版本而非当前全局目录。

首期环境记录放在主项目`.agentz/task-environments.db`，schema version=1，独立SQLite库，以task_id唯一约束，不修改piscis-engine会话schema。一次登记事务写入，重复请求返回原记录；项目根canonical化并核对。环境失效返回错误，不重新创建或回退到主目录。

命令：创建/绑定task环境；按task读取环境；执行前解析与验证。新WorkZ任务创建使用同一个session UUID。历史任务无法证明原绑定时不猜测；保留历史，在需要隔离续跑时明确提示重新绑定，未来迁移工具处理。

CodeZ原有显式本地workspace与自由目录路径暂保留；新登记WorkZ任务按后端记录运行。对managed-worktree校验git worktree登记、branch、canonical根；普通local环境不宣称沙箱。

生命周期：创建环境后开始回合→每轮重新验证→视图切换只读取→环境丢失阻塞→归档先快照→恢复到同一任务关联。M1负责环境、变更、归档；M2负责运行恢复；M4负责CodeZ交接。

迁移回退：新增独立库与向后兼容的可选IPC参数；旧环境记录不能被新数据覆盖。回退代码前保留库，禁止删除worktree或用户目录。

验收：A/B记录隔离、持久重开、重复创建、跨项目读、失效环境拒绝、base SHA不随分支推进漂移。ADR自检通过；后端与桌面实测分任务执行。
