# R10-14 Evidence: First-Run 3-Step Onboarding Modal & Beta Experimental Gate

- **Date**: 2026-10-19
- **Task**: R10-14 (`r10-14-onboarding-and-beta-gate`)
- **Status**: Completed

## Summary of Changes

1. **First-Run 3-Step Onboarding Modal (`src/components/OnboardingModal.tsx`, `src/App.tsx`)**:
   - Created `OnboardingModal` guiding first-time users through:
     1. Configuring & verifying LLM Provider API Key (`[data-testid="onboarding-step-1"]`).
     2. Choosing the primary workspace mode (`WorkZ` vs `CodeZ`, `[data-testid="onboarding-step-2"]`).
     3. Launching the first task (`[data-testid="onboarding-step-3"]`).
   - Automatically triggers on startup when `!isOnboardingDone() && !hasConfiguredModel` and can be skipped or completed cleanly.
2. **Beta Experimental Gate (`src/workspaces/workz/index.tsx`, `src/i18n/zh.ts`, `src/i18n/en.ts`)**:
   - Marked multi-agent Swarm / Workflow team selectors as `(Beta)` and collapsed them under a secondary toggle (`t("workz.betaAdvancedToggle")`, `[data-testid="workz-beta-swarm-toggle"]`) by default so new users are not overwhelmed by experimental multi-agent states.

## Verification

- `vitest run src/components/OnboardingModal.test.tsx`: **1 passed**.
