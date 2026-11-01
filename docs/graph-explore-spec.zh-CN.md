# graph_explore — 自研规格（参照 CodeGraph）

[English](graph-explore-spec.md) | **简体中文**

> 本文是 [graph-explore-spec.md](graph-explore-spec.md) 的中文翻译。两者不一致时以英文版为准。

> 参照项目：[colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) v1.1.1  
> 对照样本：AgentZ 仓库 `codegraph init` 索引（464 文件 · 7691 符号）  
> 状态（2026-11-02 按代码复核，AgentZ 1.0.40）：**Phase 0 已上线，Phase 1 大部分已上线**——file 级索引 + 源码片段 + blast radius，外加 tree-sitter AST 符号、`calls` / `ipc_calls` 边、按文件增量同步和 staleness 提示。连接时对账、Tauri IPC 以外的框架 route 边、Dashboard 读 `graph.db`（Phase 2–3）尚未实现。

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
| `query` | string | 自然语言或 symbol/path 片段（必填） |
| `limit` | int | 种子节点数，默认 12，最大 25（`explore_graph` 内钳制到 1–25） |

图索引尚未就绪时，工具直接返回 `graph_index::not_ready_message` 的提示而不执行探索。首次使用时会自动从 `graph.json` 填充 `graph.db`（`ensure_synced`）。

**输出结构**（对齐 CodeGraph `codegraph_explore` Markdown；以下为 `explore_graph` 当前实际输出的形态）

```markdown
⚠️ **Pending re-index:** {count} file(s) changed recently — Read edited files directly for live content.

**Exploration: {query}**

Found {n} node(s) across {m} file(s).

**Repository Memory Notes (Source-Anchored)**

- {✓ [ACTIVE] | ⚠️ [STALE]} **[{kind}] {title}** (`{scope_target}`): {body} — refs: {citations}

**Blast radius — what depends on these (update/verify before editing)**

- {每个 1-hop importer / caller 一行}

**Call / IPC reference sites**

- {被查询符号的调用点，或前端 invoke("…") 调用点}

**Import / call / IPC flow (1-hop)**

- `{from}` --[{kind}]--> `{to}`

**Source Code**

> Verbatim on-disk source with line numbers — treat as already Read.

**`{path}:{start}-{end}`** — `{signature or name}` ({kind})

```
1  ...
```

**`{path}`** — {name} ({kind}, {layer})

```
1  ...
```
```

当前输出说明：

- staleness 提示**放在最前面**，仅在 `pending_sync` 有记录时出现，报告的是文件**数量**，而不是逐文件路径与编辑时间。
- Repository Memory、Call / IPC reference sites、flow 三节为空时省略；blast radius 为空时输出 `_No indexed importers or callers in 1-hop._`。
- 最多 6 个命中的 AST 符号按行范围聚焦展示，之后最多 8 个整文件；代码块不带语言标记。

### 2.2 `@graph` mention

[`graph_context_block`](../src-tauri/src/commands/chat_turn/prompt_assembly.rs) 已经调用 `graph_db::explore_graph(root, query, 12)`（原计划在 Phase 2 才从 `search_graph` 切换）。它会从查询中去掉以 `@` 开头的词，无匹配时不注入。

### 2.3 系统简报

[`coding_brief_excerpt`](../src-tauri/src/commands/graph_agent.rs) 保持不变；Deep wiki / domain 叠加不变。

---

## 3. 存储：`.agentz/graph.db`

与 [`index.db`](./agentz-artifacts.zh-CN.md) 并列，**专用于结构图**（同时存放 Repository Memory 笔记与模块 wiki 缓存）。

### 3.1 表结构（以 `graph_db.rs` 中 `open_graph_db` 实际创建的为准）

```sql
-- meta: version, generated_at, graph_json_hash
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE nodes (
  id TEXT PRIMARY KEY,          -- sym:path:line:name | file:path
  kind TEXT NOT NULL,           -- file | function | method | ...
  path TEXT,                    -- repo-relative
  name TEXT NOT NULL,
  layer TEXT,
  summary TEXT NOT NULL DEFAULT '',
  start_line INTEGER,
  end_line INTEGER,
  signature TEXT NOT NULL DEFAULT '',
  content_hash TEXT NOT NULL DEFAULT ''
);

CREATE TABLE edges (
  from_id TEXT NOT NULL,
  to_id TEXT NOT NULL,
  kind TEXT NOT NULL,           -- imports | calls | ipc_calls | contains | depends
  weight INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (from_id, to_id, kind)
);
CREATE INDEX idx_edges_from ON edges(from_id);
CREATE INDEX idx_edges_to ON edges(to_id);

CREATE VIRTUAL TABLE nodes_fts USING fts5(
  name, path, summary, kind,
  content='nodes', content_rowid='rowid'
);

CREATE TABLE pending_sync (
  path TEXT PRIMARY KEY,
  updated_at INTEGER NOT NULL   -- unix ms
);

-- Repository Memory（源码锚定笔记）与模块 wiki 缓存
CREATE TABLE repo_notes (...);
CREATE TABLE module_wiki_cache (...);
```

PRAGMA：`journal_mode=WAL`（与 CodeGraph 一致，支持并发读）；连接另设 10 秒 `busy_timeout`。

### 3.2 与 `graph.json` 关系

