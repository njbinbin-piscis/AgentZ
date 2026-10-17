# AZ-004 completed live observations independent artifact review

2026-10-09. Read all 36 recorded artifacts, 12 distinct tasks x 3 fixed-order repeats. No API call, Cargo, credential access or harness/original results modification. Results SHA-256 c75a779d64e79f7d2c572ec3c48889ec4feb21063fd75adbdcc005be64bab88a.

## Original automatic metric (unchanged)

33/36 artifact-oracle/sample successes, 3/36 failures, all J2-04 repeats. No recorded provider Error codes, timeout or collector failure; 108 logical calls, 45599 recorded input tokens and 8175 output tokens; zero unknown-usage responses. Cost remains null and actual invoice/wire retries unverified. A successful runner process does not replace these per-sample results.

## Independent adjudication (separate from automatic metric)

- J1-01/02/03 all 9 artifacts executed in fresh Node vm contexts with no injected fs/process/network bindings, 100ms script timeout. Only reviewed tiny source was evaluated, removing export keyword. add passed (2,3),(-2,3),(0,0),(1.5,2.5); eligible passed 17/18/19 boundary; items exactly red,blue,green. This is finite-input behavior evidence, not general JS correctness or an adversarial vm security boundary.
- J1-04 all 3 artifacts parse to exactly enabled:true,timeout:30 with two keys.
- J2-01 all 3 accurately map module-a.ts/start and module-b.ts/finish; J2-02 retain cited retry3 and TTL60 seconds; J2-03 each separately labels Windows tested and macOS/Linux untested without broader support claims.
- J2-04 all 3 explicitly state observed timeout, unknown cause and trace.md attribution. Repeat1 says “No network failure is asserted”; repeats2/3 similarly negate network failure or any identified cause. The forbidden substring network failure mechanically triggers failure even in negated statements. For these three exact observed artifacts, semantic adjudication finds the requested uncertainty preserved; this is an oracle false-negative, not a reason to edit original records or globally dismiss all failures.
- J3-01 all 3 checklist artifacts preserve Ada/Friday/draft; J3-02 retain Local report/synthetic/unknown cost and explicitly avoid zero; J3-03 retain the exact HUMAN NOTE plus Risks section; J3-04 retain offline goal/overview and add backup instructions. These are fresh synthetic text continuations, not real persisted-session recovery.

Limited independent semantic adjudication: 36/36 meet these small fixture content requirements, including the 3 separately explained oracle disagreements. Original automatic metric stays 33/36. No complex code execution, real repository research or desktop task was inferred. Function checks actually ran via .cache/az004-artifact-review.cjs, exit0; original artifact bytes were read only. No U+FFFD replacement character appeared in the 36 UTF-8 artifact reads.

## Acceptance boundary

This establishes the first fixed real-kernel/provider synthetic baseline, including failures and oracle weaknesses. It does not pass desktop CodeZ/WorkZ, MB Gate, native sandbox, restart/continuation, permission flow, billed settlement or cross-platform acceptance. Three repeated identical/simple code artifacts are not independent randomized evidence. Future oracle improvements require a new pinned baseline/version rather than rewriting this run. Actual model latency and recorded usage should be reported with null cost/remaining unverified fields; a peak known-usage estimate may be quoted separately from actual billing.


Root reports paid process 94060 exit0 after all36 observations, p50 3271ms/p95 4468ms and known-usage peak-rate estimate USD0.0234897; these are kernel sample latency/estimate, not UI or invoice. The reviewed JSON includes model/endpoint/thinking/OS/arch/source identity, per-sample result/time/artifact/usage, null cost and explicit unverified list. This meets the limited AZ004 first-baseline evidence delivery criterion; it does not itself authorize or pass a milestone Gate. Next-v2 follow-up: use assertion-aware semantic tests for negated causal claims; preserve this v1 population and original metrics.
