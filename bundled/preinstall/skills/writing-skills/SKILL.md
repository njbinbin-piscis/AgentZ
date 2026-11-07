---
name: "writing-skills"
description: "Write skills that change agent behavior by testing them like code: observe failures without the skill, write the minimal fix, close loopholes. Use when creating or revising a skill."
description_zh: "像写测试一样写技能：先观察无技能时的失败，再写最小修复并堵住漏洞。"
version: "1.0.0"
source: agentz
---

# Writing Skills

Writing a skill is test-driven development applied to process documentation. You watch an agent fail without the skill, write the smallest skill that fixes the failure, then refine it until the agent keeps following it under pressure.

If you do not know how the agent fails without the skill, you do not know whether the skill teaches the right thing.

For the file format, frontmatter keys, lifecycle and the `skill_manage` tool, see skill-development. This skill is about making a skill actually work.

## What a skill is for

A skill is a reference for a technique, pattern or tool that was not obvious to the agent. It is injected as text into a conversation. It is not a narrative of how you solved a problem once, and it should not duplicate rules that belong in project rules (`.agentz/rules`), where they would apply to every conversation.

Good topics: a multi-step procedure with a right order, a discipline that is easy to skip under pressure (write the test first, verify before claiming done), a tool or API with sharp edges, a decision framework.

Poor topics: one-off solutions, conventions that tooling can enforce, general knowledge the agent already has.

## Skill types

- **Technique**: concrete steps to follow (root-cause debugging, safe migrations).
- **Pattern**: a way of thinking about a class of problems (when to split work across sub-agents).
- **Reference**: API notes, command tables, file formats.
- **Discipline**: rules the agent will be tempted to bend. These need the most testing.

## The cycle: red, green, refactor

### Red: observe the failure

1. Write two or three realistic scenarios, with a bit of pressure (time, sunk cost, "just this once").
2. Run each in a fresh conversation without the skill.
3. Write down what the agent did and the exact rationalizations it gave ("it is too simple to test", "I already checked").

### Green: write the minimal skill

Address only the failures you saw. Enable the skill in a fresh conversation and run the same scenarios. The agent should now comply.

### Refactor: close loopholes

Each new rationalization you find gets an explicit counter in the skill. Repeat the scenarios, now with more pressure, until the behavior holds. Then trim anything that did not matter.

## Discipline skills need extra structure

- State the rule once, and state that violating its letter violates its spirit.
- List the excuses you observed in a table: Excuse, Reality.
- Add a short "red flags, stop and restart" list of thoughts that signal the agent is about to cheat.
- Make the check concrete: "Run the tests with `shell` and read the exit code before saying it passes."

## Writing the description

The `description` is what the agent sees when deciding whether to ask for the skill. Make it a trigger, not a summary of the process.

- Begin with what it does, then "Use when ..." with the situations and symptoms.
- Do not describe the steps; an agent that reads a step-by-step summary may follow it and skip the body.
- Plain single-line text, English, at most 200 characters; `description_zh` is a Chinese version up to 80 characters.

## Body guidelines

- Short and scannable: When to Use, Procedure, Pitfalls, Verification.
- One excellent example beats five mediocre ones. Use a real, runnable one.
- Name tools by AgentZ ids (`shell`, `file_read`, `plan_todo`, `chat_ui`, `delegate`).
- Everything the agent needs must be in the body; supporting files are not loaded automatically.
- Use one term for each concept and keep it throughout.
- Do not add flowcharts for linear steps. A numbered list is clearer and cheaper.

## Testing checklist

- Baseline run without the skill recorded, with the failures quoted.
- Skill addresses those failures and no others.
- Runs after the skill pass in at least two different scenarios.
- Loopholes found in pressure runs are closed in the text.
- `description` triggers on the real situations and not on unrelated ones.
- Size is proportionate: remove sections the agent followed without being told.

## Common mistakes

- Writing the skill first and testing never.
- A description that summarizes the workflow.
- Long prose where a table or a short list would do.
- Examples in several languages for the same idea.
- Claiming a skill is finished because it reads well. The test is behavior.
