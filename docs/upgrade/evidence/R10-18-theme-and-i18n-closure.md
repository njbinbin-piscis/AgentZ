# R10-18 Evidence: Theme Semantic Tokens & i18n Zero-Hardcoded Closure

- **Date**: 2026-10-19
- **Task**: `R10-18` — Converge hardcoded dark-mode hex colors to semantic CSS variables (`var(--...)`) and eliminate all hardcoded Chinese strings from production TSX components (`0` matches).

## 1. i18n Full Closure (`0` Hardcoded Chinese Strings in TSX)

Migrated all remaining hardcoded Chinese strings across 9 production `.tsx` files into `src/i18n/zh.ts` and `src/i18n/en.ts` with 100% key parity:
- `src/workspaces/workz/WorkzEmptyState.tsx`: Replaced `promptZh`/`promptEn` with `promptKey` (`workz.starterFilesPrompt`, `workz.starterDocsPrompt`, `workz.starterWebPrompt`, `workz.starterCodePrompt`).
- `src/workspaces/workz/index.tsx`: Replaced inline Chinese continuation prompt with `t("workz.continueWithEditsPrompt", { path })`.
- `src/workspaces/codez/CodeEditor.tsx`: Replaced LSP status labels with `t("ide.lspRecovering")`, `t("ide.lspConnecting")`, and `t("ide.lspUnavailable")`.
- `src/workspaces/codez/Terminal.tsx`: Replaced bilingual alert with `i18n.t("ide.terminalStopFailedAlert")`.
- `src/workspaces/codez/resources/SkillsInstalledView.tsx`: Replaced SHA-256 verification status, requested tools prefix, and version history prefix with `skills.*` i18n keys.
- `src/components/TaskReminderPanel.tsx`: Replaced all reminder labels/options with `taskReminder.*` i18n keys.
- `src/workspaces/codez/AssistantMessagesPanel.tsx`: Replaced channel names and connection status labels with `assistantPanel.*` i18n keys.
- `src/workspaces/codez/settings/AssistantsTab.tsx`: Replaced all IM channel settings headings, status badges, WeChat QR login states, and buttons with `assistantPanel.*` i18n keys.
- `src/components/chat/InteractiveCard.tsx`: Updated `defaultValue` fallback in `chat.planSuggestCountdown`.

## 2. CSS Semantic Theme Token Convergence

- `src/workspaces/codez/IDE.css`: Converted `.ide-assistant-panel`, `.ide-assistant-line--*`, `.ide-assistant-prompt`, `.ide-assistant-input-row`, and `.ide-assistant-input` from hardcoded dark hex colors (`#14141c`, `#e8e8f0`, `#9be4ff`, `#9585ff`, `#ff8585`) to semantic tokens (`var(--bg-elev)`, `var(--text-primary)`, `var(--accent)`, `var(--accent-hover)`, `var(--error)`, `var(--border)`).
- `src/workspaces/codez/Markdown.css`: Converted `.agentz-markdown-fallback` background `#0c0c12` to `var(--code-bg, #0d1117)`.
- `src/extensions/notebook/notebook.css`: Converted `.agentz-nb*` rules from hardcoded dark hex colors (`#1e1e1e`, `#252526`, `#2b2b2b`, `#181818`, `#d4d4d4`) to `var(--bg-primary)`, `var(--bg-secondary)`, `var(--bg-tertiary)`, `var(--bg-hover)`, `var(--code-bg)`, `var(--border)`, `var(--text-primary)`, and `var(--text-secondary)`.

## 3. Automated Gate Verification

Added a permanent scan test in `src/i18n/i18n.test.ts` (`has zero hardcoded Chinese characters in production TSX files (R10-18)`) that recursively scans all non-test `.tsx` files under `src/` for `/[\u4e00-\u9fff]/` and asserts `0` violations.

- `npx vitest run src/i18n/i18n.test.ts src/workspaces/workz/WorkzEmptyState.test.tsx` -> `2 passed (4 tests)`.
