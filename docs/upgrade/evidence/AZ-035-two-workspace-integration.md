# AZ035 双工作区前端集成补验

2026-10-09，CodeZ和本地单任务WorkZ均挂载同TaskDetailsPanel，调用同chat_task_details授权读取服务。团队/pool未挂载，unsupported未伪装支持。

64411实际11项Vitest通过，随后tsc --noEmit exit0。修复focus在途丢刷新P2，合并一次待刷新且旧generation/unmount不写。首帧scope/task变化屏蔽旧数据，失败清旧值；不显示未接通预算的虚假余额、不恢复执行。

之前13360/44194失败已保留：node组件缺window已改真实EventTarget fixture；service beforeEach返回mock函数被Vitest当cleanup再调用，导致正确host rejection仍失败，改block不返回函数后通过；生产host错误传播未改。补验覆盖取消计划保留、迟到请求、错误清空、focus合并、卸载停止读取及未知源。

领域纯聚合5、plan readonly16、宿主4已实跑。此次不是桌面IPC/GUI或跨平台验收，未包含既有安装器，不发布。正式AZ035保持候选。

61531限定四新前端文件ESLint exit0。类型检查64411 exit0。候选源码冻结，真实桌面验收后移。
