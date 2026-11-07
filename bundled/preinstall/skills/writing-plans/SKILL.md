---
name: "writing-plans"
description: "Write detailed step-by-step implementation plans with exact files, code and test commands. Use when you have a spec or requirements for a multi-step task, before touching code."
description_zh: "编写详细的分步实施计划，包含确切的文件、代码和测试命令，适用于动手写代码之前"
version: "1.0.0"
source: agentz
---

# Writing Plans

## Overview

Write comprehensive implementation plans assuming the engineer has zero context for our codebase and questionable taste. Document everything they need to know: which files to touch for each task, code, testing, docs they might need to check, how to test it. Give them the whole plan as bite-sized tasks. DRY. YAGNI. TDD. Frequent commits.

Assume they are a skilled developer, but know almost nothing about our toolset or problem domain. Assume they don't know good test design very well.

**Context:** Ideally run this in a dedicated git worktree (see the using-git-worktrees skill).

**Save plans to:** `.agentz/plans/YYYY-MM-DD-<feature-name>.md` (use the `plan_write` tool, or `file_write`).

## Bite-Sized Task Granularity

**Each step is one action (2-5 minutes):**
- "Write the failing test" - step
- "Run it to make sure it fails" - step
- "Implement the minimal code to make the test pass" - step
- "Run the tests and make sure they pass" - step
- "Commit" - step

## Plan Document Header

**Every plan MUST start with this header:**

```markdown
# [Feature Name] Implementation Plan

> **For the implementing agent:** implement this plan task-by-task, running each verification step before moving on.

**Goal:** [One sentence describing what this builds]

**Architecture:** [2-3 sentences about approach]

**Tech Stack:** [Key technologies/libraries]

---
```

## Task Structure

```markdown
### Task N: [Component Name]

**Files:**
- Create: `exact/path/to/file.py`
- Modify: `exact/path/to/existing.py:123-145`
- Test: `tests/exact/path/to/test.py`

**Step 1: Write the failing test**

```python
def test_specific_behavior():
    result = function(input)
    assert result == expected
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/path/test.py::test_name -v`
Expected: FAIL with "function not defined"

**Step 3: Write minimal implementation**

```python
def function(input):
    return expected
```

**Step 4: Run test to verify it passes**

Run: `pytest tests/path/test.py::test_name -v`
Expected: PASS

**Step 5: Commit**

```bash
git add tests/path/test.py src/path/file.py
git commit -m "feat: add specific feature"
```
```

## Remember
- Exact file paths always
- Complete code in plan (not "add validation")
- Exact commands with expected output
- Name any relevant skills the implementer should enable
- DRY, YAGNI, TDD, frequent commits

## Execution Handoff

After saving the plan, tell the user where it is and offer execution choices:

"Plan complete and saved to `.agentz/plans/<filename>.md`. Two execution options:

1. **Step-by-step in this conversation** - I implement one task at a time and review the result between tasks. Fast iteration.
2. **Separate session** - open a new conversation (ideally in a worktree) and implement the plan in batches with checkpoints.

Which approach?"

If step-by-step is chosen: stay in this conversation, implement each task, and review the diff after each one (`file_diff`, `read_lints`, tests).

If a separate session is chosen: guide the user to open a new conversation in the worktree and point it at the plan file.
