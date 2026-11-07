---
name: "dispatching-parallel-agents"
description: "Split independent investigations or tasks across sub-agents in AgentZ with delegate, then merge their results. Use when facing two or more tasks that share no state and have no ordering between them."
description_zh: "把互不依赖的调查或任务拆给多个子智能体并行处理，再汇总结果。"
version: "1.0.0"
source: agentz
---

# Dispatching Parallel Agents

When several problems are unrelated (different failing test files, different subsystems, different questions), working through them one by one wastes time and fills the context with details that do not matter to the other problems. Give each independent domain to its own sub-agent and run them together.

## What a sub-agent is in AgentZ

- `delegate` starts a read-only research sub-agent. It can read and search, but it cannot edit files or run mutating commands.
- Agents registered with `delegable: true` (see agent-development) can also be delegated to, and may be allowed to write depending on their `permission_mode` (`read_only`, `ask`, `inherit`) and `tool_allowlist`.
- A sub-agent starts with no history. It sees only the task text you give it, and only its final message comes back to you.
- Sub-agents cannot ask the user questions, cannot update your plan, and cannot delegate further.
- Several sub-agents can run at the same time. The number is capped by the delegation settings (default 4).

Because the built-in sub-agent is read-only, the usual split is: sub-agents investigate and propose, you apply the changes.

## When to use

Use when:
- two or more problems have different root causes
- each can be understood without the others
- they touch different files or subsystems
- you only need their conclusions, not their intermediate output

Do not use when:
- failures look related (fixing one may fix the rest): investigate together first
- you do not yet know what is broken
- tasks must run in order, or would edit the same files
- the whole job is small enough to finish faster yourself

## Procedure

1. **Group by domain.** List the problems and put each in its own bucket (for example one failing test file per bucket). Confirm that the buckets do not share files or state.
2. **Write a focused task for each.** Include:
   - the exact scope (files, test names, error text pasted in full)
   - the goal ("find the root cause and propose a minimal fix")
   - constraints ("read-only", "do not change public API")
   - the output format ("root cause, evidence with file and line, proposed diff")
3. **Dispatch together.** Issue all `delegate` calls in the same step so they run concurrently.
4. **Review the summaries.** Read each one critically. Spot-check claims with `file_read` or `file_search`; sub-agents can be confidently wrong.
5. **Integrate.** Apply the fixes yourself with `file_edit`. Check that no two proposals touch the same code in conflicting ways.
6. **Verify.** Run the full test or build command with `shell` and make sure everything passes together.

## Writing a good task

Weak: "Fix the tests."

Strong:

```text
Investigate why these 3 tests in tests/abort.test.ts fail:
1. "keeps partial output on abort" expects "interrupted at" in the message
2. "mixed completed and aborted tools" fast tool reports aborted
3. "tracks pending tool count" expects 3, gets 0

Read the test file and the code under src/runtime/abort.ts. Decide whether each is a timing problem or a real bug. Do not suggest raising timeouts.
Return: root cause per test, evidence (file:line), and a minimal proposed change as a diff. Do not modify files.
```

## Common mistakes

- Too broad a scope, so the sub-agent wanders.
- No context: missing error text, paths or constraints.
- No output format, so you cannot merge the results.
- Dispatching tasks that actually share files.
- Trusting a summary without verifying it.

## Larger jobs

If the work needs real parallel edits by different specialists, define a team (see agent-development) instead of many ad-hoc sub-agents. If tasks are sequential with review between them, use subagent-driven-development.
