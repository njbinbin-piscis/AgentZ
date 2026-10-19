# R10-16 Evidence: Decompose `src/workspaces/codez/index.tsx` & `AssistantPanel.tsx` and Unify Stream Event Normalization

- **Date**: 2026-10-19
- **Task**: R10-16 (`r10-16-codez-split`)
- **Status**: Completed

## Summary of Changes

1. **Decomposed `src/workspaces/codez/index.tsx` (previously ~1,847 lines -> 509 lines)**:
   - Extracted `src/workspaces/codez/useCodezWorkspace.ts` for file tree, open tabs, disk conflict tracking, and layout persistence.
   - Extracted `src/workspaces/codez/useCodezShortcuts.ts` for keyboard shortcuts (`Ctrl+S`, `Ctrl+P`, `Ctrl+Shift+P`) and command palette command definitions.
2. **Decomposed `src/workspaces/codez/AssistantPanel.tsx` (previously ~1,670 lines -> 312 lines)**:
   - Extracted `src/workspaces/codez/useAssistantSession.ts` for session lifecycle, streaming turn events, and composer state.
3. **Unified Stream Context Usage Normalization (`src/services/tauri/runEventProjection.ts`)**:
   - Added `normalizeStreamContextUsage` in `runEventProjection.ts` and wired it into `useAssistantSession.ts`.

## Verification

- `src/workspaces/codez/index.tsx` line count: **509 lines** (`< 650` lines).
- `src/workspaces/codez/AssistantPanel.tsx` line count: **312 lines** (`< 650` lines).
- `tsc --noEmit` & `vitest run src/workspaces/codez/`: **all passed**.
