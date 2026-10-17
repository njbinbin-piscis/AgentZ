# AZ-018 被动执行事件预览与已提交游标通知

2026-10-08，三个 Codex 子代理独占投影模块、面板及宿主接线，主线程复核与集成。仅当前 Attempt 的只读预览，不恢复模型、工具或审批；完整 session 对话仍由已有消息存储提供。

投影匹配实际 AgentEvent serde 类型，合并 text_delta 而忽略 message_commit 避免重复；显示工具名及状态、错误计数，隐藏原始工具参数/结果、权限卡和错误详情。陌生类型或坏格式明确失败；限制5000事件、64Ki字符文本与100工具。常见凭证模式有限脱敏，不保证识别任意秘密。多 text segment 暂连续拼接，无完整对话轮次重建。

面板保持 scope/requestKey 首帧隔离及旧请求 sequence 屏蔽，只在完整分页验证后显示。独立复核发现原100页可能累计接近400MiB且最终才拒绝5000事件；主线程增加每页append前累计5000条/8MiB JSON字节预算，超限停止读取且不显示部分结果，最终snapshot复用一次。该预算限制存储JSON而非浏览器总堆内存，仍有当页IPC及临时副本开销。

宿主在原agent_event发布后发送agent_event_committed完整持久Event，仅真实durable writer路径。原channel与payload兼容；sink无客户端ack。前端尚不消费新通知，不宣称live去重/断流自动恢复完成；下一包用通知作为高水位，经完整Task/Environment/Run/Attempt和UI generation校验后读取补齐，不能跳过生命周期sequence或重复回灌现有文字。

实际主线程35288：27项前端目标测试、typecheck及限定ESLint命令链exit0。格式化后15项目标重跑通过。首次Cargo误用包名agentz，exit1未执行测试；改用agentz-desktop后48041编译及writer4项通过exit0。新增预算测试与最终前端验证12017待结果追加。GUI、真实模型、流式吞吐、跨平台与安装/回滚未验收；当前修改未包含旧0.9.7安装器，未发布。

主线程收尾12017最终exit0：28项目标测试、TypeScript检查、限定ESLint全部通过。宿主48041 writer4亦exit0。并未进行桌面/模型或端到端断流验收。
