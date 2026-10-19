# R10-05: CodeZ Dirty Buffer Conflict Guard Evidence

- **Task ID**: `R10-05`
- **Date**: 2026-10-19
- **Status**: `done`

## Summary of Changes

1. **Conflict State Machine Extensions (`src/workspaces/codez/editorConflict.ts`)**:
   - Added `DiskConflictInfo` and conflict lifecycle helpers (`getConflict`, `recordConflict`, `resolveConflictKeepMine`, `resolveConflictAcceptDisk`) to `EditorConflictTracker`.
   - When an external disk modification is detected for a dirty buffer (`tab.isDirty && !captured.allowDiscard`), `settleRead` returns `status: "conflict"` and records the conflict snapshot without overwriting the user's unsaved buffer.

2. **Non-Blocking Conflict Banner in CodeZ (`src/workspaces/codez/CodeEditor.tsx`, `src/workspaces/codez/FileViewer.tsx`, `src/workspaces/codez/index.tsx`, `src/workspaces/codez/IDE.css`)**:
   - Replaced the blocking `window.confirm` in `scheduleTabReload` with non-destructive background read settlement via `EditorConflictTracker`.
   - Mounted the top non-blocking conflict banner (`[data-testid="editor-conflict-banner"]`) above the editor with three actions:
     - **View Diff (`[data-testid="conflict-view-diff"]`)**: Opens a side-by-side Diff tab (`diff:conflict:<path>`) comparing disk content vs. local buffer.
     - **Accept Disk Version (`[data-testid="conflict-accept-disk"]`)**: Explicitly discards the dirty buffer in favor of the disk content and updates the SHA-256 stamp.
     - **Keep My Edits (`[data-testid="conflict-keep-mine"]`)**: Preserves the user's dirty buffer and updates the expected disk stamp so subsequent `Ctrl+S` saves succeed cleanly.

## Verification

- `.\node_modules\.bin\vitest.cmd run src/workspaces/codez/editorConflict.test.ts src/workspaces/codez/editorRecovery.integration.test.ts` -> **17 passed (2 test files)**
