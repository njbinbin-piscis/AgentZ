# Bundled agents, skills and commands: AgentZ content guide

Everything under `bundled/preinstall/` ships inside AgentZ and is copied into the user's config
directory on first run. It must describe AgentZ as it really is. This guide is the reference for
writing and reviewing that content. `node scripts/audit-bundled.mjs --list` checks the mechanical
parts and is part of the release gate.

## What AgentZ really has

| Thing | Where / how |
| --- | --- |
| Skill | `<config>/skills/installed/<slug>/SKILL.md` (+ optional `references/`, `scripts/`, `assets/`). Frontmatter keys read by AgentZ: `name`, `description`, `description_zh`, `version`, `tools`. Enabled per conversation (full text injected) or picked by the agent. Agents learn draft skills with `skill_manage`. |
| Agent | `<config>/agents/<id>/agent.json`. Fields: `id`, `name`, `role`, `icon`, `color`, `description`, `description_zh`, `system_prompt`, `system_prompt_zh`, `skills` (slugs), `tools` (builtin tool ids to enable), `mcp_servers`, `connectors`, `llm_provider_id`, `max_iterations`, `task_timeout_secs`, `tool_allowlist`, `permission_mode` (`read_only`/`ask`/`inherit`), `delegable`, `source`, `source_plugin`. Can be a team member (Koi) and be delegated to. |
| Command | `<config>/commands/<id>/command.json`, run as `/<slash_id>`. Fields: `id`, `slash_id`, `name`, `description`, `description_zh`, `argument_hint`, `tools`, `prompt`, `prompt_zh`. `$ARGUMENTS` in `prompt` is replaced by what the user typed. |
| Project rules | `<project>/.agentz/rules/*.md` (also reads `.cursor/rules/*.md|mdc`). There is no `AgentZ.md`, `CLAUDE.md` or `CODEBUDDY.md` memory file. |
| Project files | `.agentz/plans/*.md` (plans), `.agentz/REPO_WIKI.md` (repo wiki), `.agentz/screenshots/`, `.agentz/*.db` (internal, never edit). |
| MCP servers / connectors | Configured in **Settings → MCP Servers / Connectors**, not in plugin files. Agents call connectors through `api_connector`. |
| Project hooks | `<project>/.agentz/hooks.json` (Settings → Hooks). Each hook is `{id, name, event, command, enabled}`; `event` is `beforeAgentTurn`, `afterAgentTurn`, `beforeFileEdit` or `afterFileEdit`. The command runs through the OS shell in the project root with a 10 s timeout. `beforeAgentTurn` stdout is injected into the system prompt; a `beforeFileEdit` hook exiting with a positive status blocks that edit; `afterFileEdit` / `afterAgentTurn` are side-effect only. Hooks cannot modify tool calls. There are no hook scripts shipped with plugins and no Claude-style `PreToolUse`/`Stop` events or prompt-based hooks. See the `hook-development` skill. |
| Marketplace | The Resource library has a marketplace (official backend, ClawHub, SkillHub) for agents, skills, connectors, teams and tools. It does not distribute multi-component plugins. |
| Not present | Plugins, `plugin.json`, plugin bundles, Claude-style plugin hooks (`PreToolUse`, `PostToolUse`, `Stop`, prompt-based hooks), `.claude*` / `.codebuddy*` directories, `${..._PLUGIN_ROOT}` variables, `settings.local.md` plugin settings. Content that teaches these must be removed or rewritten for the AgentZ equivalent. |

## Tool names

Never use another product's tool names. Use AgentZ builtin tool ids:

| Instead of | Write |
| --- | --- |
| Bash / terminal | `shell` (long output: `process_control`, `code_run` for sandboxed snippets) |
| Read | `file_read` |
| Write | `file_write` |
| Edit / MultiEdit | `file_edit` |
| Glob | `file_list` / `file_search` |
| Grep / semantic search | `file_search`, `codebase_search`, `symbol_search` |
| WebFetch | `web_fetch` |
| WebSearch | `web_search` |
| TodoWrite | `plan_todo` |
| Task / sub-agent | `delegate` (read-only research sub-agent) |
| AskUserQuestion | `chat_ui` |
| Skill tool | skills are enabled for the conversation or picked by the agent; do not invent a Skill tool |
| `mcp__<server>__<tool>` | the tool exposed by the MCP server the user configured; do not hard-code another product's server names |

Other builtin ids: `file_diff`, `lsp`, `read_lints`, `graph_explore`, `graph_search`, `impact`,
`repo_memory`, `devenv`, `browser`, `terminal_read`, `pdf`, `vision_context`, `email`, `ssh`,
`memory_store`, `recall_tool_result`, `plan_write`, `plan_mode_ui`, `pool_org`, `pool_chat`,
`skill_manage`, `api_connector`. In `agent.json` and `command.json`, `tools` may only contain these ids.

## Names and branding

- No `CodeBuddy`, `Claude`, `Claude Code`, `Anthropic`, `Codex`, `Cursor`, `Copilot`, `Gemini CLI`
  used as *the host product*. Say **AgentZ** only where AgentZ really does the thing described;
  otherwise delete the sentence. A blind word swap ("AgentZ Code", "AgentZ.md",
  `.AgentZ/...`, `AGENTZ_PLUGIN_ROOT`) is wrong: the file or feature must exist in AgentZ.
- Names of real third-party products and services stay as they are (GitHub, Docker, Tencent Docs,
  Stripe...). They are fine when the content works through `shell`, `web_fetch`, `api_connector` or
  an MCP server the user configures.
- Ids and names carry no vendor prefix (`cb-`, `codebuddy-`). Skill slugs and agent ids are plain
  lowercase kebab-case.
- `source` is `agentz`, `source_plugin` is empty or a neutral pack name.

## Descriptions (what the lists show)

The Skills, Agents and Commands lists show `description` (English UI) or `description_zh`
(Chinese UI) as one plain line or short paragraph.

- `description`: English, plain text, one or two sentences, at most 200 characters. Say what it does,
  then when to use it ("Use when ..."). No Markdown, HTML, backticks, code fences, bullet lists,
  headings, numbered "(1) (2) (3)" trigger lists, newlines or escaped quotes. Do not paste trigger phrases in other languages.
- `description_zh`: Chinese, plain text, at most 80 characters, natural wording, same meaning as
  `description`. Product and technical terms may stay in English. Never a placeholder, never the
  English text, never garbled characters, never empty.
- `name`: the display name in Title Case English (agents/commands) or the slug (skills).
- Commands that have no description need one in both languages.

## Language of the body

- `SKILL.md` body, `system_prompt` and command `prompt` are written in English. Where a Chinese
  version of an agent prompt is valuable it goes in `system_prompt_zh` / `prompt_zh`.
- `references/` files may stay in their original language when they are domain documentation.
- Headings and instructions inside one file use one language. Mixed English/Chinese sentences are
  not acceptable.
- Examples with code must be valid for AgentZ; no unexplained foreign paths.

## Body format

- Skill bodies are Markdown; fenced code blocks are fine inside the body, never in a description.
- No HTML wrappers or inline `<div>` styling in descriptions or prompts unless it is the actual
  subject of the skill.
- Keep each file focused. Remove marketing banners, badges and install instructions for other
  products.

## What to remove

Delete the item (and list its id in the report) when it cannot work in AgentZ or serves a platform
AgentZ users do not have: tools for CodeBuddy/Tencent-internal platforms (WeData agent
configuration, testbuddy, tcase, ALB, cloud-game, YottaDB, testx...), plugin/hook/marketplace
development that has no AgentZ equivalent, and duplicates of builtin features.
