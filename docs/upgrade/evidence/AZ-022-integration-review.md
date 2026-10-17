# AZ-022 integrated Windows recovery candidate review

2026-10-09. Independent read-only review of current vendor shell.rs, code_run.rs, proc_windows_sandbox.rs and tool_recovery.rs. No Cargo/GUI/UAC/OS fault execution by this reviewer. Root Cargo session 74168 was underway; no outcome inferred.

## Limited result

No blocking integration defect found in the inspected paths. Preflight parameter/path/platform/elevation refusals and register refusal attach NotStarted with NotApplicable cleanup before worker setup. Runner/JoinError paths attach WorkerFailed/Unknown rather than guessing a child never launched or cleanup succeeded. Shell elevated=true is Unsupported, not a synthetic UAC denial/no-response. No unsandboxed retry is introduced.

A successful runner constructs confirmed metadata only after root/job termination, both capture joins and WorkspaceAclLease.finish. Both stdout and stderr joins are now evaluated and ACL finish attempted before selecting any error; a stdout failure no longer skips the other join or explicit ACL cleanup. Multiple simultaneous errors still return only the first error as primary, with conservative aggregate Unknown metadata; this loses diagnostic detail but does not falsely confirm cleanup. Earlier setup/termination errors remain Unknown and rely on RAII without claiming cleanup evidence.

Raw per-stream counts are checked additions of bytes actually read through EOF, independent of the retained Vec and lossy UTF-8/display length. Each stream is classified Complete at exact cap and Truncated only when observed bytes exceed retained bytes. Capture continues draining after cap. OutputCapture validates retained/observed relationships. Shell records smart truncation and repeated-line collapse; code_run records actual trim threshold. Presentation flags are monotonic and do not replace raw counts or exit/cancel/timeout outcome. Timeout/cancel branches omit output presentation but still attach raw capture metadata.

RecoveryReport.attach preserves existing error=true and sets error=true for every non-Completed outcome. Therefore ordinary nonzero command exit now correctly produces tool error rather than an apparent successful result. Current shell/code_run sibling denied-read assertions have already been changed to assert is_error; normal echo remains false. New structured report constructors/fixtures cover error vs success and truncation separately. No test result is claimed here.

Footer is passive content, not permission/automatic retry. It contains bounded operation identity and enum/count observations, not command/arguments/output/error details. Tool output could contain delimiter-looking text; no reviewed code parses such text as authority. A future structured UI parser should never treat arbitrary footer text from stdout as trusted runner evidence.

## Remaining limitations

Runner error lacks per-stage diagnostic aggregation; Unknown is deliberately conservative. Capture joins have no independent bounded join deadline after job-empty confirmation. Real simultaneous reader and ACL fault injection, launch/assignment/resume faults, GUI cancellation, full UAC interaction, cross-volume/low-privilege cases remain unverified. Nonzero result is_error is an intentional behavior change; root should run existing tool tests and refresh AZ021 tools fingerprints. This review does not upgrade AZ021 generic invocation termination to a guarantee for detached unregistered resources.
