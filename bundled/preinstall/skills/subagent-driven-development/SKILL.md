---
name: "subagent-driven-development"
description: "Execute an implementation plan task by task, with a fresh implementer per task and separate spec-compliance and code-quality reviews. Use when a plan has mostly independent tasks."
description_zh: "按计划逐项实施：每个任务用全新的子智能体，并分别做规格与代码质量审查。"
version: "1.0.0"
source: agentz
---

# Subagent-Driven Development

Run a written plan by giving each task to a fresh worker and reviewing its result in two passes: first does it match the spec, then is the code good. Fresh context per task avoids confusion; two ordered reviews catch both under-building and sloppy building.

## Roles in AgentZ

- **You (controller)**: read the plan, track tasks with `plan_todo`, write the task briefs, verify and integrate. You are the only one who talks to the user and updates the plan.
- **Implementer**: does one task. Either you do it yourself, or you delegate to a registered agent with `delegable: true`, a write-capable `permission_mode` (`ask` or `inherit`) and a `tool_allowlist` that includes the edit tools it needs. The built-in `delegate` sub-agent is read-only and cannot implement.
- **Reviewers**: use `delegate` (read-only). They read the diff and the files and report; they do not change anything.

Sub-agents see only the brief you give them, return only a final message, cannot ask the user questions and cannot delegate further. Put everything they need into the brief.

If no write-capable delegable agent exists, implement each task yourself and keep the two review passes via `delegate`. The structure still pays off.

## When to use

- You have a written plan (see `plan_write`) with tasks that are mostly independent.
- You want checkpoints without waiting for the user between tasks.

Do not use for tightly coupled tasks, for exploration with no plan, or for one-line fixes.

## Process

1. **Read the plan once.** Extract every task with its full text and the context it needs. Create the checklist with `plan_todo`.
2. **For each task, in order** (never run two implementers on the same files at once):
   1. Write the brief: the task text, relevant file paths, interfaces to respect, test command, and what "done" means.
   2. Implement it (yourself or via the delegable implementer). The implementer writes tests first when the task allows, runs them, and does a short self-review before reporting. If it has questions, answer them before it proceeds.
   3. **Spec review** (`delegate`): hand the reviewer the task text and ask it to inspect the actual changes (`file_diff`, `file_read`). It lists missing requirements and extra, unrequested behavior. Do not accept a spec review that only repeats the implementer's report.
   4. If gaps are found, send them back to the implementer, then run the spec review again.
   5. **Quality review** (`delegate`), only after the spec review passes: naming, structure, duplication, error handling, test quality, magic numbers.
   6. Fix issues, re-review, and repeat until approved.
   7. Run the project's tests with `shell`, then mark the task complete in `plan_todo`.
3. **Final review.** After the last task, delegate one review of the entire change against the whole plan, run the full test suite, and report to the user.

## Reviewer briefs

Spec reviewer: "Task text: ... Inspect the real code changes, not the summary. Report (a) requirements that are missing, (b) behavior that was added but not requested, (c) misreadings of the spec. Return a pass or fail verdict with file and line evidence. Do not edit files."

Quality reviewer: "The task passed spec review. Review the diff for correctness risks, readability, duplication, error handling, test coverage and consistency with surrounding code. Return strengths, then issues labeled critical, important or minor. Do not edit files."

## Red flags

Never:
- skip either review, or start the quality review before the spec review passes
- move to the next task with open review issues
- accept "close enough" on the spec
- give a worker the plan file to read instead of the task text it needs
- run implementers in parallel on overlapping code
- let a self-review replace an independent review
- fix a failed task by hand without recording why; if a worker fails, re-brief it with specific instructions

If a reviewer finds issues, the implementer fixes them and the same review runs again.

## Cost

More delegated calls (one implementer and two reviewers per task) and more preparation up front. In return, problems surface per task, while they are cheap to fix.
