# AZ-028 递归受保护内容范围（cycle 026）

日期：2026-10-05；Codex；主实现 AZ-028。开始时台账79项、14已验收，无运行中cargo/rustc/link；未发现适用AGENTS.md，保留已有修改。

固定内核 file_search 无受保护内容排除参数。主聊天与后台共享遍历预检现在拒绝可遍历范围内的 .env/.env.*、.secret_key 及 .agentz/.codez/.piscis/.git。按固定内核 file_search 噪声目录跳过规则，正常 .git 目录不被遍历；file_list 默认跳过点开头隐藏项，include_hidden=true 时检查这些内容。链接检查仍先执行。范围拒绝提示要求选择更窄源码目录。

行为影响：含宿主状态或秘密的根目录 file_search 暂不能继续，调用应缩小到 src 等源码目录，或使用现有安全索引查询。未实现内核遍历时排除，不能从搜索输出过滤来替代读取前保护；此方案保守拒绝整个范围，未根据 glob/include 推断安全性。

首次宿主回归16通过、1失败：旧后台测试预期含宿主状态的根目录放行。更新用例后最终17/17通过（4.14秒），进程退出0。主聊天与后台测试覆盖 .env.local 根搜索拒绝、include_hidden=true 列表拒绝、src 范围允许，主聊天delegate为零；后台ctx仍设置bypass_permissions=true。日志 `.cache/upgrade-protected-search-cycle-026.log` 和 `.cache/upgrade-protected-search-cycle-026-final.log` 保留失败及修正。未改前端/领域库，未重跑其测试。

仍有预检后TOCTOU、OS沙箱和LSP结构化返回路径待办。未运行真实模型、原生桌面、跨平台、回滚验收。AZ-028仍doing，未发布、部署或对外发送。
