# `.agentz/` Team Artifact Conventions

**English** | [简体中文](agentz-artifacts.zh-CN.md)

AgentZ maintains a `.agentz/` runtime directory in the root of every opened project. The **Wiki / knowledge graph** artifacts in it can be committed to Git so the team shares them for onboarding and as agent coding context.

> The AgentZ repository's own `.gitignore` ignores `.agentz/` (local development databases). **User projects** can selectively commit the structured artifacts described here.

## Recommended to commit (shared with the team)

| File | Purpose | How it is generated |
|------|---------|---------------------|
| `.agentz/graph.json` | Module/file dependency graph | Built and kept in sync automatically by the background graph index (rebuilt on file changes); also refreshed by the Repo Wiki actions |
| `.agentz/tours.json` | Guided-tour order | Rebuilt with the graph |
| `.agentz/domain.json` | Inferred business domains | Rebuilt with the graph |
| `.agentz/REPO_WIKI.md` | L1 quick overview for humans | Repo Wiki → **Fast architecture Wiki** |
| `.agentz/AGENT_CODING_BRIEF.md` | Agent coding brief (auto-injected) | Rebuilt with the graph |
| `.agentz/REPO_MEMORY.md` | Source-anchored repository memory notes | Repo Wiki → **Repository memory notes**, or the agent's `repo_memory` tool |
| `.agentz/WIKI_DEEP.md` | L3 deep Wiki (optional) | Repo Wiki → **Incremental AI DeepWiki** |

When they get large:

- `graph.json` > **10 MB** → consider [Git LFS](https://git-lfs.com/)
- `WIKI_DEEP.md` → can be generated only in CI or on release branches and left out of day-to-day commits

## Recommended to ignore (local / machine-specific)

Add the following to the **project's** `.gitignore` (copy as needed):

```gitignore
# AgentZ local runtime (do not commit)
.agentz/index.db
.agentz/piscis.db
.agentz/journal.db
.agentz/graph.db
.agentz/intermediate/
.agentz/*.db
.agentz/*.db-*
```

Keep committed: `graph.json`, `tours.json`, `domain.json`, `REPO_WIKI.md`, `AGENT_CODING_BRIEF.md`, `REPO_MEMORY.md` (and optionally `WIKI_DEEP.md`).

## Update flow

1. After pulling: the agent automatically reads the committed `AGENT_CODING_BRIEF.md` / `graph.json`.
2. After a major architecture change: the graph re-syncs automatically; a maintainer re-runs the Repo Wiki actions if needed and commits the updated artifacts.
3. For deep documentation: run Repo Wiki → **Incremental AI DeepWiki** (`repo_wiki_generate_deep`), which writes `.agentz/WIKI_DEEP.md` using a per-module SHA-256 cache and writes module summaries back into the `summary` field of `graph.json` nodes, `graph.db` and `AGENT_CODING_BRIEF.md`.

## How the agent uses these artifacts

| Mechanism | Artifact read |
|-----------|---------------|
| Auto-injected into every agent turn | `AGENT_CODING_BRIEF.md` (falls back to a pointer to `REPO_WIKI.md`) |
| `graph_search` tool | `graph.json` |
| `@graph` mention | Subgraph retrieval from `graph.json` |
| `repo_memory` tool | `REPO_MEMORY.md` / `graph.db` |
| `@codebase` mention | `index.db` (local, not committed) |

Recommended agent workflow: `graph_search` (structure) → `codebase_search` (implementation) → `file_read` (edit)

## Conflict handling

- `graph.json` merge conflicts: regenerate the graph and take that result; do not hand-edit the JSON
- `WIKI_DEEP.md` conflicts: resolve section by section, or re-run the DeepWiki generation

## Related docs

- [graph-schema.md](./graph-schema.md) — `graph.json` field reference
- [wiki-graph-verification.md](./wiki-graph-verification.md) — acceptance and test steps
