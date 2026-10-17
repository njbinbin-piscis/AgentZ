# AZ-039 限定实施与验证

CodeZ/WorkZ新增交付观察首屏：独立读取当前Run和最多200个保留产物版本，执行与Run交付决策分别显示；实际accepted/rejected不被隐藏，但不冒称指定产物版本验收。Validation附件只表示保留了记录，不表示测试通过。应用和外部发布缺少持久任务版本回执，明确未提供，不从工具文本或PR URL猜测。

CodeZ/WorkZ以当前project/session/opaque scope隔离迟到结果；错误来源不渲染为零产物。两个查看按钮仅打开对应评审或版本视图，不自动合并、创建PR、发送模型或发布。WorkZ查看变更仅在当前worktree匹配session且非busy时开放；CodeZ项目重置、新任务及切换历史前清除旧产物预览。

Root最终13项测试通过(session98207)：Summary6、wrapper4、真实Summary+wrapper结合3；完整TypeScript49458 exit0；作用域ESLint72607 exit0、7条已有hooks警告。当前AZ013评审17项实跑通过(session72648)，消费范围只读ChangeSet/FileView，旧openPr不纳入发布承诺。

七份源码与独立复核已冻结，record-implementation-review成功，所有旧共享消费者指纹原子刷新。正式status preparing/Gate未提升；三条真实桌面旅程、评审可用性、完整验证回执/验收动作仍未完成。

本批AZ036/038/040/039后续源码进入一次新的本地Windows工程候选构建；现有旧安装器保留。未执行签名、安装、回滚、跨平台或外部发布验收。
