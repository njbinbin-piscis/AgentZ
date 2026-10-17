# AZ-045 远程工具连接绑定候选

2026-10-07，Codex 子代理在用户授权并行后实施；仅修改 remote_fs.rs 与本证据，不修改台账或启动 Cargo。

六个远程替换工具在注册时统一包裹 BoundTool。每次调用入口通过 remote::bind 选择工作区 authority 和连接 generation，再在 remote::with_session task-local 范围内执行完整工具调用。file_edit 的读和写、file_list 的路径解析和 exec、搜索与 shell 等复合步骤由同一连接世代承载；具体失败关闭由主线程新增的 RemoteSession / scoped remote::call 实现提供。包装器透传工具名称、描述、最小描述、schema、最小 schema、只读和审批属性。

远程 URI 输入原先直接丢弃 authority，仅保留 path，因此 host-b URI 会在当前 host-a 执行。现在 join 比较输入 URI 与任务工作区 authority，不同 authority 或畸形远程 URI 明确拒绝。同 authority 比较与现有连接管理一致忽略 ASCII 大小写；相对及绝对 POSIX 路径保留现有语义，不新增根目录授权策略。

新增 tool_binding_tests 两个边界用例：不同/同 authority、畸形 URI，以及普通远程路径。尚未运行，等待主线程统一构建。没有真实 SSH、断网、主机密钥、GUI 或跨平台验收；AZ-045 保持候选实现，不计入完成。
