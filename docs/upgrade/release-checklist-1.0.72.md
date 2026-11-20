# AgentZ 1.0.72 release checklist

2026-11-20. Based on 1.0.71: session-friction fix batch #28–31, #34–36 (approval whitelist, code_run pre-launch detection, regex rewrite hint, browser busy lock, memory search fallback, stash inspect-then-confirm, stale-page recovery).

## Release gate

- [x] Version labels are 1.0.72 (package, lockfiles, Cargo, Tauri, README, RELEASE).
- [x] `CHANGELOG.md` and `docs/upgrade/release-notes-1.0.72.md` are updated.
- [x] `cargo clippy --offline --workspace --all-targets -- -D warnings` clean at cf46c7b.
- [x] Kernel suite 517 passed (incl. new #35 tests red→green: gate 19/19, headless refusal guidance).
- [x] Fix-batch suites re-verified at their commits: approval/file_list/file_search (#28), code_run 12/12 (#29), file_search 5/5 (#30), browser 9/9 (#31), store::db 27/27 (#34), browser live 5/5 (#36).
- [x] `node scripts/upgrade-version-check.mjs` re-run after docs — passed (1.0.72 labels agree).
- [x] `node scripts/upgrade-plan.mjs validate` — VALID (83 tasks, 11 milestones).
- [x] `node scripts/upgrade-release-gate.mjs` with GITHUB_REF=refs/tags/v1.0.72 — gate open.
- [ ] #27 popup adoption and #36 stale-page recovery not hand-checked in the running app.

## CI

- [x] The `v1.0.72` Release workflow produces all installers (Linux, macOS, Windows x64 + ARM64) — run 44780484734 completed **success** 2026-11-20T01:56Z (~52 min); release "AgentZ v1.0.72" published with all **12 assets** (rpm ×2, AppImage ×2, deb ×2, dmg ×2, setup.exe ×2, app.tar.gz ×2).
- [x] Main-branch CI green at `d1a83b0` (run 44785555593, Frontend + Desktop host both success). The release commit `1db26d2` initially failed CI: two `approval_execution` integration tests still asserted the pre-#28 behavior (`.agentz` search root denied). Root cause: `validate_search_path` canonicalized before the `.agentz` exemption, so a missing `.agentz` dir leaked a raw OS error; fixed in `d1a83b0` (exemption moved ahead of canonicalize, test expectations updated). Release artifacts were built from the tag and are unaffected (test-only change). Local note: git 2.24 lacks `init -b`, so two unrelated git-fixture tests fail locally but pass on CI.