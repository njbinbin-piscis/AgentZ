# AZ019 limited domain implementation review for AZ025

2026-10-09. Independent source review of recovery.rs, its Run/event transaction interfaces and existing evidence. This is a candidate code dependency review, not AZ019 completion or startup/owner Gate approval.

Reviewed recovery fingerprint SHA256 c420a768911ee951076c656f0613cd841737b27d6df4e70977700287cca45c91. Related run.rs ea4b9f4e701e2d623c690d6356a43cb9d0c9b20d6a0daa9c9b91a3ae936c01d6; events.rs 7fcfa29dc35ab483caa641d86a6b702de09b155fcebd01b6c68f4e7db7c5fe98; lib.rs a45481fc4e26e9394408b46a70d819a60707e6dc00919e9a79cf473670eb963b. Root must refresh these if source changes before recording review.

The verified domain preserves Alive/Unverifiable and requires explicitly trusted ConfirmedGone evidence; it never infers owner death from PID/age/startup. Binding/project, active expected state, current attempt and exact updated timestamp are validated under one Immediate transaction. Run and Attempt interruption and durable event append commit together; failed event insertion rolls state back. Existing database only, missing durable namespace rejected, no automatic execution/tool replay or retry.

Actual existing evidence AZ-019-recovery-integration.md records root session99087: six real SQLite fixtures passed, 0 failures, 0 ignored, 0.08s. The initial38615 fixture setup failure is explicitly retained. Fixtures cover preserved bytes, atomic interruption/reopen/no retry, competing connections, event failure rollback, stale/wrong scope/corrupt history and missing database/namespace. This reviewer did not rerun Cargo. No new test success is invented.

Suitable AZ025 dependency: the already tested rule that unknown side effects require explicit authoritative reconciliation, captured immutable scope and atomic durable state. AZ025 need not call reconcile() to issue external actions and must not manufacture ConfirmedGone. Run interruption does not prove an external service failed or permit resending.

Still outside this review: persistent cross-process owner registry/OS lock, startup scanner, real killed-process/restart journey, GUI, remote process proof, budget/approval release or rollback. Current open checks namespace presence, with append validating event schema before committed interruption; this does not amount to a global event-store audit on Alive/Unverifiable. No P1 issue found within the bounded six-fixture contract. Host owner claims remain explicitly unverified.
