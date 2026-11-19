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

- [x] The `v1.0.71` Release workflow produces all installers (Linux, macOS, Windows x64 + ARM64) — run 44440919423 completed **success** 2026-11-19T02:35:58Z (~37 min); release "AgentZ v1.0.71" published with all **12 assets** (rpm ×2, AppImage ×2, deb ×2, dmg ×2, setup.exe ×2, app.tar.gz ×2).
- [x] Main-branch CI clippy failure on the release commit (`items_after_test_module` in vendored robotz snapshot.rs, deny-warnings) fixed in `6d73208` and pushed; local `cargo clippy --workspace --all-targets` clean, robotz-browser tests 8/8. Release artifacts were built from the tag and are unaffected (the lint is test-module ordering only).