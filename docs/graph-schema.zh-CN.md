# AgentZ 知识图谱模式

[English](graph-schema.md) | **简体中文**

> 本文是 [graph-schema.md](graph-schema.md) 的中文翻译。两者不一致时以英文版为准。

版本 **1.0**（`src-tauri/src/commands/graph.rs` 中的 `GRAPH_VERSION`）。序列化到 `{project}/.agentz/graph.json`，并同步到 SQLite 存储 `{project}/.agentz/graph.db`（`src-tauri/src/commands/graph_db.rs`），`graph_explore`、`symbol_search` / `impact` 和 Wiki 从中读取。

## 顶层文档

| 字段 | 类型 | 说明 |
|-------|------|-------------|
| `version` | string | 模式版本（`"1.0"`） |
| `generated_at` | string | RFC 3339 UTC 时间戳 |
| `project` | string | 工作区文件夹名 |
| `nodes` | array | 图节点 |
| `edges` | array | 有向边 |
| `modules` | array | 模块汇总 |
| `stats` | object | `{ files, nodes, edges }` 计数 |

## 节点

| 字段 | 类型 | 说明 |
|-------|------|-------------|
| `id` | string | 稳定 id：`module:{name}`、`file:{path}` |
| `kind` | string | `module` \| `file` |
| `path` | string? | 相对路径（仅文件；模块省略） |
| `name` | string | 显示名（文件名或模块名） |
| `layer` | string | `api` \| `service` \| `data` \| `ui` \| `utility` \| `unknown`（由路径推断；模块为 `unknown`） |
| `summary` | string | 英文描述（可选，为空时省略；由 Wiki 增强填写） |

## 边

| 字段 | 类型 | 说明 |
|-------|------|-------------|
| `from` | string | 源节点 id |
| `to` | string | 目标节点 id |
| `kind` | string | `contains`（模块 → 文件）\| `imports`（文件 → 文件）\| `ipc_calls`（前端 `invoke("cmd")` 文件 → Rust `#[tauri::command]` 文件）\| `depends`（模块 → 模块，由跨模块的 `imports` / `ipc_calls` 派生） |

边按 `(from, to, kind)` 去重。

## 模块汇总

| 字段 | 类型 | 说明 |
|-------|------|-------------|
| `name` | string | 由 `logical_module()` 得到的逻辑子系统：越过容器根目录继续细分，例如 `src/workspaces/codez`、`src/services/tauri`、`src-tauri/commands`、`src-tauri/core`（直接位于 `src-tauri/src` 下的文件）、`crates/<name>`、`packages/<name>`、`vendor/<name>`；否则为顶层目录，顶层文件为 `(root)` |
| `file_count` | number | 模块内已索引的文件数 |
| `in_degree` | number | 来自其他模块的 `depends` 入边数 |

## 更新

没有按文件的增量补丁。文件监视事件经 `graph::schedule_patch` 进入 `graph_index::notify_file_change`，由每个项目一个的后台 worker 合并（2.5 秒防抖）后执行完整的 `graph::generate`。手动重建（Wiki → 重建图谱）调用 `graph_index_rebuild` 命令；`graph_index_status` 报告当前阶段（`idle` / `queued` / `building`）。每次构建都会重写 `graph.json` 并同步 `graph.db`。

## Git

如需用于团队上手，可提交 `graph.json`。不要把 `.agentz/graph.db`、`.agentz/index.db` 和本地临时文件纳入版本控制（AgentZ 仓库本身忽略整个 `/.agentz/` 目录）。
