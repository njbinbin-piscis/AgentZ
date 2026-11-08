---
name: "hook-development"
description: "Create and debug AgentZ project hooks in .agentz/hooks.json: shell commands run on lifecycle events, with output injected into the agent context. Use when adding, testing or troubleshooting a hook."
description_zh: "创建和调试 AgentZ 项目钩子（.agentz/hooks.json）：在生命周期事件上运行 shell 命令，并把输出注入 Agent 上下文。"
version: "2.0.0"
source: agentz
---

# AgentZ Hook Development

A hook is a shell command that AgentZ runs on a lifecycle event of the current project. Hooks are project-level and live in `{project}/.agentz/hooks.json`. They are managed in **Settings → Hooks** (create, enable/disable, test-run, save) or by editing the file directly.

## File format

```json
{
  "version": 1,
  "hooks": [
    {
      "id": "git-status",
      "name": "Git status",
      "event": "beforeAgentTurn",
      "command": "git status --short --branch",
      "enabled": true
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `id` | Unique id of the hook (required). |
| `name` | Label shown in the UI and above the hook output. Optional. |
| `event` | One of `beforeAgentTurn`, `afterAgentTurn`, `beforeFileEdit`, `afterFileEdit`. Saving any other value fails. |
| `command` | Shell command. Runs through `cmd /C` on Windows and `sh -c` elsewhere. Empty commands are skipped. |
| `enabled` | Defaults to `true`. Disabled hooks are never run. |

## What actually runs

- **`beforeAgentTurn`**: runs at the start of every agent turn, in the project root. Each enabled hook's trimmed stdout is added to the system prompt under `## Hook output (beforeAgentTurn)`, one `### <name>` block per hook. Hooks with empty stdout add nothing.
- **`afterAgentTurn`, `beforeFileEdit`, `afterFileEdit`**: accepted and saved, but the agent runtime does not execute them yet. Do not rely on them for guardrails; tell the user this when they ask for such a hook, and suggest a `beforeAgentTurn` hook that prints guidance, or a project rule in `.agentz/rules/`, instead.
- Every hook gets a **10 second timeout**; a timed-out hook contributes nothing.
- The **exit code is ignored** and failures are swallowed, so a broken hook never blocks a turn. Hooks cannot veto or modify a tool call.
- stderr is not injected. Only stdout reaches the model.
- Hooks run with the user's permissions and without sandboxing. Never put secrets in `command`, and never add a hook from an untrusted repository without reading it.

## Writing a good hook

1. Print only what the model needs: a few lines, no banners, no color codes.
2. Make it fast and idempotent: no network calls, no prompts, no files written.
3. Use commands that exist on the user's OS (the project may run on Windows, where `cmd /C` applies; avoid `grep`/`sed` pipelines unless the user has them).
4. Prefer a small script checked into the repo (for example `node scripts/context-hook.mjs`) over a long inline command.
5. Test it first: run the command in the project root, or use the test-run button in Settings → Hooks (10 second limit, shows stdout, stderr and exit code).

## Workflow

1. Ask what context or check the user wants at the start of each turn.
2. Draft the command and run it with `shell` in the project root to see the real output and timing.
3. Add the hook to `.agentz/hooks.json` (read the existing file first and keep other hooks), using `file_read` and `file_write`/`file_edit`.
4. Ask the user to open Settings → Hooks to confirm it appears and is enabled.
5. Start a new turn and check that the `## Hook output` section reflects the hook; if it does not, check `enabled`, the 10 second limit, and stdout.

## Useful examples

```json
{ "id": "branch", "name": "Current branch", "event": "beforeAgentTurn", "command": "git rev-parse --abbrev-ref HEAD", "enabled": true }
```

```json
{ "id": "todo-count", "name": "Open TODOs", "event": "beforeAgentTurn", "command": "node scripts/todo-count.mjs", "enabled": true }
```

## Project templates

Applying a project template can write hooks into `.agentz/hooks.json`. Hooks converted from other tools' formats are written with `enabled: false`; review each command before enabling it.
