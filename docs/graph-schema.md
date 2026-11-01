# AgentZ Knowledge Graph Schema

**English** | [简体中文](graph-schema.zh-CN.md)

Version **1.0** (`GRAPH_VERSION` in `src-tauri/src/commands/graph.rs`). Serialized to `{project}/.agentz/graph.json` and mirrored into the SQLite store `{project}/.agentz/graph.db` (`src-tauri/src/commands/graph_db.rs`), which `graph_explore`, `symbol_search` / `impact` and the Wiki read.

## Top-level document

| Field | Type | Description |
|-------|------|-------------|
| `version` | string | Schema version (`"1.0"`) |
| `generated_at` | string | RFC 3339 UTC timestamp |
| `project` | string | Workspace folder name |
| `nodes` | array | Graph nodes |
| `edges` | array | Directed edges |
| `modules` | array | Module aggregates |
| `stats` | object | `{ files, nodes, edges }` counts |

## Node

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Stable id: `module:{name}`, `file:{path}` |
| `kind` | string | `module` \| `file` |
| `path` | string? | Relative path (files only; omitted for modules) |
| `name` | string | Display name (file name, or module name) |
| `layer` | string | `api` \| `service` \| `data` \| `ui` \| `utility` \| `unknown` (inferred from the path; modules are `unknown`) |
| `summary` | string | Plain-English description (optional, omitted when empty; filled by Wiki enrichment) |

## Edge

| Field | Type | Description |
|-------|------|-------------|
| `from` | string | Source node id |
| `to` | string | Target node id |
| `kind` | string | `contains` (module → file) \| `imports` (file → file) \| `ipc_calls` (frontend `invoke("cmd")` file → Rust `#[tauri::command]` file) \| `depends` (module → module, derived from cross-module `imports` / `ipc_calls`) |

Edges are de-duplicated by `(from, to, kind)`.

## Module aggregate

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Logical subsystem from `logical_module()`: drills past container roots, e.g. `src/workspaces/codez`, `src/services/tauri`, `src-tauri/commands`, `src-tauri/core` (files directly in `src-tauri/src`), `crates/<name>`, `packages/<name>`, `vendor/<name>`; otherwise the top-level directory, or `(root)` for top-level files |
| `file_count` | number | Indexed files in module |
| `in_degree` | number | Incoming `depends` edges from other modules |

## Updates

There is no per-file patch. File-watcher events go through `graph::schedule_patch` to `graph_index::notify_file_change`, which coalesces them in a single background worker per project (2.5 s debounce) and runs a full `graph::generate`. A manual rebuild (Wiki → Rebuild graph) calls the `graph_index_rebuild` command; `graph_index_status` reports the phase (`idle` / `queued` / `building`). Each build rewrites `graph.json` and syncs `graph.db`.

## Git

Commit `graph.json` if you want it for team onboarding. Keep `.agentz/graph.db`, `.agentz/index.db` and local scratch files out of version control (the AgentZ repository itself ignores the whole `/.agentz/` directory).
