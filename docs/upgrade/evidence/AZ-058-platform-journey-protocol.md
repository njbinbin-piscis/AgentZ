# AZ-058 macOS/Linux journey protocol (candidate, 2026-10-18)

This is a reusable test card, not a platform result. Run it separately on an owned macOS test account and Linux desktop session after AZ-056 and AZ-057 have an accepted candidate. CI compilation and cross-target typechecking cannot fill any runtime receipt below. Do not publish a bundle to obtain a test artifact; use a locally built, frozen candidate with recorded SHA-256.

For each platform, copy the card below into a dated evidence record. Record commands, exit codes, logs/screenshots with timestamps and hashes, and the test operator. Keep test credentials out of evidence. A failed or skipped step stays failed/skipped with a reason; it is never converted into a pass because a build succeeded.

| Field | Required receipt |
| --- | --- |
| Identity | OS/distribution and version, architecture, desktop/session type, machine class, operator, UTC start/end |
| Candidate | source commit plus dirty-state declaration, bundle path/hash, bundled Node version, installed app version, model/provider version and test account kind |
| Preconditions | AZ-056 lifecycle candidate and AZ-057 resource limits/thresholds; fresh test profile and backed-up preexisting data, if any |
| Install/start | Package verification, install command/result, first launch/window ready time, logs, permissions and first-run state |
| Task isolation | Two distinct task/environment bindings; run and reopen each, verify correct cwd, plan/run identity, file list and artifact bytes with a cross-task negative check |
| Tool termination | Start a long-running tool with a child process; cancel the bound task, inspect the process group/session and durable cancellation state after timeout, with surviving PIDs attributed |
| Failure/recovery | Interrupt one turn or kill the app at a specified checkpoint; reopen, reconcile attempt state, verify no duplicate external write and no lost local artifact |
| Secrets | Exercise the platform adapter with temporary test credentials: available, unavailable/locked, revoke, corrupt reference, and migration/restore behavior; record cleanup |
| Uninstall/rollback | Uninstall or remove the test build, document retained data policy; perform only the supported restore-to-new-location path when AZ-063 evidence permits it |
| Differences | Record each deviation from Windows or the other platform with issue ID, severity, owner, reproduction and support-matrix change proposal |

Stop and preserve logs if there is environment crossover, unrecoverable data loss, plaintext secret fallback, authorization bypass, or an unknown external action result. Do not broaden the support matrix or mark M7 passed until both native platform cards are complete and independently reviewed. AZ-077 and AZ-078 credential acceptance remain separate, even if this card exercises their paths.

Template outcome for each row: `pass | fail | skipped | blocked`, plus observation, evidence path/hash, and follow-up issue. An empty field is not a pass. Actual macOS/Linux sessions, accounts, signing identity, real model endpoint, and frozen candidate are still unavailable in this workspace.
