# AZ025 local intent and atomic audit candidate

2026-10-09. Implemented by the Codex parallel_plan agent; initial domain/export reviewed by remote_tests, final additive audit/schema/accessor changes independently reviewed by the parent Codex. No real external action, provider query, publication or desktop interaction was performed.

Final external_action.rs SHA256: 2f078b14d753304d08dc2b200ca3d131382d96a64f56a255fd728e0d83ce3a16. lib.rs export SHA256: fec62d7ea1792066bf8c45fc30de3247c989af2b22b4ae431579571f50cc1b53.

The independent namespace opens an existing Run database without CREATE or implicit repair. Immutable task/action/provider/target/payload identity is persisted before dispatch. Immediate start validates current Running attempt and revision, commits Started plus an audit event before returning a private, non-Clone permit. Restart and duplicate start cannot recreate a permit. A scope-only accessor exposes immutable identity without duplicating authorization. Unknown/Started requires trusted reconciliation, never automatic replay. Ordinary transport errors do not prove NoEffect. Capability/binding-version freshness and matching the actual outgoing request remain the trusted host adapter's responsibility.

Parent reviewed final audit(): bounded stable action digest plus revision identity, minimal digest/state/version payload, captured authoritative stream, events::append within the same transaction as prepare/start/update, before commit. Audit failure rolls the action mutation back; Existing creates no event. Final layout explicitly requires real tables and exact column/PK/version metadata; privileged arbitrary database tampering is not claimed prevented. No event schema initialization/repair is performed by mutations. Missing event namespace, revision overflow and view replacement fail closed.

Actual execution:

- Session 70580: cargo test -p agentz-task-state --offline --locked external_action -- --test-threads=1; first slice 8 passed, 0 failed, 0 ignored, 0.52s. This result alone does not validate subsequent changes.
- Session 21246: cargo test -p agentz-task-state --offline --locked --lib -- --test-threads=1; final source 134 passed, 0 failed, 0 ignored, 410.16s; includes 12 external_action tests and existing task/Run/event/plan/approval tests. Compilation finished in 26.82s.
- Session 51264: cargo clippy -p agentz-task-state --offline --locked --all-targets -- -D warnings; exit 0, 17.43s.

Fixtures cover two-connection single-permit contention, immutable-key conflict, old attempt rejection, in-process synthetic service effect then lost response/reopen/no replay, explicit confirmation/no-effect, completion SQL rollback, missing/future/partial/corrupt/view namespace, readonly unchanged bytes, bounded audit payload/count, start/completion event INSERT rollback, revision exhaustion and missing event namespace without repair. The controlled service is a local counter, not HTTP or a real external provider.

This is a reviewed opt-in domain candidate. No host provider adapter/IPC, startup owner proof, renderer authority, real provider receipt verification, distributed exactly-once guarantee, GUI or installed package acceptance is delivered. AZ025 remains preparing and formal dependencies/Gates remain unchanged. The existing 0.9.7 installer predates this implementation.
