---
name: "rules-improver"
description: "Audit and improve project rule files in .agentz/rules: remove noise, add real commands and conventions, keep them short. Use when asked to review, update or create project rules."
description_zh: "审查并改进项目规则 .agentz/rules：去噪、补充真实命令与约定、保持简短。"
version: "1.0.0"
source: agentz
---

# Rules Improver

Project rules are Markdown files that AgentZ injects into the system prompt of every conversation in that workspace. They are the right place for facts an agent cannot discover quickly from the code.

## How rules are loaded

- Files: `<project>/.agentz/rules/*.md` (preferred). If that folder has no non-empty rule, AgentZ falls back to `<project>/.cursor/rules/*.md` and `*.mdc`.
- Every non-empty file in the chosen folder is read in file-name order and joined under a "Project rules" heading in the system prompt.
- Only that one folder is read: there is no separate memory file and no rules from subfolders or parent directories.

Because everything is sent on every turn, size is cost. Aim for the shortest rule set that prevents real mistakes.

## Procedure

### 1. Inventory

Use `file_list` on `.agentz/rules` (and `.cursor/rules` if present) and `file_read` each file. Note which folder AgentZ will actually use.

### 2. Gather ground truth

Check what is real before judging the rules: `package.json`, `Cargo.toml`, `pyproject.toml`, `Makefile`, CI config, lint and test setup, directory layout. Use `file_search` or `codebase_search` to confirm that commands, paths and module names still exist.

### 3. Score each rule

| Question | Keep if |
| --- | --- |
| Is it true today? | Commands run, paths exist |
| Is it non-obvious? | The agent could not infer it from a quick look at the code |
| Is it actionable? | It says what to do or avoid, not a general virtue |
| Is it specific to this project? | It is not generic advice |
| Is it short? | One or two lines, with a command or example when it helps |

### 4. What good rules contain

- Build, test, lint and run commands, exactly as typed, including the working directory when it matters
- Architecture notes that save exploration: which module owns what, where entry points are
- Code style that is enforced by review but not by tooling
- Gotchas: required env vars, generated files not to edit, flaky tests, ordering constraints
- Workflow expectations: branch naming, commit style, what must pass before finishing

### 5. What to remove

- Anything stale, duplicated or contradictory between files
- Generic advice ("write clean code"), long tutorials, copied documentation
- Secrets or personal paths
- Rules that depend on tools AgentZ does not have

### 6. Propose, then apply

Show the user a short diff-style summary: what you will remove, change and add, with a reason for each. Wait for approval, then apply edits with `file_edit` (or `file_write` for a new file). Split a large file by topic, for example `commands.md`, `architecture.md`, `conventions.md`, so each stays focused.

## Template for a new rules file

```markdown
# Commands
- Install: <command>
- Test: <command>
- Lint: <command>

# Architecture
- <dir>: <what lives here>

# Conventions
- <specific, checkable convention>

# Gotchas
- <non-obvious thing that has caused mistakes>
```

## Done when

- Every command and path was checked against the repository.
- No file is longer than it needs to be and nothing is duplicated.
- The user has seen and approved the change summary.
