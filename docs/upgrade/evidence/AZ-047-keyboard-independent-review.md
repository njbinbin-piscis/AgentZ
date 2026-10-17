# AZ047 bounded dock keyboard independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only inspection of four current dock/navigation files. Only this new evidence written; no source, ledger or pinned evidence changes. Root reports three controlled tests session65776 passed and earlier full TypeScript72759 exit0; final type/lint43132/63796 was pending at request. Reviewer ran no tests, typecheck, GUI or assistive technology checks.

Reviewed raw SHA256:
- src/workspaces/codez/BottomPanel.tsx: 8191f78ed0277af4c260511c050d7960476921992f14777856ff4965d8fa98d0
- src/workspaces/codez/tabNavigation.ts: 5bb9a2abc43d341f0b44904a8bd139ecb31f10ffe4dbeb5f52fccff80ae421fe
- src/workspaces/codez/BottomPanel.keyboard.test.tsx: 710f722953e9e621a99dd56352812219034a1a2f01c4b137886ada53ed221bda
- src/workspaces/codez/tabNavigation.test.ts: f4d04333ebe7802b59316732e5f7366138694bbf642b8a64025187dc198c46af

The helper wraps left/right across the supplied visible list, Home/End select boundaries and ordinary keys/empty lists return null. Removed current IDs use deterministic index-zero fallback. BottomPanel derives current from available tabs, falling back to terminal if the chosen contribution disappears. Only current tab has tabIndex0; others have -1. Supported key handlers prevent default, invoke the controlled onTabChange callback and focus the target registered DOM ref. Alt/Ctrl/Meta combinations remain untouched. Native button activation supplies Enter/Space; browser Tab is not intercepted. Controlled parent must adopt requested activeTab to update selected state; component does not mutate extension state.

useId yields instance-local tab/pane IDs. All tabs control the same dynamic panel; that single panel is labelled by the current tab. Current content is conditionally rendered, with the persistent terminal subtree display-hidden outside terminal selection. This is a deliberate shared dynamic panel, not seven separate tabpanels. Output data remains passive pre text; this change adds no model execution or authority. Dock close button is also a focusable non-tab inside tablist and tablist label remains the terminal translation for the broader dock; full screen-reader semantics and UX wording require actual evaluation.

The component fixture uses the actual BottomPanel with mocked children/store/translation and mocked focus refs. It verifies one wrapped ArrowLeft target/focus, selected/panel relationship after controlled activeTab update, and Ctrl-modified refusal. Helper fixtures verify both wraps, Home/End, absent-list and ordinary-key behavior. Translation mock switches synthetic en:/zh: prefixes; it does not load either real dictionary, render real bilingual labels, evaluate clipping or prove a bilingual desktop journey. The fixture mocks Terminal entirely.

## Real terminal focus integration gap

Existing TerminalSession visibility effect schedules term.focus after60ms when active/panelVisible become true. Keyboard-selecting terminal from another dock tab therefore can first focus its tab via BottomPanel and subsequently move focus into xterm. Current mock-based fixture cannot detect this. This limits the complete roving-focus keyboard journey and was reported to root. Native focus behavior should be suppressed for keyboard tab selection or explicitly tested/designed before declaring full dock keyboard accessibility; the current immediate controlled routing slice alone does not resolve it.

Contribution removal does not explicitly relocate focus from a removed button, and hiding the dock does not restore focus to its trigger here. CSS/native hidden behavior, focus outlines, browser automatic button behavior, terminal text input shortcuts and actual screen-reader use remain outside these tests. No broad localization/accessibility acceptance follows from three controlled tests.

No blocking source defect found in the restricted immediate controlled tab target/ARIA publication slice. It may be recorded as a bounded candidate after exact final validation, with the delayed terminal focus and real focus-removal/hide journeys remaining open; full AZ047 must not be marked complete. Any change to these four files invalidates the stated fingerprint and needs follow-up review.
