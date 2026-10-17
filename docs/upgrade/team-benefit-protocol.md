# AZ-075 matched multi-Agent benefit protocol (preparation only)

This protocol freezes the comparison before any model run. It does not report a result or authorize team mode by default. Do not start the experiment until AZ-071 provides a single team-owned budget with member reservations and cancellation settlement, and AZ-062 can retain per-run quality, latency, cost, and intervention records.

## Frozen experiment card

Record a versioned card with the candidate build commit and binary SHA-256, model/provider/endpoint and settings, tool permissions, task/environment snapshot hashes, task IDs and acceptance oracles, run order, repeat count, timeout, total token/active-time/concurrency limits, price source and date, evaluator identity, and stop rules. Use the same card for all three arms. Keep credentials and private task material outside the card. A changed build, task fixture, oracle, model setting, permission set, or budget creates a new card and cannot be pooled with earlier runs.

For each task and repeat, start from a restored identical environment snapshot. Run all three arms: `single` (one Agent), `read_only_parallel` (one writing Agent with parallel read-only members), and `implementation_team` (members may implement within the AZ-070 integration and AZ-071 budget contracts). Rotate arm order across repeats to reduce order effects. Assign a unique task attempt and environment identity to every run; never reuse mutable output from another arm. Record whether the arm was unavailable and why instead of silently dropping its sample.

## Per-run receipt

The receipt must contain `card_sha256`, `task_id`, `repeat`, `arm`, `attempt_id`, `environment_snapshot_sha256`, `build_sha256`, start/end timestamps, elapsed milliseconds, task-oracle result and evidence locator, failure class, known input/output tokens and unknown-usage count, actual billed cost when available or explicitly `null`, price-estimate method and date, logical calls, wire calls if verified, manual intervention count and minutes, member count, peak concurrency, cancellation/settlement status, and redacted trace locator. A successful process exit alone is not task success. A missing oracle, lost result, unsettled member, or unknown output is an incomplete sample, not a pass. Record retries and human fixes against the original attempt.

## Comparison and decision

Check that every planned `(task_id, repeat, arm)` has one receipt or an explicit unattempted reason. Report success rate with attempted and planned denominators, per-task outcomes, elapsed p50/p95, known and unknown usage, billed cost coverage and estimates separately, intervention totals, and failure attribution for each arm. Show paired per-task deltas against `single`; do not infer a benefit from aggregate speed alone when success or intervention worsens. Keep desktop and model observations distinct from synthetic/kernel fixtures. Report the number of independent task fixtures and repeats; do not present repeats of one fixture as independent tasks.

The default policy remains single-Agent unless the reviewed paired evidence shows a useful improvement for a named task class without unacceptable quality regression, budget breach, unsettled cancellation, or extra human intervention. An arm with unknown cost cannot support a cost-benefit claim. Reviewers record the scenario-specific default recommendation and confidence limits after actual authorized runs. This document supplies no acceptance result, threshold waiver, or Gate decision.
