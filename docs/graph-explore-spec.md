# graph_explore — In-house Specification (modelled on CodeGraph)

**English** | [简体中文](graph-explore-spec.zh-CN.md)

> Reference project: [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) v1.1.1  
> Comparison sample: `codegraph init` index of the AgentZ repository (464 files · 7691 symbols)  
> Status (verified against the code 2026-11-02, AgentZ 1.0.40): **Phase 0 shipped and most of Phase 1 shipped** — file-level index + source snippets + blast radius, plus tree-sitter AST symbols, `calls` / `ipc_calls` edges, per-file incremental sync and a staleness banner. Connect-time reconciliation, framework route edges beyond Tauri IPC, and the Dashboard reading `graph.db` (Phases 2–3) are not implemented.

---

## 1. Why

CodeGraph measured on AgentZ (2026-06):

```bash
codegraph explore "search_graph graph_agent AGENT_CODING_BRIEF"
codegraph node graph_context_block
codegraph callers search_graph
```

| Capability | CodeGraph | AgentZ `graph_search` (legacy) |
|------------|-----------|--------------------------------|
| Returns source with line numbers | Yes (verbatim, equivalent to Read) | No |
| Symbol granularity | function / method / import … | file / module |
| Blast radius | Who depends on it, what to check before changing | importers list only |
| Call path | call edges + dynamic dispatch hints | list of import edge ids |
| Storage | `.codegraph/graph.db` SQLite FTS | `.agentz/graph.json` full JSON |
| Incremental | per-file upsert + staleness marking | full rebuild |

**Goal**: provide the same **agent experience** inside `.agentz/` (a single `graph_explore` call) without depending on external CodeGraph; the Dashboard keeps reading the same index.

---

## 2. Agent entry points

### 2.1 Tool: `graph_explore` (primary)

Sits alongside / replaces `graph_search`; its description states the priority explicitly:

```
Structural questions → graph_explore FIRST
Implementation snippets → codebase_search
Legacy file/module ids → graph_search (deprecated)
```

**Input**

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | string | Natural language or a symbol/path fragment (required) |
| `limit` | int | Number of seed nodes, default 12, max 25 (clamped to 1–25 in `explore_graph`) |

If the graph index is not ready yet, the tool returns the `graph_index::not_ready_message` notice instead of exploring. On first use, `graph.db` is populated automatically from `graph.json` (`ensure_synced`).

