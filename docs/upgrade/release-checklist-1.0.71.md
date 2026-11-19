# AgentZ 1.0.71 release checklist

2026-11-18. Based on 1.0.70: built-in browser popup-target adoption (#27), vendored RobotZ, stop-test Storage stub fix.

## Release gate

- [x] Version labels are 1.0.71 (package, lockfiles, Cargo, Tauri, README, RELEASE).
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.71.md` are updated.
- [x] `npx tsc --noEmit` passed (BrowserPanel.tsx, browser.ts clean).
- [x] `cargo clippy --offline --lib` clean; browser unit tests 7/7 and live tests 3/3 pass (incl. new `panel_link_clicks_follow_same_tab_and_blank_targets_live`, red→green).
- [x] Frontend regression: BrowserPanel.tabs / browserTab tests pass; #23–26 suites re-verified at v1.0.70 (10 files / 64 tests).
- [x] `node scripts/upgrade-version-check.mjs` re-run after docs — passed.
- [x] `node scripts/upgrade-plan.mjs validate` — VALID (83 tasks, 11 milestones).
- [x] `node scripts/upgrade-release-gate.mjs` with GITHUB_REF=refs/tags/v1.0.71 — gate open.
- [x] robotz-browser switched to a direct path dep (git dep + patch still required the git rev offline); `cargo check --offline` and browser tests re-passed at 1.0.71.
- [ ] Popup adoption not hand-checked in the running app on a real `_blank`-link site.

## CI

- [ ] The `v1.0.71` Release workflow produces all installers (Linux, macOS, Windows x64 + ARM64).