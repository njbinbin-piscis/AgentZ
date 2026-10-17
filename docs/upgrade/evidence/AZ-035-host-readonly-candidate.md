# AZ-035 宿主可信绑定只读服务候选

2026-10-09。独占新增runtime/task_details.rs，SHA256 `f1dd7e7c84f2dbda08276278628a5a25ce8f88e89269c9e303e2f483b4a22c42`；单文件rustfmt exit0。未改mod/lib/commands/台账，未运行Cargo/GUI/模型，由主线程接线及统一编译。

API `read(project:&Path,binding:&PlanBinding)->Result<TaskDetails,String>`。调用者必须已取得可信project scope、fresh binding及贯穿environment lease；函数不授权renderer根路径，不复用current workspace隐式身份。

canonicalproject、bindingversion、task及登记environment核对；未登记仅允许direct:bindingversion，managed缺登记明确Err。环境使用load_existing_readonly，Run/Plan/Budget均existing READ_ONLY，不初始化namespace。文件try_exists的I/O错误与所有schema/腐坏/身份错误原样Err，只有真正文件/record缺失为Missing，不猜empty/zero。Cancelled plan用continue_plan完整记录，独立来源版本分开，atomic_snapshot恒false。eventref Unknown（不遍历整history猜游标），artifact Unsupported，不宣称完整产物接线。observer预算unknown计量保留，不授权收费。

四个fixture源码：缺direct各源不建.agentz；真实Run+Cancelled完整Plan读取bytes不变；负Attempt时间错误非Missing且不repair；跨project与未登记managed拒绝。使用create_direct真实合法Run身份，避免未登记managed fixture初始化错误。fixture未实跑。

未完成原生IPCscope注册、异步后scope再验证、CodeZ/WorkZ双视图、跨库一致性处理、GUI/模型验收。相同来源先后读取可能变化，host/renderer须按version与non-atomic标志理解；不把聚合DTO当执行snapshot。
