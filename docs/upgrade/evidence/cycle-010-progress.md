# 审批响应与surface实施周期

2026-10-05，Asia/Shanghai，heartbeat agentz；只有Codex，主任务仍AZ-028。保留已有AgentZ及内核秘密修复，没有发布或部署。

交付：共享boot的ApprovalBroker；新请求namespace与原预览摘要约束；closed receiver撤销批准；pending取消；main真实webview/origin IPC检查；卡片原task/environment/expiry显示及digest提交，旧异步响应不关闭新task/新digest卡片。

自检：15项审批测试、62项前端测试、typecheck、领域严格clippy、宿主check通过；ESLint无error但7条既有依赖warning。具体日志和时长见AZ-028-broker-surface.md。构建已全部退出；未验证原生/模型/平台/生产包，不把上周期旧领域回归算作本次全套。

复盘：上一周期误将PermissionRequest缺tool-use关联视为缺执行接口；本次深入读取真正执行点，固定内核已有before_tool/after_tool，具备真实tool-use ID。已在原证据追加更正，后续直接组合JournalWithIdeNotify，不需要为本项先发布内核新接口。

状态：AZ-028仍doing且未签发到实际工具；done14/78，Gate仅M0，其他review/模型缺口不变。下一步保持本主任务：trusted环境issuer→human预览→fresh claim→实际tool→finish；验证旧confirm配置/无map不绕过、同scope普通编辑不重复询问、取消和并发不重放。完整预览可信性和protected UI要继续验收。
