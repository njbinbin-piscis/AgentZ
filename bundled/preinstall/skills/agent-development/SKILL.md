---
name: "agent-development"
description: "Author an AgentZ agent as agent.json: fields, system prompt design, tools, skills, MCP servers, delegation, permission modes and teams. Use when creating or tuning an agent."
description_zh: "编写 AgentZ 智能体的 agent.json：字段、系统提示词、工具、技能、委派与团队。"
version: "1.0.0"
source: agentz
---

# Agent Development for AgentZ

An AgentZ agent is a single JSON file plus the prompt inside it. Agents can be chatted with directly, delegated to by the main agent, or used as team members.

## Where agents live

`<config>/agents/<id>/agent.json`. The folder name is the agent id: plain lowercase kebab-case, no vendor prefix. Agents can also be created and edited in the Agents section of the Library.

## agent.json fields

| Field | Meaning |
| --- | --- |
| `id`, `name` | Required. Stable id (matches the folder) and display name in Title Case. |
| `role` | Short role label shown in lists and used by teams, e.g. "Code Reviewer". |
| `icon`, `color` | Cosmetic. |
| `description`, `description_zh` | One plain line each (English up to 200 chars, Chinese up to 80). The main agent reads the description to decide whether to delegate, so say what the agent does and when to use it. |
| `system_prompt`, `system_prompt_zh` | The agent's instructions. The Chinese version is optional and used when the UI language is Chinese. |
| `skills` | Skill slugs whose full SKILL.md is injected whenever this agent runs. |
| `tools` | AgentZ builtin tool ids to switch on for this agent. It only adds tools; it never restricts. |
| `mcp_servers` | Names of MCP servers configured in Settings that this agent binds even if they are globally disabled. |
| `connectors` | Connector ids this agent may use (reachable through `api_connector`). |
| `llm_provider_id` | Optional provider id from Settings to run this agent on a specific model. |
| `max_iterations`, `task_timeout_secs` | Loop and wall-clock limits. 0 or omitted means the default. |
| `tool_allowlist` | Real restriction applied when the agent runs as a delegated sub-agent. Empty means the manifest imposes no limit. |
| `permission_mode` | `read_only` (default), `ask` or `inherit`. Applies when delegated. |
| `delegable` | Whether the main agent may delegate work to this agent. Default false. |
| `source` | Use `agentz`. |

`tools` and `tool_allowlist` may only contain builtin tool ids: `file_read`, `file_write`, `file_edit`, `file_list`, `file_search`, `codebase_search`, `symbol_search`, `file_diff`, `lsp`, `read_lints`, `shell`, `process_control`, `code_run`, `terminal_read`, `web_fetch`, `web_search`, `browser`, `pdf`, `vision_context`, `email`, `ssh`, `graph_explore`, `graph_search`, `impact`, `repo_memory`, `devenv`, `memory_store`, `recall_tool_result`, `plan_todo`, `plan_write`, `plan_mode_ui`, `chat_ui`, `pool_org`, `pool_chat`, `skill_manage`, `api_connector`, `delegate`.

## Minimal example

```json
{
  "id": "api-reviewer",
  "name": "API Reviewer",
  "role": "Code Reviewer",
  "description": "Reviews REST API changes for breaking changes, validation gaps and error handling. Use after editing handlers or schemas.",
  "description_zh": "审查 REST API 改动中的破坏性变更、校验缺口与错误处理。",
  "system_prompt": "You are an API reviewer. ...",
  "tools": ["file_read", "file_search", "codebase_search", "file_diff"],
  "tool_allowlist": ["file_read", "file_list", "file_search", "codebase_search", "file_diff"],
  "permission_mode": "read_only",
  "delegable": true,
  "max_iterations": 20,
  "source": "agentz"
}
```

## Writing the system prompt

Write in the second person and make every part earn its place.

1. **Identity**: one sentence on the role and the expertise it brings.
2. **Scope**: what is in and out of scope. Say what to do when a request falls outside.
3. **Process**: numbered steps the agent follows (gather context, analyze, act, verify).
4. **Quality bar**: concrete checks, not adjectives. "Cite file and line for every finding" beats "be thorough".
5. **Output format**: the exact structure of the final answer. A delegated agent returns only its final message to the caller, so it must be self-contained.
6. **Edge cases**: missing files, ambiguous requests, nothing found.

Reference tools by their builtin ids (`file_read`, `shell`, `web_fetch`, ...). Do not mention tools that the agent will not have. Keep prompts focused: 300 to 1500 words is typical. Long reference material belongs in a skill listed in `skills`.

## Delegation

- The main agent can hand work to a sub-agent with `delegate` (a builtin read-only research sub-agent) or, for agents with `delegable: true`, to your registered agent.
- A delegated agent starts with no conversation history. Its prompt and the task text the caller passes are all it knows, so the prompt should not assume prior context.
- Effective capability is the manifest ceiling narrowed by the call and by the parent session policy. No layer can widen another.
- `permission_mode`: `read_only` removes every mutating tool (`file_write`, `file_edit`, `shell`, `code_run`, `browser`, ...). `ask` allows them but still routes through approval. `inherit` follows the parent session.
- Delegated agents never get `delegate`, `chat_ui`, `plan_todo`, `plan_write`, `plan_mode_ui` or `memory_store`, and delegation depth is 1. Sensitive tools (`email`, `ssh`, `browser`, `skill_manage`) are only available when `tool_allowlist` names them.
- Only mark an agent `delegable` when its description is precise enough for the main agent to choose it correctly, and give it the narrowest `tool_allowlist` that works.

## Teams

A team groups agents for multi-agent work. Its `members` are agent ids. Two modes exist: `swarm` (a pool coordinator follows the team `org_spec`, with a `workflow_hint` of `waves`, `sequential` or `review`) and `workflow` (a deterministic graph with no coordinator). Members coordinate through `pool_chat` and `pool_org`, so give team members a clear `role` and make their prompts say how to report results.

## Choosing between an agent, a skill and a command

- Agent: a persona with its own prompt, tool ceiling and permission mode. Use for a recurring specialist or something delegated.
- Skill: a procedure or body of knowledge injected into a conversation. Use when the same instructions help many agents.
- Command: a saved prompt run as `/<slash_id>`. Use for a one-shot workflow the user triggers.

## Checklist before shipping

- `id` matches the folder name and is kebab-case.
- `description` is plain English text under 200 characters; `description_zh` is natural Chinese under 80.
- Every entry in `tools` and `tool_allowlist` is a real builtin id.
- Every slug in `skills` is installed; every name in `mcp_servers` exists in Settings.
- A reviewer or research agent is `read_only` and has no write tools.
- The prompt states the output format and what to do when blocked.
