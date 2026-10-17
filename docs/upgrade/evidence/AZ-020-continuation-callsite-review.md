# AZ-020 Continue 调用覆盖只读复核

范围为当前源码全部 chatSend/chat_send/run_agentz_turn调用、clearPlan/clear_plan字段和自动续聊动作。未修改产品、未启动build；root tsc96157/hostcompile91538当时仍在途，不能以本次静态检索替代其结果。

结论：未发现同批Continue存在runtime残留true调用或旧boolean参数类型调用。唯一 `clearPlan:true` 是service负向测试，预期invoke前拒绝。没有新增source-string断言测试。

- 生产chatSend调用只有CodeZ AssistantPanel和WorkZ index两个；两者明确false，service缺省false并拒绝显式true。没有发现其他直接invoke(chat_send)绕开service的生产入口。
- CodeZ QueuedTurn字段literal false；普通Send queue/直接run构造点全false；timeout自动续聊及planBuild动作同样false。原先UI开放项推断已移除。该结论是代码路径复核，不代表queue组件渲染实跑。
- WorkZrunRef所有后续消息/自动续聊最终经同一chatSend路径false；同任务发送不清计划投影。newTask和session切换清本地投影仍合法身份切换，未调用计划Cancel。
- chat_send先调用continuation_plan_intent：None/Some(false) →false，Some(true)明确error，发生于lease/kernel/queuedRun副作用前。它传解析后的bool给turn。
- turn自身入口guard clear_plan=true拒绝，保护其它调用端，无新目标Replace猜测。生产调用只有chat_send与runtime heartbeat；heartbeat现在传false并持环境lease，未发现第三调用端。
- planBuild动作从探索模式转agent并用文件prompt续聊，仍false；它不是同任务显式替换结构化计划的接口。新session/task仍独立身份，普通文本输入不等同授权替换。

仍未覆盖或未交付：没有typed Replace/Cancel动作、权威计划DTO查询恢复；普通UI Stop调用chatCancel仅停Run。service3个mockinvoke行为测试已有实跑，组件自动续聊/队列及重启渲染尚未实跑。本复核无GUI/模型验收结论。计划文件markdown上下文不等同todo权威库。
