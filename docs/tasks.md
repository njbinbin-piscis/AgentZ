# AgentZ Task Tracker

**English** | [简体中文](tasks.zh-CN.md)

Use this file to track multi-step work across sessions. Keep it in sync with git history so humans and agents share the same source of truth.

## Status legend

| Status | Meaning |
|--------|---------|
| `pending` | Not started |
| `in_progress` | Active work |
| `blocked` | Waiting on external input |
| `done` | Shipped and verified |

## Active tasks

| ID | Title | Status | Owner | Notes |
|----|-------|--------|-------|-------|
| task-1 | Example: Explorer create-in-folder fix | done | — | Reference only |
| upgrade-plan | Product maturity roadmap and atomic execution ledger | done | Codex | [Roadmap](upgrade/README.md), [task ledger](upgrade/tasks.md) and the local CLI were delivered; validate/render/gate self-checks pass. The ledger now has 83 items (65 `done`, 18 `deferred` as of 2026-11-02); the authoritative status lives in `upgrade/plan.json`. |

Add rows as work begins. Remove or move to **Completed** when finished.

## Completed

| ID | Title | Closed |
|----|-------|--------|
| — | — | — |

## Commit convention

Reference task IDs in commit subjects or bodies so history stays traceable:

```text
fix(codez): reveal selected file in folder manager [task-1]

- add reveal_in_folder Tauri command
- wire FileTree context menu
```

Rules:

1. One primary `[task-N]` per commit when the change maps to a tracked task.
2. Use `task-N` in PR descriptions for the same ID.
3. When closing a task, set its row to `done` in this file in the same PR (or immediately after merge).

## Session handoff

When stopping mid-task:

1. Set status to `in_progress` or `blocked`.
2. Add a one-line **Notes** entry with the next concrete step.
3. List any failing tests or uncommitted files in Notes.

## Agent workflow

1. Read this file at the start of a multi-step request.
2. Create or update a row before large changes.
3. Mark `done` only after tests relevant to the task pass.
4. Do not invent task IDs for drive-by fixes — leave Notes empty or use `—`.
