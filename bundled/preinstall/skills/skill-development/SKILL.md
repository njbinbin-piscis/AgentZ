---
name: "skill-development"
description: "Create and maintain AgentZ skills: the SKILL.md format, frontmatter keys, the installed, draft, learned and archived lifecycle, and skill_manage. Use when writing or improving a skill."
description_zh: "创建与维护 AgentZ 技能：SKILL.md 格式、生命周期与 skill_manage 工具。"
version: "1.0.0"
source: agentz
---

# Skill Development for AgentZ

A skill is a folder with a `SKILL.md` file: YAML frontmatter plus Markdown instructions. It teaches the agent a procedure or body of knowledge that would otherwise have to be re-explained every time.

## How AgentZ uses skills

- All installed and learned skills are listed to the agent as one line each: name and `description`. If a task matches one and it is not enabled, the agent asks the user to enable it.
- When the user enables a skill for a conversation (or an agent lists it in its `skills`), the **full body is injected** into the system prompt for that conversation.
- The skill folder lives outside the workspace and is not read by `file_read`. Only the `SKILL.md` body is injected. Files in `references/`, `scripts/` or `assets/` are not loaded automatically, so everything the agent needs must be in the body.

Consequences for writing: the `description` decides whether the skill is found, and the body must be self-contained and compact because every enabled skill costs context on every turn.

## SKILL.md format

```markdown
---
name: "pdf-report"
description: "Build a PDF report from structured data with consistent layout. Use when the user asks for a printable report."
description_zh: "用统一版式把结构化数据生成 PDF 报告。"
version: "1.0.0"
tools: ["file_write", "shell"]
---

# PDF Report

## When to Use
...

## Procedure
1. ...

## Pitfalls
- ...

## Verification
How to confirm the task succeeded.
```

Frontmatter keys AgentZ reads:

| Key | Rule |
| --- | --- |
| `name` | Required. Use the slug: lowercase kebab-case, no vendor prefix. It becomes the skill id and folder name. |
| `description` | English, plain text, one line, at most 200 characters. Say what the skill does, then "Use when ...". No Markdown, quotes inside, or lists. |
| `description_zh` | Natural Chinese, at most 80 characters, about this skill. |
| `version` | Defaults to 1.0. Bump it when behavior changes. |
| `tools` | Optional list of builtin tool ids the skill needs. Keep it minimal. When an installed skill is updated, a version that asks for more tools than before is refused until the old one is uninstalled and the new tools are reviewed. |

Use double-quoted single-line YAML for `description` and `description_zh`. Frontmatter is the block between the first two `---` lines, so do not put `---` inside a value.

## Lifecycle and where files live

Skills live under `<config>/skills/`:

| State | Folder | Who edits it |
| --- | --- | --- |
| installed | `installed/<slug>/` | Locked. Comes from a registry install, the bundled set or a hand-placed folder. The agent cannot change it. |
| draft | `.draft/<slug>/` | Created by the agent with `skill_manage`. Editable. |
| learned | `learned/<slug>/` | A draft the user promoted in the Skills panel. Still editable by the agent. |
| archived | `.archive/<slug>/` | Retired by the curator or the user. Can be restored. |

Installed and learned skills are available to conversations. The Skills panel shows installed skills with their source, version, content hash and requested tools, offers uninstall and lock/unlock, and lists drafts awaiting Promote or Discard.

## Authoring with skill_manage

The agent can write skills itself with the `skill_manage` tool (draft and learned skills only):

- `list`: show all skills with lifecycle and lock state.
- `create`: new draft from `name` plus full `content` (or just `description` for a template with When to Use / Procedure / Pitfalls / Verification).
- `edit`: replace the whole SKILL.md. `patch`: replace `old_string` with `new_string`.
- `write_file` / `remove_file`: add or remove a supporting file by relative `file_path`.
- `delete`: remove a draft.

Installed or locked skills are rejected. To change one, copy it to a new name as a draft.

## What to write

1. **Capture expertise the model lacks**: your project's conventions, a tricky multi-step procedure, a API with sharp edges, a decision table. Do not restate general knowledge.
2. **Imperative voice**: "Run the formatter, then check the diff" instead of "You should consider running...".
3. **Structure**: When to Use, Procedure (numbered), Pitfalls, Verification. Add short examples with real commands.
4. **Name tools by AgentZ ids**: `shell`, `file_read`, `file_edit`, `web_fetch`, `plan_todo`, `chat_ui`. Never reference tools that do not exist.
5. **Size**: aim for under about 400 lines. Cut anything that does not change the agent's behavior.
6. **Trigger clarity**: put the situations that should trigger the skill into the `description` as prose after "Use when".
7. **Safety**: never embed secrets; tell the agent to confirm before destructive or external actions.

## Process

1. Collect two or three concrete tasks the skill should make easier.
2. Run them once without the skill and note where the agent went wrong. That is the content.
3. Write the minimal SKILL.md that fixes those failures.
4. Enable the skill in a fresh conversation, rerun the tasks, and compare.
5. Tighten wording where the agent still strays; remove what it already did right.
6. Bump `version` after meaningful changes.

## Checklist

- `name` equals the slug and folder name; no vendor prefix.
- `description` and `description_zh` are single-line, plain and within limits.
- The body is English, self-contained and uses only real AgentZ tool ids.
- `tools` lists only what the procedure needs.
- No secrets, no absolute paths from your machine, no links to products you do not use.
