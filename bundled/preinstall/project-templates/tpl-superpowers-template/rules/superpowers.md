# Superpowers Engineering Discipline

This project uses the **Superpowers** engineering workflow. Follow these mandatory rules on every coding, debugging, or planning task:

## 1. Skill-First & Design-First (`brainstorming` / `writing-plans`)
- Before writing or modifying code on any non-trivial task, check the **Available skills** section in your system prompt (especially `brainstorming`, `writing-plans`, `executing-plans`, `test-driven-development`, `systematic-debugging`, `verification-before-completion`, and `subagent-driven-development`).
- When a skill matches the current phase of work, read its `SKILL.md` file with `file_read` and follow its checklist.
- For multi-step work, break the task down in `plan_todo` before making your first code edit, keeping exactly one item `in_progress` at a time.

## 2. Understand Structure & Blast Radius Before Editing
- Never guess file locations, symbol signatures, or call sites.
- Use `graph_explore`, `symbol_search`, `impact`, or `codebase_search` first to understand module boundaries and callers, then `file_read` the exact target lines before calling `file_edit`.
- Check and update `.agentz/REPO_MEMORY.md` via `repo_memory` when working around non-obvious architectural invariants.

## 3. Test-Driven Development (`test-driven-development`)
- Follow the **Red → Green → Refactor** cycle:
  1. **Red**: Write or update a focused test (or minimal reproduction script) that captures the desired behavior or bug, and verify that it fails for the expected reason.
  2. **Green**: Apply the minimal, clean code change via `file_edit` to make the test pass.
  3. **Refactor**: Clean up duplication or sharp edges while keeping tests and diagnostics green.

## 4. Systematic Root-Cause Debugging (`systematic-debugging`)
- Never apply speculative or shotgun fixes when a test, build, or runtime behavior fails.
- Gather evidence first: read the full error output/stack trace, trace the data/control flow back to the origin (`graph_explore` / `lsp` / `file_read`), state a clear root-cause hypothesis, and fix the root cause rather than masking symptoms.

## 5. Verification Before Completion (`verification-before-completion`)
- Do not claim a task is complete or a bug is fixed without fresh verification evidence in the current turn:
  1. Run `read_lints` on all modified source files first (fast LSP/compiler check) and resolve any errors or warnings you introduced.
  2. Run the relevant unit/integration test or smoke check via `code_run` / `shell` and confirm it passes.
  3. Mark the corresponding `plan_todo` item `completed` only after verification succeeds.
