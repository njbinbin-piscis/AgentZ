# R10-12 Evidence: WorkZ 4 Starter Scenario Cards & Bottom Bar Simplification

- **Date**: 2026-10-19
- **Task**: R10-12 (`r10-12-workz-starter-and-bar`)
- **Status**: Completed

## Summary of Changes

1. **WorkZ Empty State Starter Cards (`src/workspaces/workz/WorkzEmptyState.tsx`, `src/workspaces/workz/index.tsx`, `src/workspaces/workz/Agent.css`)**:
   - Created `WorkzEmptyState` presenting 4 high-frequency scenario cards (`files`, `docs`, `web`, `code` via `WORKZ_STARTER_SCENARIOS`).
   - Clicking a card populates the composer with a structured prompt and enables isolated sandbox mode (`isolate = true`) when recommended.
2. **Bottom Control Bar Simplification (`src/workspaces/workz/index.tsx`)**:
   - Enabled isolated sandbox by default (`localStorage.getItem("agentz-workz-isolate") !== "0"`).
   - Collapsed the `working vs commit` dropdown and raw `HEAD` SHA reference input behind a secondary toggle (`t("workz.sandboxAdvancedToggle")`, `[data-testid="workz-sandbox-advanced-btn"]`) so the primary bar only displays the clean sandbox status pill.

## Verification

- `vitest run src/workspaces/workz/WorkzEmptyState.test.tsx`: **1 passed**.
