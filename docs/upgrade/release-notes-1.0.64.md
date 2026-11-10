# AgentZ 1.0.64 release notes

Release date: 2026-11-11. See the 1.0.64 entry in `CHANGELOG.md`.

## Added

- **Project hooks now run.** The hook events saved in Settings > Hooks (`.agentz/hooks.json`) were never executed. They are now wired in:
  - `beforeAgentTurn`: the command's stdout is added to the system prompt.
  - `beforeFileEdit`: a non-zero exit code blocks the edit and the reason is returned to the model.
  - `afterFileEdit` and `afterAgentTurn`: side effects only.
  - Commands run in the project root with a 10 s timeout. A timeout or spawn failure never blocks (fail open). Environment: `AGENTZ_HOOK_EVENT`, `AGENTZ_PROJECT_DIR`, `AGENTZ_SESSION_ID`, `AGENTZ_TOOL_NAME`, `AGENTZ_FILE_PATH`.
  - Only `file_write` and `file_edit` trigger file-edit hooks for now (not shell edits or patch tools).
- **Built-in browser: text selection.** Drag to select with live highlighting (releasing outside the panel works), double-click selects a word, triple-click a paragraph, Ctrl/Cmd+A selects all, Ctrl/Cmd+C copies the selection.
- **Built-in browser: close tabs.** Every tab has a close button. Closing the active tab switches to another one; closing the last tab shuts down the browser. Tabs of other sessions cannot be closed.

## Changed

- **Bundled agents, skills, commands and project templates are aligned with AgentZ.** Third-party wording (CodeBuddy and similar) is replaced, descriptions are plain text with English and Chinese variants, and the entries that do not apply to AgentZ are removed (32 skills, 21 agents, 110 commands).
  - Existing installs are migrated once: only bundled items (source `codebuddy` or `agentz`) are renamed or replaced, and replaced items are moved to `preinstall-retired/` in the config directory. Items you created are never touched.
- New `hook-development` skill and "Lifecycle Hooks" project template describe the real hook behavior.
