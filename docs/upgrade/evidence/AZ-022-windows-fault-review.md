# AZ-022 Windows fault and result contract review

2026-10-09. Independent source review only; no new Cargo, GUI, OS fault, provider or UAC execution was performed. Existing Windows test results below are referenced from AZ-034-production-runner.md, not rerun by this reviewer.

## Findings and concrete integration points

1. `vendor/piscis-engine/piscis-kernel/src/proc_windows_sandbox.rs:96` AppContainerOutput provides exit_code/stdout/stderr/timed_out/cancelled but no captured byte counts or truncation flags. `spawn_capture` at 475 continues draining correctly after its limit, but silently discards remaining bytes. Shell smart_truncate cannot identify bytes already discarded. Add per-stream observed/captured byte counts and truncated flags (checked/saturating counts); distinguish original capture clipping from subsequent display clipping. A command printing exactly the cap must not be falsely marked truncated; cap+one must be marked. Test UTF-8 split at byte boundary and both streams concurrently.

2. Errors are unstructured anyhow across launch, process, capture and ACL stages. AssignProcessToJobObject failure at 283 best-effort TerminateProcess ignores result and does not await root termination; ResumeThread failure at 292 best-effort TerminateJobObject ignores result. Cancellation/timeout termination at 311/319, abnormal wait at 338, root wait at 346 and job-empty wait at 362 can return before explicit complete cleanup acknowledgement. RAII kill-on-close/drop does not prove successful tree teardown or ACL restoration. The new AZ-021 receipt conservatively marks runner Err unconfirmed; preserve that behavior. Add a typed failure stage plus cleanup evidence (root/tree/pipes/ACL each Confirmed or Unknown/Failed), preserving primary and cleanup errors rather than substituting generic success.

3. Stdout join at 365 uses early `?`: panic/read error skips explicit stderr join. Job-empty normally closes inherited pipe writers, but an unjoined reader is not an acknowledged pipe completion. Collect both joins before choosing the primary error. Join has no independent bounded deadline; current job-empty prerequisite narrows risk but does not constitute tested bounded behavior on fault paths. Do not mark full cleanup Confirmed when any stage is unknown.

Normal success order is root termination -> job empty -> both capture joins -> WorkspaceAclLease.finish -> profile drop. That is the right structured success boundary. Returned nonzero exit codes are command failures, not cleanup failures. `timed_out`/`cancelled` are reasons, not proof of cleanup. A structured ToolResult contract should carry exit_reason and capture metadata to host/UI while the owned receipt consumes only aggregate cleanup evidence; avoid parsing human-readable messages.

## Existing evidence that should be reused

AZ-034 production evidence records real Windows AppContainer tests for timeout/cancel descendants, sibling read/move denial, network denial, hard-link refusal, ACL null-DACL grant rollback, retained-handle/path replacement, original DACL inheritance state, WRITE_DAC cleanup failure followed by retry, and deterministic same-volume new/moved objects cleanup. It records 353 passed / 1 ignored at its latest cited full run and also a historical parallel Job marker failure retained in the evidence. These results are not proof of new AZ-022 structured output integration or all fault permutations.

## Minimum next validation package

- Reuse `production_runner_cancels_and_reaps_the_job` and root/child marker checks, assert typed cancellation/timeout reason plus tree/pipes/ACL confirmed only after completion.
- Extend existing actual WRITE_DAC cleanup failure fixture to assert typed ACL Failed, preserved earlier process outcome, and AZ-021 drain Failed; retain safe test ACL restoration guard and existing successful retry.
- Pure reader fixture for exact cap/cap+1/UTF-8 multibyte/zero cap, then one real runner producing bounded stdout+stderr excess; ensure drained child exits without hanging and bytes are explicitly clipped.
- Controlled launch/assignment/resume fault seam confined to cfg(test), if necessary, must assert no success/Confirmed and preservation of cleanup Unknown. Do not trigger a real UAC prompt to test rejection.

Shell `elevated=true` refusal occurs before execution (`tools/shell.rs:104`) and its actual negative fixture is at 331/389. Main host registry excludes elevate (`chat_turn.rs:53,1152`). No reviewed runner ShellExecute/runas path requests UAC. Existing code_run permission-denied advice at 357 still suggests elevated privileges despite current restricted policy; recommend changing this wording to the actual unsupported capability rather than suggesting an unavailable recovery action. This is a presentation mismatch, not evidence of an elevation bypass.

No new test script/file was created; no concurrent Cargo invocation. Cross-volume, non-NTFS, low-privilege/multi-user, truly concurrent reparse/ACL mutations and full desktop Stop interaction remain separate unverified acceptance. Existing AZ-021 generic invocation receipts do not attest detached unregistered external resources.
