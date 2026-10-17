# AZ-035 共享被动任务概况面板候选

2026-10-09。新增TaskDetailsPanel.tsx/test.tsx，调用parallel代理taskDetails.readTaskDetails(scopeHandle,sessionId)，props projectDir/sessionId/busy。主线程之后挂载两个workspace。本代理未运行npm/Cargo/typecheck/GUI，也不修改共享入口/台账。

scope opaque handle来自useProjectScope；requestKey包含project/handle/session，render第一帧不匹配时屏蔽旧数据，effect generation/disposed防旧异步回复。请求开始清空旧数据，失败明确alert且不显示原始host错误。初始、busy变化、focus/manual刷新，无timer polling；在途focus/manual忽略，卸载移除listener和屏蔽迟到回复。

只显示执行状态、持久计划状态与done/total（Cancelled仍保留完整items）、环境/event/artifact/source未知。预算source可available但明确执行控制未接/no remaining balance；不显示余额0。own-property状态映射避免prototype名字成为Reactchild。没有执行/恢复/权限按钮，source非原子提示清晰，不展示原始ID、payload、工具参数或完整objective。

四个组件fixture源码：Cancelled进度/opaque参数/无假余额；切session旧回复拒绝；刷新失败清旧并不暴露raw错误；缺scope不读及恶意状态名unknown。尚未运行，主线程统一目标测试和tsc/lint；首帧隔离代码不以effect先执行为前提。

主线程13360实际4组件fixtures全部失败：node react-test环境无window，tsc链因此尚未执行。测试已采用既有RunHistoryPanel同样的真实EventTarget window stub及卸载后unstub清理，生产focus行为未修改。本代理不运行npm；主线程重跑后再记录结果。

主线程独立P2指出在途focus会丢刷新意图。生产组件已改pending布尔合并，finally在同generation未disposed时追加一次fresh read，不跳scope、不timer polling。第五fixture用deferred请求+重复focus证明仅补一次且展示新snapshot、卸载移除listener；未运行npm，主线程统一验证。
