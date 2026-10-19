# R10-09 Evidence: WorkZ Editable Document Canvas (`AgentFilePreview`)

- **Task ID**: `R10-09` (`r10-09-editable-preview-canvas`)
- **Date**: 2026-10-19
- **Status**: Completed

## 1. Changes Implemented

1. **Three-Mode Preview/Edit/Diff Canvas (`src/workspaces/workz/AgentFilePreview.tsx`, `src/workspaces/workz/Agent.css`)**:
   - Upgraded `AgentFilePreview` to support `Preview` (`[data-testid="preview-mode-preview"]`), `Edit` (`[data-testid="preview-mode-edit"]`), and `Diff` (`[data-testid="preview-mode-diff"]`) modes for text/markdown/code artifacts.
   - Added in-place editing (`[data-testid="preview-edit-textarea"]`), `Ctrl+S` / button save (`[data-testid="preview-save-btn"]`) via `ideApi.writeFile`, automatic `recordUserRecentEdit` accounting, and a footer `[基于最新修改继续对话]` (`[data-testid="preview-continue-with-edits"]`) button wired to `WorkZ`'s composer in `src/workspaces/workz/index.tsx`.
2. **i18n Keys (`src/i18n/zh.ts`, `src/i18n/en.ts`)**:
   - Added `agent.previewEdit`, `agent.previewSave`, `agent.previewSaving`, `agent.previewSaved`, and `agent.continueWithEdits`.

## 2. Verification Commands & Outputs

- `.\node_modules\.bin\vitest.cmd run src/workspaces/workz/AgentFilePreview.test.tsx`:
  - `1 passed (1)` — verified switching to Edit mode, editing text, saving via `ideApi.writeFile`, recording `userRecentEdit`, and triggering `onContinueWithEdits`.
