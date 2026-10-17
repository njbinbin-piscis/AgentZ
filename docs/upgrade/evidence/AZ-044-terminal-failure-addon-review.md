# AZ044 navigation helper failure propagation add-on review

2026-10-10. Reviewer: Codex current_source_review. Independent read-only review limited to two terminal helper changes, the added helper fixture and existing App callers. No source/ledger edits or test/native/GUI execution by reviewer. Historical final review remains valid version history, not the hash of this newer Terminal revision. Root session47333 was running at review request; no result is asserted here.

Current raw SHA256:
- src/workspaces/codez/Terminal.tsx: 87d4ac873b7c34f295a443ac6c5797d7e617425daf1f8e3001476434781f248d
- src/workspaces/codez/Terminal.lifecycle.test.tsx: 08fac00a9f1b23cb98275f3fadd2833f7549236d7761cdff012ee9259cfec4f3
- src/App.tsx: 81d99ad3755dfaff43bd74343ba2bf0cc21bf2e9df5960358745646bba78f471

confirmTerminalCloseOnProjectChange now returns false when terminalCount rejects instead of assuming safe navigation. preflightWorkspaceNavigation treats false confirmation as cancelled; App switch/close return before destructive cleanup or route changes when preflight is not ready. closeProject's wrapper also returns false immediately before its browser confirmation. Zero/nonpositive successful counts and explicitly confirmed positive counts retain their existing meaning; runtime count shape/NaN validation is not added by this change.

destroyAllTerminals now awaits terminalDestroyAll without swallowing rejection. Both App callers await it within their outer try/catch, before revokeProjectScope and setProjectDir. Thus an explicit destroy-all error reaches catch and prevents subsequent scope revocation and project route commit. switchProject returns falsy after catch, so openProjectInCodeZ does not change mode and pickFolder can reject/revoke the newly selected scope. Both callers release the navigation mutex in finally. This is rejection propagation, not a rollback: partial backend destruction preceding an error remains possible and cannot be restored by the catch.

The additional fixture invokes the actual exported helper functions against rejected count/destroyAll mocks and checks false/rejection. It verifies the concrete helper contract, not a mounted App navigation or native failure journey. No test result is inferred merely from the fixture's presence.

No blocking source issue found within these two failure-propagation changes. Once owner verifies the exact final sources, this add-on can accompany the bounded candidate freeze and supersede the previous review's count-error/destroy-all-swallowing limitation specifically. Host authority, uncertain per-tab failure reconciliation, timeouts, retained poisoned IDs, cross-renderer/OS descendants, native response truthfulness and GUI acceptance remain open. These improvements do not certify native terminal absence or atomic process cleanup.
