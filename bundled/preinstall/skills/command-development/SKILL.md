---
name: "command-development"
description: "Create AgentZ slash commands as command.json: slash_id, argument_hint, tools and a prompt with $ARGUMENTS. Use when writing or improving a reusable /command."
description_zh: "编写 AgentZ 斜杠命令 command.json：slash_id、参数提示、工具与 $ARGUMENTS 提示词。"
version: "1.0.0"
source: agentz
---

# Command Development for AgentZ

A command is a saved prompt that the user runs by typing `/<slash_id>` in the chat box, optionally followed by arguments.

## Where commands live

`<config>/commands/<id>/command.json`. They can also be created and edited in the Commands section of the Library.

## command.json fields

| Field | Meaning |
| --- | --- |
| `id` | Folder name and stable id. Plain lowercase kebab-case. |
| `slash_id` | What the user types after `/`. Defaults to `id` when empty. Matching is case-insensitive, and the id also works. |
| `name` | Display name in Title Case. |
| `description`, `description_zh` | One plain line each, shown in the slash menu and lists. English up to 200 characters, Chinese up to 80. Both are required. |
| `argument_hint` | Placeholder shown to the user, e.g. `<file or folder>`. |
| `tools` | Builtin tool ids to enable for the turn this command runs. Only ids that really exist. |
| `prompt`, `prompt_zh` | The prompt text sent to the agent. `prompt_zh` is used when the UI language is Chinese and it is not empty. |
| `source` | Use `agentz`. |

## How a command runs

1. The user submits `/review src/api`.
2. AgentZ finds the command by `slash_id` (or id), splits off the first word, and treats the rest of the line as the arguments.
3. Every `$ARGUMENTS` (and `$arguments`) in the prompt is replaced by the argument text. With no arguments it is replaced by an empty string.
4. The resulting prompt is sent as the user message, with the command's `tools` enabled.

That is the whole mechanism. There is no positional `$1`/`$2`, no inline shell execution, no file-reference syntax, and no frontmatter. If a prompt needs structure from the arguments, tell the agent how to parse `$ARGUMENTS` in the prompt text.

## Example

```json
{
  "id": "explain-error",
  "slash_id": "explain-error",
  "name": "Explain Error",
  "description": "Explain an error message or stack trace, find the likely cause in the codebase and suggest a fix.",
  "description_zh": "解释报错或堆栈，在代码库中定位可能原因并给出修复建议。",
  "argument_hint": "<error text or file path>",
  "tools": ["file_read", "file_search", "codebase_search", "shell"],
  "prompt": "Explain this error and find its cause in the current project.\n\nInput: $ARGUMENTS\n\nSteps:\n1. If the input is a path, read it; otherwise treat it as the error text.\n2. Search the codebase for the failing symbol or message.\n3. Explain the root cause in plain language.\n4. Propose a minimal fix and show the exact change. Do not apply it unless asked.",
  "source": "agentz"
}
```

## Writing good command prompts

- **Write instructions for the agent, not a message to the user.** "Review the code for security issues" works; "I will review..." does not.
- **Put `$ARGUMENTS` where the input belongs**, usually in an explicit "Input:" line, and say what to do when it is empty (ask the user via `chat_ui`, or fall back to a sensible default such as the current changes).
- **Keep it one workflow.** A command that needs three unrelated behaviors should be three commands.
- **State the output.** Say what the final answer looks like and whether files may be changed.
- **Request only the tools needed.** A read-only analysis command should not list `file_write`, `file_edit` or `shell`.
- **Prefer `prompt` in English** and add `prompt_zh` only when a translated prompt adds value. Do not mix both languages in one prompt.
- **Be explicit about side effects.** For commands that edit files or run `shell`, say what must be confirmed first.

## Naming

Use short verbs or verb-noun pairs: `review`, `explain-error`, `write-tests`. Avoid clashing with other commands: check the existing list before choosing a `slash_id`.

## Testing a command

1. Save the file, then type `/` in the chat box and confirm the command shows with the right description in both languages.
2. Run it with typical arguments, with no arguments, and with an awkward value (quotes, long text, a path with spaces).
3. Check that the enabled tools are enough and that nothing extra is needed.
4. Tighten the prompt wherever the agent guessed.

## Checklist

- `id` is kebab-case and matches the folder; `slash_id` is what the user will type.
- Both descriptions are plain single-line text; `argument_hint` is present when arguments matter.
- `$ARGUMENTS` appears at least once if the command takes input.
- Every id in `tools` is a real builtin tool.
