---
name: "find-skills"
description: "Find an existing skill for a task: check installed skills, then the SkillHub and ClawHub registries. Use when the user asks whether a skill exists or wants to extend what AgentZ can do."
description_zh: "为任务查找现有技能：先查已安装，再到 SkillHub、ClawHub 注册表搜索。"
version: "1.0.0"
source: agentz
---

# Find Skills

Help the user find a skill that already exists instead of writing a new one.

## When to use

- The user asks "is there a skill for X" or "how can I get better at X".
- A task is specialized (a framework, a file format, a deployment target) and a packaged procedure would help.
- The user wants to extend AgentZ and has not said how.

## Procedure

1. **Clarify the need.** Identify the domain, the task type and any constraints (language, framework, platform). Ask one short question with `chat_ui` if it is unclear.
2. **Check what is installed.** Call `skill_manage` with `action: "list"`. It shows every loaded skill with its lifecycle (installed, draft, learned) and one-line description. If a match exists, tell the user and suggest enabling it for this conversation.
3. **Search the registries.** Skill discovery in AgentZ happens in the Skills panel, **Discover** tab, which searches **SkillHub** and **ClawHub**. The agent cannot press those buttons, so send the user there with two or three concrete search keywords. You may also use `web_search` or `web_fetch` to look up candidate skills and bring back their names.
4. **Compare candidates.** For each one report: name, what it does, what tools it asks for, source and version. Prefer a smaller, clearly scoped skill over a broad one.
5. **Vet before recommending install.** Apply the skill-vetter checklist to anything from a registry you do not recognize. See skill-install for how installing works.
6. **If nothing fits**, offer to do the task directly, or to create a draft skill with `skill_manage` so the approach can be reused (see skill-development).

## Answer format

- Installed match: "You already have **name**: description. Enable it in the conversation settings and I will follow it."
- Registry match: "Search **keyword** in Skills, Discover, SkillHub: **name** looks relevant because ...".
- No match: say so plainly and give the direct alternative.

## Guidelines

- Do not invent skill names or registry results. Only report what you saw.
- Do not tell the user to run command-line installers that AgentZ does not provide.
- Always say which tools a candidate requests before suggesting it.
