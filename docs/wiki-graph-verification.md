# Wiki / Knowledge Graph — Acceptance and Test Guide

**English** | [简体中文](wiki-graph-verification.zh-CN.md)

This guide verifies that the in-house Wiki roadmap (P0–P3) works, with the focus on whether **Agent coding ability** has materially improved (not just the Dashboard visualisation).

## 1. Automated tests (run these first)

From the repository root:

```bash
./scripts/verify-wiki-graph.sh
```

Or manually:

```bash
cd src-tauri
cargo check
cargo test graph_agent -- --nocapture
cargo test graph:: -- --nocapture
cargo test graph_db -- --nocapture
cargo test graph_index -- --nocapture
```

The script also exports `CARGO_INCREMENTAL=0` and a local `CARGO_TARGET_DIR` (default `/tmp/agentz-cargo-target`). If `AGENTZ_VERIFY_PROJECT` points at a directory, it additionally runs `cargo run --example graph_verify <project>` (example source: `src-tauri/examples/graph_verify.rs`). The `graph_index` step is best-effort and does not fail the script.

**Pass criteria**: no compile errors; all `graph_agent` tests green (including `search_graph`, `merge_deep_wiki` and the coding brief).

---

## 2. Local functional verification (requires running AgentZ)

### 2.1 Preparation

1. Open a project **with source code** in AgentZ (the AgentZ repository itself or one of your business repositories is a good choice).
2. Wait for indexing to finish (or trigger indexing via Wiki → Quick overview).

### 2.2 P0 — Graph and artifacts

| Step | Action | Expected |
|------|------|------|
| 1 | Wiki menu → **Rebuild graph** | No errors |
| 2 | Inspect `.agentz/` | `graph.json`, `tours.json`, `domain.json`, `AGENT_CODING_BRIEF.md` exist (`graph.db` is built from `graph.json` on first `graph_explore` use) |
| 3 | Wiki → **Quick overview** | `REPO_WIKI.md` contains a Mermaid module-dependency diagram |
| 4 | Edit and save any `.rs`/`.ts` file | About 2 s later, `generated_at` in `graph.json` is updated (debounced) |

### 2.3 P1 — Dashboard

| Step | Action | Expected |
|------|------|------|
| 1 | CodeZ sidebar → **Graph** | Default **Structure** view: directory clusters connected by import-dependency edges (not isolated top-level modules) |
| 2 | Click a directory node | 1-hop upstream/downstream edges and neighbours are highlighted; everything else is dimmed |
| 3 | Double-click a directory, or **Expand subdirectories** in the details | Breadcrumb drill-down into a subdirectory cluster graph |
| 4 | **View file dependencies** in the details | File subgraph (hub-centred, ≤25 nodes); clicking a file opens it in the editor |
| 5 | Search box | Filters directory clusters |
| 6 | Switch to **Top-level modules** / **Layers** / **Business domains** | Architecture views still work |
| 7 | With a WorkZ worktree, click **Impact analysis** | Diff overlay highlights changed/affected clusters and files |

### 2.4 P2 — Agent coding enhancements (core)

| Step | Action | Expected |
|------|------|------|
| 1 | Start an Agent task from CodeZ or WorkZ | System context contains `Repository coding brief` |
| 2 | Type `@graph auth` or `@graph <module name>` | The turn expands a knowledge-graph subgraph (imports/dependencies) |
| 3 | Type `@codebase <some feature>` | Code snippets are expanded |
| 4 | Watch the Agent's tool calls | `graph_explore` / `graph_search` first, then `codebase_search`, then `file_read`/`edit` |
| 5 | Type `@g` for completion | `@graph` and `@codebase` appear (supported in both WorkZ and CodeZ) |

**Agent quality check prompt (copy into chat)**:

```
Without modifying any code, use graph_search to find the hub files related to chat_turn,
then use codebase_search to explain how wiki context is injected, and finally list the files that should be read first.
```

Expected: the Agent calls `graph_search("chat_turn")`, cites hubs/importers, then calls `codebase_search`, and does not glob blindly.

### 2.5 P3 — Team conventions and personas

| Step | Action | Expected |
|------|------|------|
| 1 | Read [agentz-artifacts.md](./agentz-artifacts.md) | It is clear which `.agentz` files may be committed |
| 2 | Dashboard → persona **Junior developer** | Shows layer explanations and Tour hints |
| 3 | Persona **Product manager** | Business-domain view by default; module details focus on responsibilities/domains |
| 4 | Persona **Power user** | Shows node ids, dependency-edge statistics and full paths |
| 5 | If `.agentz/WIKI_DEEP.md` exists | After rebuilding the graph, module nodes have a non-empty `summary`; the brief contains Module summaries |

---

## 3. Deep Wiki integration (optional)

1. Run `/ralph-loop-wiki` in WorkZ (or an equivalent slash command; bundled as `cb-ralph-loop-wiki`).
2. Confirm `.agentz/WIKI_DEEP.md` is produced.
3. **Rebuild graph** again.
4. Check the `summary` field of the corresponding module in `graph.json`.
5. Start a new Agent turn → the brief contains **Module summaries (from WIKI_DEEP)**.

---

## 4. graph_validate

There is no dedicated frontend wrapper for this command (the `validateGraph` exported from `src/services/tauri/workflow.ts` validates team workflow graphs, not the knowledge graph). Call the Tauri command `graph_validate` directly from DevTools or a custom script:

```typescript
import { invoke } from "@tauri-apps/api/core";
const v = await invoke<{ ok: boolean; warnings: string[] }>("graph_validate", { projectDir });
// Ideal state: v.ok === true with no warnings
```

---

## 5. FAQ

| Symptom | Resolution |
|------|------|
| `graph.json not found` | Wiki → Rebuild graph |
| `@graph` returns nothing | Same as above; the mention needs a query term, e.g. `@graph src-tauri` |
| Agent does not call graph_search | Confirm you are in Agent/WorkZ mode; check whether the brief was injected |
| `.agentz` is not in git | Commit per project as needed, see agentz-artifacts.md |

---

## 6. Acceptance checklist (P0–P3 summary)

- [ ] Automated: `./scripts/verify-wiki-graph.sh` passes
- [ ] L1 `REPO_WIKI.md` usable
- [ ] L2 `graph.json` + Dashboard + Tour + diff overlay
- [ ] L3 `WIKI_DEEP.md` path unified (ralph-loop)
- [ ] Agent: `AGENT_CODING_BRIEF` auto-injected + `graph_search` + `@graph`
- [ ] WorkZ: `@graph` / `@codebase` completion
- [ ] P3: agentz-artifacts doc + three personas + domain view

When every item is checked, the in-house Wiki roadmap (option A) is accepted.
