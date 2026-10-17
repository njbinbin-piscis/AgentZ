# AZ044 current App and translation limited independent review

2026-10-10. Reviewer: Codex current_source_review. Read-only independent inspection of current src/App.tsx, src/i18n/en.ts and src/i18n/zh.ts in the existing bounded AZ017/AZ036 navigation and task-environment explanation scopes. Read AZ-044-current-drift.json, previous AZ-041-shared-review.md and relevant AZ036/AZ017 review history. Applicable AGENTS search from this review sequence found none. Only this new evidence file is written; no source, ledger or existing pinned evidence is changed. No tests, typecheck, native IPC, GUI or model execution was run by this reviewer.

Current raw SHA256:
- src/App.tsx: 81d99ad3755dfaff43bd74343ba2bf0cc21bf2e9df5960358745646bba78f471
- src/i18n/en.ts: 95c75f1c919cf60a40217959be3d4710968b3329ddb4aaa6de80d9f578ec893b
- src/i18n/zh.ts: 14fdeba7b7caedb92907893010532009b716a5b94e6b30fa200a83b2027e3e29

Previous pins are App 79fd684ddaad63cb573db8740358aff68a3a08a818f573325a8d41361db2f465, en c718a52cd27c3b1f6e2dc0950fd6c4d3992ab6af9ae8d6b5837592670d69493e and zh bbca86c830405bc983d8297bf000d34dfeea9975f85e6693ac22f2be739f8984. Exact reviewed dirty baseline bytes are unavailable. No historical semantic equivalence, exact delta, authorship or newline-only explanation is claimed. This is a fresh limited current-consumer inspection.

## Current navigation guards

App stores a synchronous live editor getter and passes its stable registration callback into the continuously mounted CodeZ pane. Project switch and close assemble the live editor snapshot rather than relying solely on the debounced editor patch. Missing getter fails preflight; the helper clones snapshots, refuses dirty buffers, checks current route/snapshot across confirmation and successful persistence, and returns blocked on persistence failure. App checks ready before terminal destruction, scope revocation and route mutation. Busy mutex guards ordinary navigation; picker acquires the same mutex before awaiting native scope selection and holds it through adoption/rejected-scope cleanup, with the navigationOwned inner switch leaving release to the picker finally.

App rechecks current project and complete snapshot after terminal destruction and scope revocation before committing the route. These checks reject late edits and route changes; they cannot undo terminal cleanup or a completed revocation if subsequent checks fail. Cleanup failure is caught and prevents route success. openProjectInCodeZ only changes mode if awaited switchProject succeeds, including same-project success. Same normalized project returns early without a destructive switch. Scope acquisition/revocation calls use opaque existing service handles; this wiring does not grant authority from a path or demonstrate native lifecycle acceptance.

Historical workspace restoration attempts host restoreProjectScope for local projects. Failed scope restoration can retain editing restoration but does not construct a verified durable-plan authority in App; authority-gated actions remain responsible for rejecting unavailable scopes. Existing running-count close confirmation queries chat_running_count, cancels when requested, persists, then acknowledges close; that observation is not proof of complete process-tree cancellation or stable atomic Run state. These retained limitations are not newly certified by source inspection.

## Current relevant translation keys

English and Chinese app.saveBeforeProjectSwitch/Close require saving or closing edited files and unsaved drafts. projectSwitchSaveFailed/projectCloseSaveFailed explicitly describe cancelled navigation on workspace-preservation failure. App only displays these after corresponding actual blocked reasons; strings do not bypass guards or turn a failed operation into success.

agent.isolateHint in both tables separates Git worktree/branch editing isolation from independently controlled system-tool permissions. taskEnvironmentMissing explicitly identifies historical tasks without verified isolated binding, preserves history and requires a new task or explicit original-workspace recovery instead of silent directory movement. isolateOn/Off are configuration labels, not permission receipts or execution evidence. Relevant strings are plain data; no native execution or scope authority is created by their wording. The remainder of translation tables and localization layout are outside this review.

## Limited conclusion

No additional blocking defect was identified in the current inspected AZ017/AZ036 navigation, opaque-scope integration and relevant explanation-key consumers. Root may atomically refresh all affected consumers to these exact current hashes, preserving historical reviews and appending this evidence with the separate current index review. Any subsequent source change invalidates this conclusion's fingerprint. Full App equivalence, all Run observation/authorization semantics, actual native permission tests, whole-table translation equality, desktop roundtrip, model, rollback and full milestone acceptance are not established here. Previously documented partial cleanup/revocation effects and backend authority limitations remain open rather than being counted complete.
