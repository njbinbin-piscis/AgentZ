---
name: "repo-cartographer"
description: "Use when exploring unfamiliar modules, tracing cross-language Tauri IPC flows, assessing blast radius before refactoring, or recording source-anchored repository invariants"
description_zh: "代码图谱测绘与仓库外部记忆维护技能：结合 graph_explore 与 repo_memory 精准定位跨端调用链、评估改动影响面并沉淀源码锚点绑定的架构不变量"
version: "1.0.0"
source: agentz
---

# Repo Cartographer — Knowledge Graph & Anchored Repository Memory

## Overview

 Do not guess module boundaries or grep blindly across hundreds of files. AgentZ maintains a live SQLite + Tree-sitter AST knowledge graph (`.agentz/graph.db`), cross-language Tauri IPC edges (`invoke("cmd")` ↔ `#[tauri::command]`), PageRank hub scores, and source-anchored engineering memory (`.agentz/REPO_MEMORY.md`).

## Workflow

### 1. Structural Reconnaissance (`graph_explore`)
Before modifying a shared type, Tauri command, or cross-module interface:
1. Call `graph_explore` with the symbol name, command name, or subsystem path.
2. Inspect the returned sections in order:
   - **Repository Memory Notes (Source-Anchored)**: Check active invariants and pitfalls for this module. If a note is marked `⚠️ STALE`, verify the cited lines (`path:start-end`) against the current source code before relying on it.
   - **Blast radius**: Note every importer, AST caller (`calls`), and frontend-to-Rust IPC caller (`ipc_calls`).
   - **Source Code**: Review the exact numbered AST symbol definition ranges.

### 2. Cross-Boundary & Impact Verification
- When renaming or changing the payload of a Rust `#[tauri::command]`, always check the `ipc_calls` edges in `graph_explore` to update every TypeScript `invoke("...")` call site in lockstep.
- Use `impact` or `lsp` (`references`) when changing function signatures with transitive callers.

### 3. Distilling Source-Anchored Memory (`repo_memory`)
After fixing a subtle bug, uncovering a non-obvious state machine rule, or establishing a cross-layer contract:
- Call `repo_memory` with `action: "record"`:
  - `scope_kind`: `"module"` | `"file"` | `"symbol"` | `"architecture"`
  - `scope_target`: logical subsystem (e.g. `"src-tauri/commands"`, `"src/workspaces/workz"`) or file path
  - `kind`: `"invariant"` | `"pitfall"` | `"dataflow"` | `"decision"`
  - `citations`: include `{ "path": "...", "start_line": ..., "end_line": ..., "symbol": "..." }` so AgentZ computes a SHA-256 source anchor and automatically flags the note as `stale` if those lines are later edited.
- When you encounter a `stale` note during exploration and confirm the rule still holds (or update its description), call `repo_memory` with `action: "verify"` (or `action: "record"`) to refresh its source hash.
