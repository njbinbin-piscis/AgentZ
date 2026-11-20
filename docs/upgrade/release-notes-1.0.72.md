# AgentZ 1.0.72 release notes

Release date: 2026-11-20. See the 1.0.72 entry in `CHANGELOG.md`.

## Fixed

- **Agent self-inspection of `.agentz/` (#28).** Read-only access to the repo's own memory/plans/screenshots no longer requires approval; the previous whitelist blocked it while shell could reach it anyway (inconsistent posture).
- **`code_run` pre-launch syntax detection (#29).** PowerShell constructs and multi-line `python -c` under cmd.exe now fail fast with an actionable hint instead of a silent exit 255.
- **`file_search` look-around rewrite hint (#30).** Regex-engine errors for unsupported look-around now explain how to rewrite the pattern.
- **Browser observable/cancellable during navigate (#31).** Cached-frame fallback for read-only polling, lock-independent close, busy errors name the lock holder.
- **`memory_store search` fuzzy fallback (#34).** Shared-token matching when the exact phrase misses.
- **`git stash drop/clear` inspect-then-confirm (#35).** Specific warning naming `git stash list` / `git stash show -p <stash>`; headless refusal carries the guidance. Confirmation stays mandatory.
- **Embedded browser "set viewport failed" after self-closing pages (#36).** robotz probes cached Page handles with a real CDP round-trip and recreates dead ones; live regression tests red→green.

## Changed

- CI failures surface as check-run annotations (private-repo logs/artifacts require auth); vendored robotz crates inherit the workspace lint policy again.

## Known limitations

- #27 popup adoption and #36 stale-page recovery are verified by live Chromium tests but not yet hand-checked in the running app.
- #32 CI diagnostics remain annotation-only until a read-only CI token is configured.