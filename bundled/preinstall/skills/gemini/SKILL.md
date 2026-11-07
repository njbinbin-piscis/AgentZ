---
name: "gemini"
description: "Run the Gemini CLI from the shell for code analysis and generation with Google Gemini models. Use when you want a second model's opinion on a complex reasoning task."
description_zh: "通过 shell 调用 Gemini CLI，用 Gemini 模型进行代码分析与生成。"
version: "1.0.0"
source: agentz
---

# Gemini CLI Integration

## Overview

Run the Google Gemini CLI from `shell` to get a second opinion from a Gemini model. This skill ships an optional helper script, `scripts/gemini.py`, in its own directory.

## When to Use

- Complex reasoning tasks where a second model's perspective helps
- Code generation and analysis with Gemini models
- Alternative perspective on code problems

## Requirements

- The Gemini CLI installed and authenticated (`gemini --version` should work)
- Optional: `uv` or Python 3 to run the helper script

## Usage

Call the CLI directly with `shell`:

```bash
gemini -p "<prompt>"
```

Or use the helper script from this skill's directory (path shown in the skill listing):

```bash
uv run scripts/gemini.py "<prompt>" [working_dir]
# or
python3 scripts/gemini.py "<prompt>" [working_dir]
```

### Parameters

- `prompt` (required): Task prompt or question
- `working_dir` (optional): Working directory (default: current directory)

### Environment Variables

- **GEMINI_MODEL**: Select the model (default: `gemini-3-pro-preview`), for example `export GEMINI_MODEL=gemini-3`

### Timeouts

Gemini can take a long time on large prompts. Run long calls with `shell` in the background and poll with `process_control` rather than blocking.

### Return Format

Plain text output from Gemini. Errors are written to stderr as `ERROR: <message>`.

### Examples

**Basic query:**

```bash
python3 scripts/gemini.py "explain quantum computing"
```

**Code analysis:**

```bash
python3 scripts/gemini.py "review this code for security issues: $(cat app.py)"
```

**With a specific working directory:**

```bash
python3 scripts/gemini.py "analyze project structure" "/path/to/project"
```

## Notes

- The script uses only the Python standard library and is cross-platform (Windows/macOS/Linux)
- It follows PEP 723 inline script metadata, so `uv run` manages the environment automatically
- All Gemini model variants are supported via `GEMINI_MODEL`
- Output is streamed directly from the Gemini CLI
