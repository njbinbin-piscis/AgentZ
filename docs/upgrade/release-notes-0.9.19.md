# AgentZ 0.9.19 发布说明

发布日期：2026-10-19。版本号与 `CHANGELOG.md` 的 0.9.19 条目一致。

本版本把代码图谱从文件级概览升级为符号级调用图、前后端 IPC 关联，并加入带源码锚点的仓库记忆和双轨 Wiki。

**本版本未通过上线验收门禁。** 未验收项与用户测试用例仍见 `docs/upgrade/acceptance-0.9.12.md`。

## 变更

- **图谱：** 按子系统划分模块；Tree-sitter 符号与调用边写入 `graph.db`；前端 `invoke` 连到对应的 Rust command；PageRank 标出核心文件。
- **探索：** `graph_explore` 返回符号定义行、调用方、IPC 引用，以及匹配的仓库笔记。驼峰函数名可以查到。
- **仓库记忆：** `repo_memory` 记录不变量和踩坑，锚在文件行段哈希上。被引用的代码改了，笔记会标成 stale，并同步 `.agentz/REPO_MEMORY.md`。
- **Wiki：** 快速架构 Wiki 含 AST 卡片、导览和 IPC 图；深度 Wiki 按模块内容哈希增量生成。顶栏菜单可以打开这两份文档和记忆笔记。
- **审阅：** 内置 `repo-cartographer` 技能。Markdown 里的 `path:line` 可打开源码。任务评审可按被引用次数排序变更文件。

## 已知限制

- 未通过验收门禁。发布时未把 `release-readiness` 接入 `release.yml`，否则安装包任务不会开始。
- 安装包未签名。macOS 安装包未公证，首次打开需要手动允许。
- macOS 与 Linux 安装包未在真机上运行。
- 0.9.18 的已知限制仍然适用，见 `docs/upgrade/release-notes-0.9.18.md`。
