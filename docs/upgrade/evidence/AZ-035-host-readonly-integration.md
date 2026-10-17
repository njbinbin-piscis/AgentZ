# AZ035 宿主只读接口实际集成结果

27341实际cargo test -p agentz-desktop runtime::task_details::tests --offline --lib：4 passed、0 failed、0 ignored，执行0.16秒，编译6分32秒。有效数据库读取完整取消计划/Running Run且bytes不变；缺库不建；坏库错误不假missing；crossproject和未登记managed scope拒绝。

chat_task_details已注册，同opaque scope/mainWindow/existing session/freshbinding/environment lease及await后scope重核；独立接口复核未见阻断。仅宿主领域fixture，并未真实调用桌面IPC、GUI或两个视图验收。事件unknown、产物unsupported、预算观察式，跨库非原子；整体读取失败返回Err。

下一步前端统一service/DTO及两视图接入。本批未打包或发布。
