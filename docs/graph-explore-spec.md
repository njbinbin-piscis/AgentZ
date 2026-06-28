# graph_explore — 自研规格（参照 CodeGraph）

> 参照项目：[colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) v1.1.1  
> 对照样本：AgentZ 仓库 `codegraph init` 索引（464 文件 · 7691 符号）  
> 状态：**Phase 0 实现中**（file 级 + 源码片段 + blast radius）；Phase 1 起 tree-sitter 符号

---

## 1. 为什么要做

CodeGraph 在 AgentZ 上的实测（2026-06）：

```bash
codegraph explore "search_graph graph_agent AGENT_CODING_BRIEF"
codegraph node graph_context_block
codegraph callers search_graph
```

| 能力 | CodeGraph | AgentZ `graph_search`（旧） |
|------|-----------|---------------------------|
| 返回带行号源码 | 是（verbatim，等同 Read） | 否 |
| 符号粒度 | function / method / import … | file / module |
| Blast radius | 谁依赖、改前注意 | 仅 importers 列表 |
| Call path | call 边 + 动态 dispatch 提示 | import 边 id 列表 |
| 存储 | `.codegraph/graph.db` SQLite FTS | `.agentz/graph.json` 全量 JSON |
| 增量 | per-file upsert + staleness 标记 | 全量 rebuild |

**自研目标**：在 `.agentz/` 内提供同等 **Agent 体验**（`graph_explore` 一次调用），不依赖外部 CodeGraph；Dashboard 继续读同一索引。

---

## 2. Agent 入口

### 2.1 工具：`graph_explore`（主）

替代/并列 `graph_search`，描述中明确优先级：

```
Structural questions → graph_explore FIRST
Implementation snippets → codebase_search
Legacy file/module ids → graph_search (deprecated)
```

**输入**

| 参数 | 类型 | 说明 |
|------|------|------|
| `query` | string | 自然语言或 symbol/path 片段 |
| `limit` | int | 种子符号数，默认 12，最大 25 |

**输出结构**（对齐 CodeGraph `codegraph_explore` Markdown）

```markdown
**Exploration: {query}**

Found {n} symbol(s) across {m} file(s).

**Blast radius — what depends on these (update/verify before editing)**

- `{name}` ({path}:{line}) — {in_count} importer(s) in `{paths...}`; {test_hint}

**Call / import flow (1-hop)**

- `{from}` --[{kind}]--> `{to}`

**Source Code**

> Verbatim on-disk source with line numbers. Treat as already Read.

**`{path}`** — {symbol_summary}

```{lang}
1  ...
```

**Staleness**（Phase 1+）

> ⚠️ Pending re-index: `src/foo.rs` (edited 1.2s ago). Read file directly for live content.
```

### 2.2 `@graph` mention

[`graph_context_block`](../src-tauri/src/commands/chat_turn.rs) 在 Phase 2 改为调用 `explore_graph` 而非 `search_graph`（Phase 0 仍用旧路径，避免行为突变）。

### 2.3 系统简报

[`coding_brief_excerpt`](../src-tauri/src/commands/graph_agent.rs) 保持不变；Deep wiki / domain 叠加不变。

---

## 3. 存储：`.agentz/graph.db`

与 [`index.db`](./agentz-artifacts.md) 并列，**专用于结构图**。

### 3.1 表结构（v1）

```sql
-- meta: version, generated_at, graph_json_hash
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE nodes (
  id TEXT PRIMARY KEY,          -- sym:path:line:name | file:path
  kind TEXT NOT NULL,           -- file | function | method | ...
  path TEXT,                    -- repo-relative
  name TEXT NOT NULL,
  layer TEXT,
  summary TEXT DEFAULT '',
  start_line INTEGER,
  end_line INTEGER
);

CREATE TABLE edges (
  from_id TEXT NOT NULL,
  to_id TEXT NOT NULL,
  kind TEXT NOT NULL,           -- imports | calls | contains | depends
  weight INTEGER DEFAULT 1,
  PRIMARY KEY (from_id, to_id, kind)
);

CREATE VIRTUAL TABLE nodes_fts USING fts5(
  name, path, summary, kind,
  content='nodes', content_rowid='rowid'
);

CREATE TABLE pending_sync (
  path TEXT PRIMARY KEY,
  updated_at INTEGER NOT NULL   -- unix ms
);
```

PRAGMA：`journal_mode=WAL`（与 CodeGraph 一致，支持并发读）。

