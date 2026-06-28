# AgentZ Knowledge Graph Schema

Version **1.0**. Serialized to `{project}/.agentz/graph.json`.

## Top-level document

| Field | Type | Description |
|-------|------|-------------|
| `version` | string | Schema version (`"1.0"`) |
| `generated_at` | string | ISO 8601 UTC timestamp |
| `project` | string | Workspace folder name |
| `nodes` | array | Graph nodes |
| `edges` | array | Directed edges |
| `modules` | array | Top-level module aggregates |
| `stats` | object | `{ files, nodes, edges }` counts |

## Node

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Stable id: `module:{name}`, `file:{path}` |
| `kind` | string | `module` \| `file` |
| `path` | string? | Relative path (files only) |
| `name` | string | Display name |
| `layer` | string | `api` \| `service` \| `data` \| `ui` \| `utility` \| `unknown` |
| `summary` | string | Plain-English description (optional, LLM-filled later) |

## Edge

| Field | Type | Description |
|-------|------|-------------|
| `from` | string | Source node id |
| `to` | string | Target node id |
| `kind` | string | `imports` \| `contains` \| `depends` |

## Module aggregate

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Top-level directory or `(root)` |
| `file_count` | number | Indexed files in module |
| `in_degree` | number | Incoming cross-module import edges |

## Incremental updates

`graph::patch_file` removes all nodes/edges for one file, re-parses imports, and rewrites `graph.json`. Full rebuild via `graph_generate`.

## Git

Commit `graph.json` for team onboarding. Ignore `.agentz/index.db` and local scratch files.
