# R10-06: WorkZ ↔ CodeZ One-Click Worktree Handoff Evidence

- **Task ID**: `R10-06`
- **Date**: 2026-10-19
- **Status**: `done`

## Summary of Changes

1. **Handoff Entry Points (`src/workspaces/workz/AgentTaskReview.tsx`, `src/workspaces/workz/AgentFilePreview.tsx`)**:
   - Added primary action button `在 CodeZ 中打开` (`t("agent.openInCodeZ")`, `[data-testid="review-open-in-codez"]` and `[data-testid="preview-open-in-codez"]`).
   - Passes `{ taskId, worktreePath, filePath }` to `WorkZWorkspace` -> `App.tsx`.

2. **Worktree Binding & Return Pill (`src/App.tsx`, `src/App.css`)**:
   - `App.tsx` tracks `codezTaskBinding` (`{ taskId, worktreePath, originProjectDir }`) and temporarily binds `CodeZWorkspace` and `IdeSidePanel` (`projectDir={codezTaskBinding?.worktreePath ?? projectDir}`) so the file tree, editor, terminal, and search operate directly on the isolated worktree.
   - Displays a top handoff pill (`[data-testid="worktree-handoff-bar"]`) showing the active task ID and worktree path, with a one-click `返回 WorkZ 对话` (`[data-testid="worktree-handoff-return"]`) button that restores the user to the original `WorkZ` task conversation.

## Verification

- `.\node_modules\.bin\vitest.cmd run src/workspaces/workz/AgentTaskReview.test.tsx` -> **15 passed**