### 3.2 与 `graph.json` 关系

| 阶段 | graph.json | graph.db |
|------|------------|----------|
| Phase 0 | 仍生成（Dashboard + 兼容） | 从 GraphDoc **导入** file/import 边 |
| Phase 1 | 可选导出快照 | **主存储** + 符号 upsert |
| Phase 2 | 只读导出 / 废弃 | 唯一真相源 |

---

## 4. 索引管线

### Phase 0（当前）

1. `graph::generate` → `graph.json`（现有 regex import）
2. `graph_db::sync_from_graph_doc` → 写入 `graph.db`
3. `explore_graph`：FTS 匹配 file 节点 → 读盘带行号源码 → import 边 blast radius

### Phase 1 — tree-sitter 符号

- 语言优先：**Rust、TypeScript/TSX**（AgentZ 主栈）
- 节点：function、method、struct、impl、interface
- 边：`calls`（同文件确定 + 跨文件 resolve）、保留 `imports`
- 增量：`sync_file(path)` 删除该文件相关行再插入（**禁止**全量 rebuild）

### Phase 2 — explore 质量

- Connect-time `(size, mtime, hash)` 对账（会话开始）
- `pending_sync` + 工具输出 staleness banner
- 框架 route 边（Express / Axum / Tauri command 等，按优先级）

### Phase 3 — Dashboard 统一

- GraphDashboard 读 `graph.db`（cluster 聚合可在 SQL 或 TS 层）
- diff overlay 映射 symbol/file

---

## 5. CodeGraph 实测 → AgentZ 应对

### 5.1 Wiki / graph 注入链（应对 explore 问题）

**问题**：`how does chat_turn inject wiki or graph context`

CodeGraph 精准命中：

```
graph_context_block (chat_turn.rs:662)
  → calls search_graph (graph_agent.rs:367)
expand_file_refs (chat_turn.rs:819)
  → calls graph_context_block

repo_wiki_context (chat_turn.rs:~1458)
  → coding_brief_excerpt + deep_wiki_excerpt + domain_context_excerpt
```

**自研 explore 应一次返回上述 3 个函数源码 + 调用链**，而不是 200 个无关 `search` 符号。

**检索策略（Phase 0 改进）**：

1. FTS 短语 + path 加权（`chat_turn`, `graph_agent`, `graph_context`）
2. 排除 `bundled/preinstall/**`（CodeGraph 噪声来源）
3. Rust 路径优先于 TS 当 query 含 `chat_turn` / `graph_agent`

### 5.2 `search_graph` blast radius

CodeGraph callers：

| Caller | Location |
|--------|----------|
| `graph_context_block` | chat_turn.rs:662 |
| `GraphSearchTool::call` | tools/graph_search.rs:51 |
| test | graph_agent.rs:707 |

explore 输出必须包含此表，而非仅 import importers。

---

## 6. 验收标准

### 自动化

```bash
cd AgentZ/src-tauri && cargo test graph_db -- --nocapture
./scripts/verify-wiki-graph.sh
```

### 手动（AgentZ 自举）

| 查询 | 预期 |
|------|------|
| `graph_explore("graph_context_block wiki inject")` | 含 `chat_turn.rs` 源码 + `search_graph` 调用 |
| `graph_explore("GraphSearchTool")` | 含 `graph_search.rs` 全文 + callers 摘要 |
| 改 `graph_agent.rs` 后 2s 内 | staleness banner（Phase 1） |

### 对标 CodeGraph（Dogfood）

同一问题 CodeGraph / AgentZ 输出 **文件重叠率 ≥ 60%**（Phase 1 起测量）。

---

## 7. 文件清单

| 文件 | 职责 |
|------|------|
| [`graph_db.rs`](../src-tauri/src/commands/graph_db.rs) | SQLite schema、sync、explore |
| [`graph_explore.rs`](../src-tauri/src/tools/graph_explore.rs) | Agent 工具 |
| [`graph_agent.rs`](../src-tauri/src/commands/graph_agent.rs) | `explore_graph()` 门面 |
| [`graph.rs`](../src-tauri/src/commands/graph.rs) | generate 后 sync db |
| 本文档 | 规格与 Phase 划分 |

---

## 8. 参考

- CodeGraph README — explore 输出格式、auto-sync 三层机制
- AgentZ [`graph-schema.md`](./graph-schema.md) — 现有 JSON schema
- AgentZ [`wiki-graph-verification.md`](./wiki-graph-verification.md) — 验收清单
