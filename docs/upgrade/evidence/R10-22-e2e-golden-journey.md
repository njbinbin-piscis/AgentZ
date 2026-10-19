# R10-22 Evidence: Cross-View Golden User Journey E2E Suite & CI Integration

- **Task ID**: `R10-22` (`r10-22-e2e-golden-journey`)
- **Date**: `2026-10-19`
- **Status**: `PASSED`

## 1. Summary of Changes

1. **Cross-View Golden User Journey E2E Suite (`e2e/agentz.spec.tsx`)**:
   - Exercises the 5-stage 1.0 golden user journey end-to-end:
     - **Stage 1**: First-run 3-step `OnboardingModal` wizard (`onboarding-step-1` -> `onboarding-step-2` -> `onboarding-step-3` -> `onboarding-finish-btn`).
     - **Stage 2**: `WorkzEmptyState` starter scenario selection (`workz-starter-docs`).
     - **Stage 3**: `AgentFilePreview` editable preview canvas (`preview-mode-edit` -> `preview-edit-textarea` -> `preview-save-btn`), automatic human edit ledger capture (`consumeUserRecentEdits` + `<user_recent_edits>` prompt injection block), `preview-continue-with-edits`, and `preview-open-in-codez` handoff.
     - **Stage 4**: `CodeZ` dirty buffer conflict guard (`EditorConflictTracker` conflict detection on concurrent disk write + `resolveConflictAcceptDisk`).
2. **CI & Release Workflow Integration (`package.json`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`)**:
   - Added `"test:e2e": "vitest run e2e/agentz.spec.tsx"` to `package.json`.
   - Wired `npm run test:e2e` and `node scripts/perf-harness.mjs` into `.github/workflows/ci.yml` and `.github/workflows/release.yml`.

## 2. Verification Results

- `npm run test:e2e`: `1 passed (1)` (`397ms`).
