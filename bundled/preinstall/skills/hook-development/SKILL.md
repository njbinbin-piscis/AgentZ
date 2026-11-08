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

## What runs, and when

| Event | When | Effect |
|---|---|---|
| `beforeAgentTurn` | Start of every agent turn | Trimmed stdout of each hook is added to the system prompt under `## Hook output (beforeAgentTurn)`, one `### <name>` block per hook. |
| `beforeFileEdit` | Before `file_write` / `file_edit` | **Guardrail.** A hook that exits with a positive status blocks the edit; its stderr (or stdout) is returned to the model as the reason. |
| `afterFileEdit` | After a successful `file_write` / `file_edit` | Side effects only (formatter, linter). Runs before the IDE refreshes the file. |
| `afterAgentTurn` | After a turn finishes without failure | Side effects only (notify, sync). Runs in the background and never delays the reply. |

Rules for all events:

- Hooks run in the project root with a **10 second timeout**. A timed-out hook, a hook that fails to start, or one killed by a signal never blocks anything.
- Hooks of one event run in file order. A blocking `beforeFileEdit` hook stops the remaining ones.
- Only `beforeAgentTurn` output reaches the model. For the other events stdout and stderr are discarded (except the block reason above).
- Environment variables set for every hook: `AGENTZ_HOOK_EVENT`, `AGENTZ_PROJECT_DIR`, `AGENTZ_SESSION_ID`. File events add `AGENTZ_TOOL_NAME` and `AGENTZ_FILE_PATH` (the path exactly as the model gave it).
- Hooks cannot modify a tool call or its result, and there are no hooks for other tools (shell, browser, ...).
- Hooks run with the user's permissions and without sandboxing. Never put secrets in `command`, and never enable a hook from an untrusted repository without reading it.

## Writing a good hook

1. Print only what the model needs: a few lines, no banners, no color codes.
2. Make it fast and idempotent: no network calls, no prompts. A `beforeFileEdit` guardrail must exit 0 when all is fine and a positive status only for a real violation; a command such as `grep` that exits 1 when nothing matches would block every edit.
3. Use commands that exist on the user's OS (the project may run on Windows, where `cmd /C` applies; avoid `grep`/`sed` pipelines unless the user has them).
4. Prefer a small script checked into the repo (for example `node scripts/context-hook.mjs`) over a long inline command.
5. Test it first: run the command in the project root, or use the test-run button in Settings → Hooks (10 second limit, shows stdout, stderr and exit code).

## Workflow

1. Ask what context or check the user wants at the start of each turn.
2. Draft the command and run it with `shell` in the project root to see the real output and timing.
3. Add the hook to `.agentz/hooks.json` (read the existing file first and keep other hooks), using `file_read` and `file_write`/`file_edit`.
4. Ask the user to open Settings → Hooks to confirm it appears and is enabled.
5. Check the effect: the `## Hook output` section for `beforeAgentTurn`, a blocked edit for `beforeFileEdit`, the changed file for `afterFileEdit`. If nothing happens, check `enabled`, the event name, the 10 second limit, and the exit code.

## Useful examples

```json
{ "id": "branch", "name": "Current branch", "event": "beforeAgentTurn", "command": "git rev-parse --abbrev-ref HEAD", "enabled": true }
```

```json
{ "id": "no-env", "name": "Protect .env", "event": "beforeFileEdit", "command": "node scripts/guard-env.mjs", "enabled": true }
```

`scripts/guard-env.mjs` reads `process.env.AGENTZ_FILE_PATH`, prints the reason to stderr and calls `process.exit(2)` when the path ends with `.env`; otherwise it exits 0.

```json
{ "id": "fmt", "name": "Format", "event": "afterFileEdit", "command": "npx prettier --write \"%AGENTZ_FILE_PATH%\"", "enabled": true }
```

The variable syntax depends on the shell: `%AGENTZ_FILE_PATH%` on Windows, `"$AGENTZ_FILE_PATH"` elsewhere. A small Node script avoids the difference.

## Project templates

Applying a project template can write hooks into `.agentz/hooks.json`. Hooks converted from other tools' formats are written with `enabled: false`; review each command before enabling it.