| 阶段 | graph.json | graph.db |
|------|------------|----------|
| Phase 0 | 仍生成（Dashboard + 兼容） | 从 GraphDoc **导入**（`sync_from_graph_doc`）：file 节点、import 边，以及 AST 符号与 `calls` 边 |
| Phase 1 | 可选导出快照 | **主存储** + 符号 upsert（按文件 upsert 已实现；`graph.json` 仍在生成） |
| Phase 2 | 只读导出 / 废弃 | 唯一真相源（未实现） |

---

## 4. 索引管线

### Phase 0（已实现）

1. `graph::generate` → `graph.json`（regex import，外加跨语言 Tauri IPC `ipc_calls` 边：前端 `invoke("cmd")` → `#[tauri::command] fn cmd`）
2. `graph_db::sync_from_graph_doc` → 写入 `graph.db`
3. `explore_graph`：FTS 匹配节点（名称精确匹配优先，其次 bm25，再以 LIKE 兜底）→ 读盘带行号源码 → 基于 import / call 边的 blast radius

### Phase 1 — tree-sitter 符号（大部分已实现）

- 语言：按计划支持 **Rust、TypeScript/TSX**，另外还支持 JavaScript、Python、Go（`symbols.rs` `extract_file_skeleton`）
- 节点：function、method、struct、impl、interface（以符号提取器报告的 kind 为准）
- 边：`calls`（按符号名解析被调用者，最多 6 个匹配）、`contains`（file → symbol）；保留 `imports`
- 增量：`sync_file_in_graph_db(path)` 删除该文件的符号行与相关边再插入（**禁止**全量 rebuild）；由 `graph_index.rs` 的文件变更处理通过 `mark_pending_sync` 触发

### Phase 2 — explore 质量

- Connect-time `(size, mtime, hash)` 对账（会话开始）——未实现
- `pending_sync` + 工具输出 staleness banner——已实现（按数量提示；全量同步后 `clear_pending_sync`）
- 框架 route 边（Express / Axum / Tauri command 等，按优先级）——仅实现 Tauri IPC（`ipc_calls`）

### Phase 3 — Dashboard 统一（未实现）

- GraphDashboard 读 `graph.db`（cluster 聚合可在 SQL 或 TS 层）
- diff overlay 映射 symbol/file

---

## 5. CodeGraph 实测 → AgentZ 应对

### 5.1 Wiki / graph 注入链（应对 explore 问题）

**问题**：`how does chat_turn inject wiki or graph context`

CodeGraph 精准命中（2026-06 实测；`chat_turn.rs` 此后已拆分为 `src-tauri/src/commands/chat_turn/` 模块，`graph_context_block` 现位于 `chat_turn/prompt_assembly.rs`）：

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

1. FTS 短语 + path 加权（`chat_turn`, `graph_agent`, `graph_context`）——已实现为带引号词项的 FTS，名称精确匹配优先，再按 bm25 排序
2. 排除 `bundled/preinstall/**`（CodeGraph 噪声来源）——已实现
3. Rust 路径优先于 TS 当 query 含 `chat_turn` / `graph_agent`——代码中未能核实

### 5.2 `search_graph` blast radius

CodeGraph callers（2026-06）：

| Caller | Location |
|--------|----------|
| `graph_context_block` | chat_turn.rs:662（现已不再调用；改为调用 `explore_graph`） |
| `GraphSearchTool::call` | tools/graph_search.rs:51（现为 L71） |
| test | graph_agent.rs:707（现为 L738） |

explore 输出必须包含此表，而非仅 import importers。*当前代码：`explore_graph` 增加了 "Call / IPC reference sites" 一节（AST `calls`/`ipc_calls` 边，加上对调用点与 `invoke` 调用点的文本扫描）。*

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
| `graph_explore("graph_context_block wiki inject")` | 含 `chat_turn/prompt_assembly.rs` 源码 + `explore_graph` 调用 |
| `graph_explore("GraphSearchTool")` | 含 `graph_search.rs` 全文 + callers 摘要 |
| 改 `graph_agent.rs` 后 2s 内 | staleness banner（Phase 1） |

### 对标 CodeGraph（Dogfood）

同一问题 CodeGraph / AgentZ 输出 **文件重叠率 ≥ 60%**（Phase 1 起测量）。

---

## 7. 文件清单

| 文件 | 职责 |
|------|------|
| [`graph_db.rs`](../src-tauri/src/commands/graph_db.rs) | SQLite schema、sync、按文件增量同步、`explore_graph()` |
| [`graph_explore.rs`](../src-tauri/src/tools/graph_explore.rs) | Agent 工具 |
| [`graph_agent.rs`](../src-tauri/src/commands/graph_agent.rs) | 旧版 `search_graph()` 与 `coding_brief_excerpt()`（不再是 `explore_graph()` 门面） |
| [`graph.rs`](../src-tauri/src/commands/graph.rs) | generate 后 sync db；Tauri IPC 边 |
| [`symbols.rs`](../src-tauri/src/commands/symbols.rs) | tree-sitter 符号 / 调用提取（`extract_file_skeleton`） |
| [`graph_index.rs`](../src-tauri/src/commands/graph_index.rs) | 索引 worker、就绪提示、文件变更时 `mark_pending_sync` |
| [`prompt_assembly.rs`](../src-tauri/src/commands/chat_turn/prompt_assembly.rs) | `@graph` mention（`graph_context_block`） |
| 本文档 | 规格与 Phase 划分 |

---

## 8. 参考

- CodeGraph README — explore 输出格式、auto-sync 三层机制
- AgentZ [`graph-schema.md`](./graph-schema.zh-CN.md) — 现有 JSON schema
- AgentZ [`wiki-graph-verification.md`](./wiki-graph-verification.zh-CN.md) — 验收清单
