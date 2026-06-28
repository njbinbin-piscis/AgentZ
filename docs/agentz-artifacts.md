# `.agentz/` 团队产物规范

AgentZ 在每个打开的项目根目录下维护 `.agentz/` 运行时目录。其中 **Wiki / 知识图谱** 相关产物可提交到 Git，供团队 onboarding 与 Agent 编码上下文共享。

> AgentZ 自身仓库的 `.gitignore` 会忽略 `.agentz/`（本地开发数据库）。**用户项目**可按本文选择性提交结构化产物。

## 建议提交（团队共享）

| 文件 | 用途 | 生成方式 |
|------|------|----------|
| `.agentz/graph.json` | 模块/文件依赖图谱 | Wiki → 重建图谱 |
| `.agentz/tours.json` | 导览顺序 | 随图谱重建 |
| `.agentz/domain.json` | 推断的业务域 | 随图谱重建 |
| `.agentz/REPO_WIKI.md` | L1 人类快览 | Wiki → 快速概览 |
| `.agentz/AGENT_CODING_BRIEF.md` | Agent 编码简报（自动注入） | 随图谱重建 |
| `.agentz/WIKI_DEEP.md` | L3 深度 Wiki（可选） | `/ralph-loop-wiki` |

体积较大时：

- `graph.json` > **10 MB** → 考虑 [Git LFS](https://git-lfs.com/)
- `WIKI_DEEP.md` → 可仅在 CI 或 release 分支生成，日常开发不提交

## 建议忽略（本地/机器相关）

将以下内容加入**项目** `.gitignore`（按需复制）：

```gitignore
# AgentZ local runtime (do not commit)
.agentz/index.db
.agentz/piscis.db
.agentz/journal.db
.agentz/diff-overlay.json
.agentz/intermediate/
.agentz/*.db
.agentz/*.db-*
```

保留提交：`graph.json`、`tours.json`、`domain.json`、`REPO_WIKI.md`、`AGENT_CODING_BRIEF.md`（以及可选的 `WIKI_DEEP.md`）。

## 更新流程

1. 拉取代码后：Agent 会自动读取已提交的 `AGENT_CODING_BRIEF.md` / `graph.json`
2. 大改架构后：维护者执行 **Wiki → 重建图谱**，再提交更新的产物
3. 需要深度文档：运行 `/ralph-loop-wiki`，输出到 `.agentz/WIKI_DEEP.md`；下次重建图谱会把模块摘要写回 `graph.json` 节点 `summary`

## Agent 如何使用这些产物

| 机制 | 读取的产物 |
|------|------------|
| 每轮 Agent turn 自动注入 | `AGENT_CODING_BRIEF.md` |
| `graph_search` 工具 | `graph.json` |
| `@graph` mention | `graph.json` 子图检索 |
| `@codebase` mention | `index.db`（本地，不提交） |

推荐 Agent 工作流：`graph_search`（结构）→ `codebase_search`（实现）→ `file_read`（编辑）

## 冲突处理

- `graph.json` 合并冲突：以维护者 **重建图谱** 为准，勿手工编辑 JSON
- `WIKI_DEEP.md` 冲突：按文档章节协商，或重新跑 `/ralph-loop-wiki`

## 相关文档

- [graph-schema.md](./graph-schema.md) — `graph.json` 字段说明
- [wiki-graph-verification.md](./wiki-graph-verification.md) — 功能验收与测试步骤
