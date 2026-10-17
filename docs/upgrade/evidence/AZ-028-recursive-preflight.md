# AZ-028 递归链接预检（cycle 023）

日期：2026-10-05；Codex；主实现 AZ-028，用户已要求持续执行。

主聊天 hook 与后台包装层在调用 file_search/file_list 前增加阻塞工作线程预检：按请求深度检查链接与目录读取失败，最多14层、20,000条目录项，超出则拒绝。file_search遵循固定内核的常见噪声目录跳过规则；file_list不继承该跳过规则。正常根搜索保持可用，发现链接时要求缩小到已解析源码范围。

宿主integration17/17通过（编译3m46s，测试3.84s），日志 `.cache/upgrade-approval-recursive-cycle-023.log`。新增真实Windows junction：主聊天delegate调用为零，后台包装层即使bypass_permissions=true也拒绝。原16项回归通过。台账validate和git diff --check通过（已有行尾提示）。构建退出。

这是有界预检，非OS sandbox，不消除检查后文件系统TOCTOU；未完成所有递归内容的秘密/元数据过滤，未修改固定内核遍历实现。大型目录可能需缩小范围，性能基线未测。真实模型/桌面/跨平台/回滚未验收。继续AZ-028，未发布、部署或对外发送。
