# R10-23 Evidence: Redacted Diagnostic Bundle Export & Top 8 VS Code Extension Compatibility

- **Task ID**: `R10-23` (`r10-23-diagnostics-and-extensions`)
- **Date**: `2026-10-19`
- **Status**: `PASSED`

## 1. Summary of Changes

1. **Redacted Diagnostic Bundle Export with Active Context Summary (`src-tauri/src/diagnostics.rs`, `src/services/tauri/diagnostics.ts`, `src/workspaces/codez/settings/DiagnosticsSection.tsx`)**:
   - Extended `DiagnosticExportPreview` and `DiagnosticManifest` with `DiagnosticContextSummary` (`latest_run_id`, `latest_task_id`, `latest_environment_id`, `failure_count`).
   - Updated `DiagnosticsSection.tsx` to display the active context summary (`[data-testid="diagnostics-context-summary"]`) before exporting a local redacted copy (`[data-testid="diagnostics-export-btn"]`).
   - Added bilingual i18n keys (`settings.diagnosticsContextSummary`) in `src/i18n/en.ts` and `src/i18n/zh.ts`.
2. **Top 8 VS Code Extension Compatibility Archive (`docs/extension-compatibility-1.0.md`, `src/workspaces/codez/settings/DiagnosticsSection.test.tsx`)**:
   - Documented the 14 implemented `MainThread*` RPC surfaces (`HOST_VSCODE_VERSION = 1.96.0`) and the compatibility verdicts for the Top 8 extensions:
     - **Full Support (5)**: Prettier (`esbenp.prettier-vscode`), ESLint (`dbaeumer.vscode-eslint`), Markdown All in One (`yzhang.markdown-all-in-one`), YAML (`redhat.vscode-yaml`), Tailwind CSS IntelliSense (`bradlc.vscode-tailwindcss`).
     - **Degraded Support (3)**: Python (`ms-python.python`), rust-analyzer (`rust-lang.rust-analyzer`), GitLens (`eamodio.gitlens`), with explicit documentation of degraded proposed APIs (`terminalShellIntegration`, `scmHistoryProvider`) and `compatStore.missingApis` observability.

## 2. Verification Results

- `npx vitest run src/workspaces/codez/settings/DiagnosticsSection.test.tsx src/i18n/i18n.test.ts`: `5 passed (5)`.
- `cargo test -p agentz-desktop --lib diagnostics::tests`: `15 passed; 0 failed`.
