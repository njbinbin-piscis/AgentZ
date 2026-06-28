# Wiki / 知识图谱 — 验收与测试指南

本文用于验证 P0–P3 Wiki 自研路线是否正常工作，重点确认 **Agent 编码能力** 是否实质提升（而非仅 Dashboard 可视化）。

## 一、自动化测试（推荐先跑）

在项目根目录执行：

```bash
./scripts/verify-wiki-graph.sh
```

或手动：

```bash
cd AgentZ/src-tauri
cargo check
cargo test graph_agent -- --nocapture
cargo test graph:: -- --nocapture
```

**通过标准**：无编译错误；`graph_agent` 全部测试 green（含 `search_graph`、`merge_deep_wiki`、编码简报）。

---

## 二、本地功能验证（需启动 AgentZ）

### 2.1 准备

1. 用 AgentZ 打开一个**有源码**的项目（建议用 AgentZ 自身仓库或你的业务仓库）
2. 等待索引完成（或 Wiki → 快速概览触发 index）

### 2.2 P0 — 图谱与产物

| 步骤 | 操作 | 预期 |
|------|------|------|
| 1 | Wiki 菜单 → **重建图谱** | 无报错 |
| 2 | 检查 `.agentz/` | 存在 `graph.json`、`tours.json`、`domain.json`、`AGENT_CODING_BRIEF.md` |
| 3 | Wiki → **快速概览** | `REPO_WIKI.md` 含 Mermaid 模块依赖 |
| 4 | 修改任意 `.rs`/`.ts` 文件并保存 | ~2s 后 `graph.json` 的 `generated_at` 更新（debounce） |

### 2.3 P1 — Dashboard

| 步骤 | 操作 | 预期 |
|------|------|------|
| 1 | CodeZ 侧栏 → **图谱** | 默认 **结构** 视图：目录 cluster 之间有 import 依赖边（非顶层 module 孤岛） |
| 2 | 点击某目录节点 | 1-hop 上下游边与邻居高亮，其余 dim |
| 3 | 双击目录或详情 **展开子目录** | 面包屑下钻，子目录 cluster 图 |
| 4 | 详情 **查看文件依赖** | file 子图（hub 为中心，≤25 节点）；点击文件打开编辑器 |
| 5 | 搜索框 | 过滤目录 cluster |
| 6 | 切换 **顶层模块** / **分层** / **业务域** | 架构视图仍可用 |
| 7 | WorkZ 有 worktree 时点 **影响分析** | diff overlay 高亮变更/波及 cluster 与 file |

### 2.4 P2 — Agent 编码增强（核心）

| 步骤 | 操作 | 预期 |
|------|------|------|
| 1 | CodeZ 或 WorkZ 发起 Agent 任务 | 系统上下文含 `Repository coding brief` |
| 2 | 输入 `@graph auth` 或 `@graph 模块名` | turn 展开知识图谱子图（import/依赖） |
| 3 | 输入 `@codebase 某功能` | 展开代码片段 |
| 4 | 观察 Agent 工具调用 | 先 `graph_explore` / `graph_search`，再 `codebase_search`，再 `file_read`/`edit` |
| 5 | 输入 `@g` 补全 | 出现 `@graph`、`@codebase`（WorkZ/CodeZ 均支持） |

**Agent 质量检查问题（可复制到聊天）**：

```
在不修改代码的前提下，用 graph_search 找出与 chat_turn 相关的 hub 文件，
再用 codebase_search 说明 wiki 上下文是如何注入的，最后给出应优先阅读的文件列表。
```

预期：Agent 调用 `graph_search("chat_turn")`，引用 hub/importers，再 `codebase_search`，不会盲目 glob。

### 2.5 P3 — 团队规范与 Persona

| 步骤 | 操作 | 预期 |
|------|------|------|
| 1 | 阅读 [agentz-artifacts.md](./agentz-artifacts.md) | 明确哪些 `.agentz` 文件可提交 |
| 2 | Dashboard → 视角 **初级开发** | 显示分层说明、Tour 提示 |
| 3 | 视角 **产品经理** | 默认业务域视图，模块详情偏职责/域 |
| 4 | 视角 **高级用户** | 显示 node id、依赖边统计、完整路径 |
| 5 | 若存在 `.agentz/WIKI_DEEP.md` | 重建图谱后 module 节点 `summary` 非空；简报含 Module summaries |

---

## 三、深度 Wiki 联动（可选）

1. WorkZ 运行 `/ralph-loop-wiki`（或等价 slash）
2. 确认输出 `.agentz/WIKI_DEEP.md`
3. 再次 **重建图谱**
4. 检查 `graph.json` 中对应 module 的 `summary` 字段
5. 新开 Agent turn → 简报中出现 **Module summaries (from WIKI_DEEP)**

---

## 四、graph_validate

在 DevTools 或自定义脚本中调用：

```typescript
import { validateGraph } from "./services/tauri/graph";
const v = await validateGraph(projectDir);
// v.ok === true 且无 warnings 为理想状态
```

---

## 五、常见问题

| 现象 | 处理 |
|------|------|
| `graph.json not found` | Wiki → 重建图谱 |
| `@graph` 无内容 | 同上；mention 后需带查询词，如 `@graph src-tauri` |
| Agent 未调用 graph_search | 确认是 Agent/WorkZ 模式；查看简报是否注入 |
| `.agentz` 不在 git 中 | 用户项目按需提交，见 agentz-artifacts.md |

---

## 六、验收清单（P0–P3 总表）

- [ ] 自动化：`./scripts/verify-wiki-graph.sh` 通过
- [ ] L1 `REPO_WIKI.md` 可用
- [ ] L2 `graph.json` + Dashboard + Tour + diff overlay
- [ ] L3 `WIKI_DEEP.md` 路径统一（ralph-loop）
- [ ] Agent：`AGENT_CODING_BRIEF` 自动注入 + `graph_search` + `@graph`
- [ ] WorkZ：`@graph` / `@codebase` 补全
- [ ] P3：agentz-artifacts 文档 + Persona 三档 + domain 视图

全部勾选即视为 Wiki 自研路线（方案 A）验收通过。
