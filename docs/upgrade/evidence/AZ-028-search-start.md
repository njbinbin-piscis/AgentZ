# AZ-028 搜索起点与新索引链接检查（cycle 018）

日期：2026-10-05；Codex；主实现 AZ-028。

已读取进度、plan 校验和 cycle 017 证据；仓库/祖先未发现 AGENTS.md，开始时无运行中的构建，保留已有修改。

实际固定内核 file_search/file_list 允许输入绝对或相对目录。主聊天审批 hook 及后台包装层现在校验搜索起点：正常工作根 `.` 允许；越界、ParentDir 及受保护路径拒绝。后台的显式依赖只读根沿用宿主授权。file_search 无 path 时仅使用工作根，不回退磁盘根。

AgentZ 新建代码索引的 collect_files 现在跳过 symlink/reparse 链接条目，避免通过目录链接收集外部源码。只改变新建/重建收集过程，不宣称旧缓存索引已清除或所有递归工具已修复。

## 验证

宿主 integration 13/13 通过，编译 4m01s，测试 5.29s。日志 `.cache/upgrade-approval-search-cycle-018.log`。

新增主聊天搜索起点测试：真实外部目录、ParentDir、.agentz 均不能进入 delegate；工作根可进入。扩展后台实际 call 测试：搜索/list 越界和元数据返回错误，正常根允许。新增真实 Windows Junction 夹具：外部目录含唯一源码标记，链接放在索引工作根内，首次搜索未收集该标记。其余审批/Git/SQLite/journal/provider 回归通过。

Windows 夹具仅创建临时目录和 junction，未递归删除或移动目录；不代表原生桌面或跨平台验收。台账 validate（79 tasks/11 milestones）与 git diff --check 通过（已有行尾提示），前端/领域库未改未重跑。

剩余：固定内核递归 file_search/file_list 仍可能跟随起点内链接；起点校验不是递归 sandbox。旧索引缓存的外部路径需过滤/迁移，其他图/LSP 资源路径亦未全面验证。AZ-028 保持 doing，优先处理缓存结果边界，再核对原生 protected preview。真实模型、跨平台与回滚未验收；AZ-079 未修复，本轮未发布、部署或对外发送。
