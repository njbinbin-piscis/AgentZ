# 1.0.0 post-signoff test repair (2026-10-19)

Independent validation of commit `327980e` found that `npm test` failed in two test fixtures: `Terminal.lifecycle.test.tsx` loaded the real i18n initializer through `Terminal.tsx` while mocking only `useTranslation`, and `IdeStatusBar.test.tsx` supplied provider `name` fields although the status bar reads `label` from `LlmProviderConfig`.

The terminal fixture now mocks the i18n module used by the component. The status bar fixture supplies `label` fields. Product code and roadmap status were not changed.

Verification after the repair:

- `npx vitest run src/workspaces/codez/Terminal.lifecycle.test.tsx src/workspaces/codez/IdeStatusBar.test.tsx`: 2 files, 8 tests passed.
- `npm test`: 52 files, 296 tests passed.
- `npm run typecheck`: passed.
- `node scripts/upgrade-plan.verify.mjs`, `node scripts/upgrade-plan.mjs validate`, and `node scripts/upgrade-version-check.mjs`: passed.
- `cargo test -p agentz-host --test kernel_contract`: 4 passed.
- `cargo test -p agentz-task-state`: 165 unit tests and 5 integration tests passed.
- `cargo test -p agentz-desktop --lib`: 375 passed, 7 ignored. The ignored cases include local Chromium, live SSH/remote services, and opt-in SQLite performance probes; they were not accepted by this run.

These local checks do not constitute native desktop, model-provider, cross-platform, external-service, signing, or rollback acceptance. ADR-006 deferred items retain their existing status.
