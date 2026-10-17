# AZ-020 普通续聊前端接线

仅独占修改 chat.ts、新 chat.test.ts、CodeZ AssistantPanel.tsx、WorkZ index.tsx 与本证据，保留其它既有 dirty。宿主 continuation 同批接线由主线程负责。

`chatSend` 缺省与显式 false 均发送 clearPlan:false。显式 true 在 invoke 前 Promise reject，明确字段废弃且不能授权替换目标。普通 CodeZ Send不再基于本地planItems开放状态推测新目标；QueuedTurn clearPlan类型限定false，所有提交/自动续聊/交互回复保存false；runTurn不因普通提交清投影。WorkZ普通runRef不再无条件清planItems，发送明确false。新会话/新Task及切换身份原先清投影行为保留。

未新增同任务Replace/Cancel UI、typed目标接口或plan_get DTO恢复。前端保留已有投影不代表数据库恢复接线完成；重启查询与GUI验收仍是未完成范围。Stop仍取消Run，不是取消目标。

实际运行 `npx vitest run src/services/tauri/chat.test.ts`：exit0，1 file/3 tests通过，5.72s（mock invoke）：缺省续聊false；显式false queued/automatic调用；legacy true拒绝且不invoke。fixture只验证IPC边界，不能称组件队列渲染/桌面验收通过。

不存在现成AssistantPanel或WorkZ index组件测试，本包未用脆弱source-string镜像测试代替渲染集成。CodeZ queue生产构造点静态复核均为literal false；组件跨task/迟到响应及重启查询仍待后续DTO包针对验证。

限定git diff --check exit0。四文件ESLint session50678本记录写入时尚在执行，最终结果由下方追加；没有运行全量构建、模型或桌面。

ESLint最终：exit0，0 errors/7 react-hooks exhaustive-deps warnings（CodeZ3、WorkZ4）。不声称零warning；本包未扩大修改到这些回调依赖。

主线程96157 npx tsc --noEmit 全前端类型检查exit0。宿主同批接线91538尚在编译，当前类型检查不证明真实续聊/GUI已验收。

