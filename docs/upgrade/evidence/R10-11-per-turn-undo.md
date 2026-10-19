# R10-11 Evidence: Per-Turn File Undo (`journalUndoTurn`) Mounted in CodeZ & WorkZ

- **Date**: 2026-10-19
- **Task**: R10-11 (`r10-11-per-turn-undo`)
- **Status**: Completed

## Summary of Changes

1. **CodeZ Per-Turn File Undo (`src/workspaces/codez/AssistantMessageList.tsx`, `src/workspaces/codez/AssistantPanel.tsx`)**:
   - Added `onUndoTurn?: (turnId: string) => void | Promise<void>` to `AssistantMessageListProps`.
   - Rendered `[撤销本轮文件修改]` (`t("agent.rollbackTurn")`, `[data-testid="undo-turn-<turnId>"]`) beneath inline `FileDiffCard` lists when `diffs.length > 0` and `m.turnId` is present.
   - Wired `undoTurnChanges(turnId)` in `AssistantPanel.tsx` calling `journalUndoTurn(projectDir, sessionRef.current, turnId)`, clearing the turn's diff cards, refreshing the file tree/git status, and surfacing `t("agent.rollbackTurnOk")`.
2. **WorkZ Per-Turn File Undo (`src/workspaces/workz/agentArtifacts.ts`, `src/workspaces/workz/index.tsx`)**:
   - Added `turnId?: string` to `AgentStep` and persisted `res.turn_id` on completed assistant turns.
   - Mounted `[撤销本轮文件修改]` (`[data-testid="workz-undo-turn-<turnId>"]`) on assistant turn steps in `WorkZ`, invoking `journalUndoTurn(projectDir, sid, m.turnId)` and refreshing git/task changes.

## Verification

- `vitest run src/workspaces/codez/AssistantMessageList.test.tsx`: **1 passed**.
