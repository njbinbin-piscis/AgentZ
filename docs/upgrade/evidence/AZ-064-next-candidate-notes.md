# 下一Windows 0.9.7本地工程候选说明

2026-10-09。适用于本轮冻结源码、输入资源和构建会话66034。当前状态：前端与extension-host生产资源已生成；offline/locked、unsigned NSIS构建正在进行，尚无本轮新安装器完成结果。本文件不是发布声明，不授权安装、外发、tag、push、签名或公开发行。构建失败时保持失败，不引用旧安装器冒充新结果。

## 本轮冻结范围

候选目标是Windows x64、本地单Agent任务的正确性与可见性。版本号仍0.9.7；不能仅以版本号辨认新旧候选，必须使用独立目录、产物SHA和source snapshot。

- 已有本地任务环境登记与后端绑定、完整变更评审/可恢复归档/Undo、权限意图及Windows受限执行候选沿用；不扩大为所有远程、团队或桌面能力已支持。
- 本地执行状态、耐久事件游标、被动历史预览、任务绑定计划与显式计划替换/取消沿用。普通续聊保持计划；读取不恢复模型或工具执行。
- AZ023新增严格错误分类和有限主调用重试/退避：身份或配额错误不受混合transient正文诱导；主调用共享有界窗口；取消可打断等待。辅助模型/客户端内部兼容重发不是完整总费用预算。
- AZ035新增CodeZ和本地单任务WorkZ共享任务概况：同opaque项目scope与existing session读取权威环境、当前Run、完整计划和观察账。缺失/未知/未支持分别展示，取消计划保留完整items；晚任务回包被拒，焦点在途刷新合并一次补读。来源分别读取，明确不是原子快照。events暂Unknown，artifacts暂Unsupported。
- AZ021新增Attempt作用域取消完成屏障：统一最终化取消快照，工具调用future丢弃不算已确认结束；本地sandbox shell/code_run另持worker receipt直到清理返回。未确认/超时/失败不能标记已取消完成，而是Interrupted。正常工具返回只证明调用结束，不能证明其任意detached外部资源终止。
- AZ019包含显式领域对账候选：Alive/Unverifiable保持原状态，可信ConfirmedGone前提下状态与事件原子提交。没有自动启动扫描、持久跨进程owner证明或自动重跑。
- AZ024包含观察账与独立reservation存储核心候选；跨Attempt累计、未知保留、单次启动CAS与结算事务供后续宿主接线。未接全LLM/wire/辅助任务/成员请求，不能宣传任务硬费用、时长或并发上限已执行，也不能展示虚构余额。

## 已有工程验证与本轮构建证据

- source snapshot为AZ-064-next-source-snapshot.json，captured_at2026-10-09T07:50:10.857Z，package0.9.7、included_source_count314、stable_during_capture=true。HEAD仍a787b6d…，工作区dirty，明确reproducibility=not-established-dirty-source-is-not-head；该快照不证明此前安装器源码。输入资源身份另见AZ-064-next-build-inputs.json。
- 前端53861实际tsc/Vite生产构建exit0，4146模块，Vite5m37s；extension-host生产构建exit0。现有大chunk与静态/动态混合导入警告保留，不当作首屏性能验收通过。
- AZ035实际核心聚合5、plan16、宿主4及前端11/TypeScript/lint通过，见AZ-035-two-workspace-integration.md及host evidence；包含真实fixture失败与修复记录，不删失败历史。
- AZ021宿主94796/39522实际3项、kernelregistry4及通用invocation2通过，独立AZ-021-final-review.md限定local单Agent无阻断。尚未跑本轮真实GUI Stop/Windows后代pipe/ACL teardown旅程。
- AZ023实际loop43、classifier15与loopback真实OpenAiClient四项矩阵通过，见AZ-023-retry-integration.md；这是本地假HTTP端点，不是真实provider验收。
- 本轮66034 NSIS结果待主线程落日志、独立归档、产物大小/SHA及offline preflight。无结果不填新hash、不声称新包完成。测试成功不自动提升正式Gate或路线图done。

## 旧包与新包身份

此前完成的工程候选保留独立目录：target/release/candidate-0.9.7-20261008/AgentZ_0.9.7_x64-setup.exe，98,648,395 bytes，SHA256623ed5710ec0ea954c63976aa9c323564bb5ed06ad7098a679387c6d19e3ad11。它不包含后续AZ023、AZ024、AZ035和本轮AZ021更新，不能用该旧hash或旧预检为新功能背书。新包必须待构建成功后另存并记录实际hash；当前尚未填写。

## 支持与已知缺口

仅限定Windows x64、本地单Agent候选。Team/pool、remote/SSH全部工具资源终止、后台桌面/浏览器/连接器、provider billing、跨进程boot归属不在本次取消确认保证内；正常返回后隐藏detached资源也不被receipt证明。外层IPC/进程退出可使Run标Interrupted，但不能从父进程锁释放猜所有子进程已经停止。未知预算保持未知，不因Interrupted释放不明风险。

大事件历史与payload有显式预算，超出时报错而非无限读取或猜测缺失内容；事件回放只显示历史，不重新授权、不继续模型、不重复工具动作。远程generation候选、扩展兼容、Webview、LSP等各自原验收未被此包替代。

未验证：原生GUI核心旅程、真实模型效果/成本、干净机器安装卸载、已有数据升级/回滚、签名、macOS/Linux、公证、可信更新、对外分发。unsigned候选不能称正式稳定版。供应链/许可证与公共notice未齐备项沿RC-source-provenance-review.md记录，不能自行补造授权或因本地源码存在就批准公开分发。

## 数据与升级注意

代码涉及独立task-environments.db、runs.db事件/预算namespace、plans.db与conversation数据；各库没有统一事务或通用自动回滚。被动读取使用existing readonly opener，不创建或修复坏库；未知schema/损坏记录显式拒绝。写namespace初始化不等于旧数据完整迁移或真实恢复已验收。

实际使用新候选前应在应用退出、后台任务停止后保留完整项目.agentz、全局配置/会话与必要工作目录备份，不只拷贝单个SQLite主文件；WAL/SHM及完整一致性应由正式备份策略处理。当前没有经过本轮安装回滚测试的降级承诺，不建议把新旧相同版本号解释为数据兼容证明。正式迁移/备份/恢复验收继续归AZ063及发行Gate，不能以本文替代。

本地构建完成更新：66034 exit0，新包target/release/candidate-0.9.7-20261009/AgentZ_0.9.7_x64-setup.exe；98,687,336bytes；SHA256 d0844c488dbfaedced4aaa1019774293807903c2317f67f9438a8712d1a3395e；NotSigned。之前在途说明由此结果更新，release_ready仍false；未安装或公开分发。
