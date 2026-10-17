# AZ-035 纯任务详情聚合独立复核

2026-10-09。独立只读审查task_details.rs SHA256 `e3c19f9f5e0ab7f45ca9bb24811f70b0ef1dcaac45215c2e1921be77d5826b21`及候选文档。主线程32039执行专项/Clippy在途；本代理不运行Cargo、不改源码或台账。

## 限定结论

未见纯聚合范围内必须修复的P1/P2。该API只聚合由可信host/store已验证的数据，不读取库、不解析renderer路径、不授权或执行。不是宿主权限验证器、跨库原子快照或完整Task详情产品闭环；AZ035正式验收保持未完成。

- Available environment匹配project/task/env；Run匹配task/env；PlanBinding精确全字段；event要求Available Run且完整stream task/env/run/currentAttempt一致、sequence非负。oldAttempt cursor不能被混为currentRun详情。
- Cancelled Plan保留完整objective/items/status/revision，未以空projection掩盖持久内容。
- Source Available(value,version)、Missing、Unsupported、Unavailable、Stale、Unknown各自不同；纯函数无catch吞错误或把坏库当Missing。未来host必须将真正not-found与I/O/schema/授权错误区分，不能所有Err转Missing。
- atomic_snapshot恒false；每来源version单独提供，不保证读取同时性/新鲜。event/run交叉来源发生变化时应重查或Stale，不以聚合成功冒称同事务。
- budget仅Snapshot.root_task校验，因为Snapshot没有project/env字段；未来host必须从capturedcanonicalproject的权威库查询，不能用其他project相同task数据作为Available。unknown计数/币种原样保留，不补0、不执行限额。
- artifact opaque IDs限定task/env，最多200、每ID256、无空/control、拒重复；内容/路径的真正授权仍由host实现，本函数本身不是完整脱敏保证。

## 五fixture复核及缺口

源码五fixture覆盖状态JSON序列化和非原子标志、Cancelled完整items、Run/Plan跨scope、event必须当前Run/Attempt、非法version/错artifact/重复ID。当前 run() fixture把Succeeded与NotReady组合，这是Run领域非法状态，却用于event正向用例；建议只修fixture到AwaitingReview以贴合“调用者已验证Run”契约，未发现生产聚合承担storage校验的声明或行为。已报告主线程，变更后需target重跑/指纹刷新。

未来宿主接线需真实fixture：同名task跨project budget拒绝、scope在异步读取后撤销、currentAttempt在跨库观察期间切换标Stale、缺库不初始化、坏schema/权限拒绝不变Missing、Cancelled计划仍完整。它们不是本纯函数已完成的测试。

本轮无GUI/native IPC/model或跨平台验收；不由这五测试批准AZ035正式done。当前仅可消费纯DTO聚合契约，host读路径/权限/双视图接线下一包实现。