**Output structure** (aligned with CodeGraph's `codegraph_explore` Markdown; this is the shape `explore_graph` emits today)

```markdown
⚠️ **Pending re-index:** {count} file(s) changed recently — Read edited files directly for live content.

**Exploration: {query}**

Found {n} node(s) across {m} file(s).

**Repository Memory Notes (Source-Anchored)**

- {✓ [ACTIVE] | ⚠️ [STALE]} **[{kind}] {title}** (`{scope_target}`): {body} — refs: {citations}

**Blast radius — what depends on these (update/verify before editing)**

- {one line per importer / caller in 1 hop}

**Call / IPC reference sites**

- {call sites or frontend invoke("…") sites for the queried symbol}

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

Notes on the current output:

- The staleness banner is printed **first**, only when `pending_sync` has rows, and reports a file **count** rather than per-file paths and edit times.
- The Repository Memory, Call / IPC reference sites and flow sections are omitted when empty; when blast radius is empty it prints `_No indexed importers or callers in 1-hop._`.
- Up to 6 matched AST symbols are rendered as focused line ranges, then up to 8 whole files; code fences carry no language tag.

### 2.2 `@graph` mention

[`graph_context_block`](../src-tauri/src/commands/chat_turn/prompt_assembly.rs) already calls `graph_db::explore_graph(root, query, 12)` (the original plan deferred this switch from `search_graph` to Phase 2). It drops `@`-prefixed words from the query and omits the block when there are no matches.

### 2.3 System brief

[`coding_brief_excerpt`](../src-tauri/src/commands/graph_agent.rs) is unchanged; the Deep wiki / domain overlays are unchanged.

---

## 3. Storage: `.agentz/graph.db`

Lives alongside [`index.db`](./agentz-artifacts.md) and is **dedicated to the structure graph** (it also holds Repository Memory notes and the module wiki cache).

### 3.1 Schema (as created by `open_graph_db` in `graph_db.rs`)

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

-- Repository Memory (source-anchored notes) and module wiki cache
CREATE TABLE repo_notes (...);
CREATE TABLE module_wiki_cache (...);
```

PRAGMA: `journal_mode=WAL` (same as CodeGraph, supports concurrent reads); connections also set a 10 s `busy_timeout`.

### 3.2 Relationship with `graph.json`

| Phase | graph.json | graph.db |
|-------|------------|----------|
| Phase 0 | Still generated (Dashboard + compatibility) | **Imported** from GraphDoc (`sync_from_graph_doc`): file nodes, import edges, plus AST symbols and `calls` edges |
| Phase 1 | Optional snapshot export | **Primary store** + symbol upsert (per-file upsert implemented; `graph.json` is still generated) |
| Phase 2 | Read-only export / deprecated | Single source of truth (not implemented) |

---

## 4. Indexing pipeline

### Phase 0 (implemented)

1. `graph::generate` → `graph.json` (regex imports, plus cross-language Tauri IPC `ipc_calls` edges from frontend `invoke("cmd")` to `#[tauri::command] fn cmd`)
2. `graph_db::sync_from_graph_doc` → writes `graph.db`
3. `explore_graph`: FTS match on nodes (exact-name first, then bm25, LIKE fallback) → read numbered source from disk → blast radius from import / call edges

### Phase 1 — tree-sitter symbols (largely implemented)

- Languages: **Rust, TypeScript/TSX** as planned, plus JavaScript, Python and Go (`symbols.rs` `extract_file_skeleton`)
- Nodes: function, method, struct, impl, interface (whatever kinds the skeleton extractor reports)
- Edges: `calls` (callee resolved by symbol name, up to 6 matches), `contains` (file → symbol); `imports` kept
- Incremental: `sync_file_in_graph_db(path)` deletes the file's symbol rows and edges and re-inserts them (**no** full rebuild); triggered from file-change handling in `graph_index.rs` via `mark_pending_sync`

### Phase 2 — explore quality

- Connect-time `(size, mtime, hash)` reconciliation (session start) — not implemented
- `pending_sync` + staleness banner in tool output — implemented (count-based banner; `clear_pending_sync` after a full sync)
- Framework route edges (Express / Axum / Tauri command etc., by priority) — only Tauri IPC (`ipc_calls`) implemented

### Phase 3 — Dashboard unification (not implemented)

- GraphDashboard reads `graph.db` (cluster aggregation in SQL or the TS layer)
- diff overlay maps symbol/file

---

## 5. CodeGraph findings → AgentZ response

### 5.1 Wiki / graph injection chain (the explore problem)

**Question**: `how does chat_turn inject wiki or graph context`

CodeGraph hit it precisely (2026-06 measurement; `chat_turn.rs` has since been split into the `src-tauri/src/commands/chat_turn/` module, and `graph_context_block` now lives in `chat_turn/prompt_assembly.rs`):

```
graph_context_block (chat_turn.rs:662)
  → calls search_graph (graph_agent.rs:367)
expand_file_refs (chat_turn.rs:819)
  → calls graph_context_block

repo_wiki_context (chat_turn.rs:~1458)
  → coding_brief_excerpt + deep_wiki_excerpt + domain_context_excerpt
```

**The in-house explore should return the source of these 3 functions + the call chain in one go**, not 200 unrelated `search` symbols.

**Retrieval strategy (Phase 0 improvements)**:

1. FTS phrase + path weighting (`chat_turn`, `graph_agent`, `graph_context`) — implemented as quoted-term FTS with exact-name-first ordering and bm25
2. Exclude `bundled/preinstall/**` (the CodeGraph noise source) — implemented
3. Prefer Rust paths over TS when the query contains `chat_turn` / `graph_agent` — not verified in code

### 5.2 `search_graph` blast radius

CodeGraph callers (2026-06):

| Caller | Location |
|--------|----------|
| `graph_context_block` | chat_turn.rs:662 (now no longer a caller; it calls `explore_graph`) |
| `GraphSearchTool::call` | tools/graph_search.rs:51 (now L71) |
| test | graph_agent.rs:707 (now L738) |

explore output must include this table rather than just import importers. *Current code: `explore_graph` adds a "Call / IPC reference sites" section (AST `calls`/`ipc_calls` edges plus a text scan for call and `invoke` sites).*

---

## 6. Acceptance criteria

### Automated

```bash
cd AgentZ/src-tauri && cargo test graph_db -- --nocapture
./scripts/verify-wiki-graph.sh
```

### Manual (AgentZ self-hosting)

| Query | Expected |
|-------|----------|
| `graph_explore("graph_context_block wiki inject")` | Contains `chat_turn/prompt_assembly.rs` source + the `explore_graph` call |
| `graph_explore("GraphSearchTool")` | Contains the full `graph_search.rs` + callers summary |
| Within 2 s of editing `graph_agent.rs` | staleness banner (Phase 1) |

### Against CodeGraph (dogfood)

For the same question, CodeGraph / AgentZ output **file overlap ≥ 60%** (measured from Phase 1).

---

## 7. File list

| File | Responsibility |
|------|----------------|
| [`graph_db.rs`](../src-tauri/src/commands/graph_db.rs) | SQLite schema, sync, incremental per-file sync, `explore_graph()` |
| [`graph_explore.rs`](../src-tauri/src/tools/graph_explore.rs) | Agent tool |
| [`graph_agent.rs`](../src-tauri/src/commands/graph_agent.rs) | Legacy `search_graph()` and `coding_brief_excerpt()` (no longer the `explore_graph()` facade) |
| [`graph.rs`](../src-tauri/src/commands/graph.rs) | generate, then sync db; Tauri IPC edges |
| [`symbols.rs`](../src-tauri/src/commands/symbols.rs) | tree-sitter symbol / call extraction (`extract_file_skeleton`) |
| [`graph_index.rs`](../src-tauri/src/commands/graph_index.rs) | Index worker, readiness notice, `mark_pending_sync` on file changes |
| [`prompt_assembly.rs`](../src-tauri/src/commands/chat_turn/prompt_assembly.rs) | `@graph` mention (`graph_context_block`) |
| This document | Spec and phase plan |

---

## 8. References

- CodeGraph README — explore output format, three-layer auto-sync mechanism
- AgentZ [`graph-schema.md`](./graph-schema.md) — existing JSON schema
- AgentZ [`wiki-graph-verification.md`](./wiki-graph-verification.md) — acceptance checklist
