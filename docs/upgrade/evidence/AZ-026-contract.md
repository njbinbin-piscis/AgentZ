# AZ026 fixed kernel compatibility candidate

2026-10-09. Consume only the current independently reviewed AZ022 tool-recovery and AZ023 retry contracts. Formal upstream acceptance/M1 and public release remain pending. This task prepares a local versioned source/compatibility artifact; it does not publish an upstream tag or package.

The existing reconstruction script covers an earlier partial overlay chain. It cannot reproduce the entire current product vendor and must not be presented as doing so. Prepare a separate immutable-base-to-current-vendor overlay, verify actual forward application and reverse restoration across the complete file set, pin every source and Cargo integration input, and preserve a relocatable archive. Baseline is immutable commit 527e55496caf4437e4265e02869f0a8c764549f6 (v0.8.68); package manifest version can differ from this upstream tag and is reported explicitly.

Snapshotting current vendor makes its contents reviewable and reproducible, not automatically reviewed or released. No sibling uncommitted files may be used as implementation input: sibling Git is only a provider of the fixed baseline object. No global Git settings or user files are changed. A source change while freezing must fail validation. The artifact must record actual source SHA256, overlay SHA256 and archive SHA256, and distinguish integration checks from GUI/platform/install/rollback acceptance.

Primary implementation: a separate freeze/verify script and artifact, existing Cargo.lock/pinned local vendor compatibility audit, and targeted current fixed-kernel integration checks. New installation packaging follows only a final reviewed source freeze; the earlier 0.9.7 installer remains an older candidate.
