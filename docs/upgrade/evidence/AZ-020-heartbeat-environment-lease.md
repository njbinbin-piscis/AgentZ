# AZ-020 heartbeat 环境租约前置修复

本次仅改 `src-tauri/src/runtime/heartbeat.rs::dispatch_pool_attention`：将 AppState 获取前移，在 open_project_kernel_state、ensure_fixed_session、journal/runtime等操作之前取得 `state.environment_access.execution().await`，RAII guard 留在函数 scope，贯穿整个 run_agentz_turn 及后续处理；early error/drop 同样释放。删除原先稍后的重复 state 声明。

本次新增 delta 只有上述 guard/state移动与两行说明；git diff中的旧格式变化、turn调用额外project参数/queued参数属于已存在dirty修改，全部保留。未更改clear_plan=true、slot、模型行为、外发、Cargo、其它产品文件或台账。

与 chat_send 采用同一 AppState gate：shared execution读租约阻止cleanup/merge等独占写租约进入，多个执行读租约仍可并行；不会获得工具权限。turn不能再嵌套取得execution租约，以免公平读写锁在writer排队时形成自等待。后续默认continuation接线需与heartbeat clear_plan一起改，本包不先改变语义。

实际验证：限定 `git diff --check -- src-tauri/src/runtime/heartbeat.rs` exit 0，人工检查guard在首次session/runtime副作用之前且无手动提前drop。未运行Cargo、桌面或真实heartbeat模型调用。

可复用既有 `crates/agentz-task-state/src/access.rs::tests::executions_and_cleanup_exclude_each_other_and_release_on_drop` 验证同gate多execution、cleanup互斥与drop释放；本轮没有复制该实现写镜像测试，也未在本代理运行它。主线程可统一运行该case与host编译；真实heartbeat/清理争用宿主验收仍未完成。
