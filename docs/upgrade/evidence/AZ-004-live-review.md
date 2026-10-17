# AZ-004 live synthetic baseline independent review

2026-10-09. Read-only review of scripts/upgrade-model-baseline.mjs and vendor piscis-kernel/tests/upgrade_live_baseline.rs. No key read/received, provider request, Cargo or desktop execution by this reviewer. This document reviews the pre-fix candidate; pending fixes require a final source check.

## Must fix before interpreting results

- Current success predicate accepts AgentLoop Ok plus verified artifact/read/write, but Error events can accompany Ok termination. A saved artifact followed by budget/provider failure can become success. Require no terminal/error observations and no auth/quota stop, or report artifact_success and turn_success separately. collector.await.unwrap_or_default suppresses JoinError and can manufacture an empty error list; preserve an explicit failure observation. Both points were reported directly to owner parallel_plan and root.
- Stop auth/quota BEFORE another provider call. Current flag stops only outer sample loop; wrapper must precheck stop because kernel retries may still call the wrapper after sanitized error text no longer classifies as auth/quota. Owner is preparing that fix.
- Keep byte cap and manifest synchronized (owner/root preparing 16384 per sample). Count is a request-body proxy, not complete HTTP payload/token reservation; provider client wire retries remain outside logical-call counts. No hard currency/billing guarantee.

## Limited useful scope

The 12 fixed prompts are real AgentLoop/provider calls using a single synthetic read/write fixture tool. They test simple compact edit, source-summary and document-artifact behavior; they do not test CodeZ editor, WorkZ tasks, actual tool authorization/sandbox, persisted Run/Event, restart, real project continuation or desktop UX. J2 file names are strings in one source.txt, not retrieval across real files. J3 continuation is fresh sample text, not a resumed persistent session. Do not call 12 passed samples complete desktop baseline.

The artifact oracle verifies required/forbidden substrings, with whitespace compact fallback; JSON task additionally parses and compares exact JSON. Others are intentionally weak: required Windows/macOS/Linux/not tested substrings do not associate evidence with each platform; code substring inclusion does not execute JS or rule out incorrect surrounding behavior. Report artifact-oracle success, retain samples for independent manual review, and do not infer semantic correctness beyond those checks. Immutable ID/spec/source hashes distinguish changed test populations.

1..3 repeats means 12..36 observations but only 12 unique synthetic tasks. Report unique tasks, repeat count, observed sample count, per-task outcomes and any early stop separately; fixed task order/repeats are not an independent random population or a confidence claim. Success must not use planned maximum denominator without reporting unattempted samples after stop. No aggregate result currently exists; derive any summary transparently from records.

## Credential and execution boundary

Key is inherited via temporary environment, not command line/config/prompt/tool input; child stdout/stderr are consumed without being printed. Error content exposed to record is bounded classification only; raw provider messages are not persisted. Artifact contains model text but no supplied key in prompt. Root owns injection/removal; this reviewer did not inspect environment values. Child.kill of Cargo is not proven process-tree/provider cancellation, so deadline/log-size limits are operational guards, not billing caps. Spec hash is launcher-derived; Rust currently validates its format rather than recomputing, so source/spec hashes should accompany parent-run evidence, and the wrapper should not accept an unrelated output file as completed success.

Unknown/zero usage is stored as null totals, never zero cost; cost stays null, correctly separating measurement from paid-budget settlement. Existing source declares scope and unverified areas explicitly. No new production/ledger file or test/build was modified/run by this reviewer.


## Final frozen-source re-review

The preceding must-fix list describes the reviewed earlier version, not outstanding defects in the frozen version below. All four reviewed points are now addressed: success requires empty Error-code list, no last_error/stop and no collector failure; JoinError produces collector_failed; fatal auth/quota precheck prevents another provider call; 16384-byte sample cap matches launcher maximum 196608*repeats. Hash validation now uses compiled include_bytes fixture content and a local Node stdin hash helper with credential removed from its environment, comparing both spec and source fingerprints before paid work. The helper runs via hidden-window proc::std_command.

Final SHA-256:
- fixture: 1a9243308dd564364e91ccbdbe46a727cc2cc5f8ad0ed5abfd43a5654977ad7e
- launcher: 85a916c4257f430c08ddfae5547de60d255180b908eab9a645e6b6ae4c6ff225
- report: c22bcc8afc337469ea4118913d2a34fa0b119801d98077367b55dbb1abda5a3b
- report verifier: bba8140a3ccbab4daf1eeaca35ba7513cb522fbf22edbfc28cdba46fd8a870e2

Report script checks fixture/config fingerprint equality, unique task/repeat pairs, contradictory successful records and integer known usage/latencies. It separates planned/attempted/unattempted samples, unique tasks/repeats, statuses and per-task outcomes. Empty/unknown usage remains unknown and whole-run billed cost null; known-sample price estimate is explicitly not the provider bill. Independently read official https://api-docs.deepseek.com/quick_start/pricing/ on 2026-10-09: Flash peak cache-miss input 0.3 USD / million and output 1.2 USD / million agree with report; cache/time can lower actual fees. Estimate covers recorded known samples only, never unknown observations.

Root reports Node verifier exit 0 covering partial population, duplicate, error-success contradiction, source hash mismatch and unknown usage; this reviewer read fixtures but did not rerun them. No-run compile 85757 underway at review time, no compile result claimed. No actual paid results/artifacts existed in this review yet; they require independent post-run inspection. Establishing an honest baseline need not mean all samples pass; it also cannot auto-pass desktop or MB Gate acceptance. Remaining oracle and scope limitations above stand.
