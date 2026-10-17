# AZ036 host registration independent review

2026-10-09. Read-only review, no Cargo/GUI or production/ledger mutation by this reviewer.

Current commands/mod.rs adds pub mod task_view and lib.rs generate_handler adds commands::task_view::task_view_resolve. These registration-only additions leave existing AZ017 Run/approval and AZ035 details contracts, initialization and command handlers unchanged. They expose a new observational command rather than reinterpret existing data as execution permission. Shared-source fingerprints change and require parent review-history/manifest refresh; this narrow review does not claim unchanged whole-file hashes.

New entry accepts only opaque scope_handle/session_id, captures environment execution lease before main-window scope lookup, obtains existing_binding via supported session metadata, captures registered environment read-only, resolves actual local workspace and verified execution binding asynchronously, compares PlanBinding, then rechecks opaque scope within with_verified_scope before describe. describe rereads captured metadata/environment and verifies canonical original project/task/workspace identity. No renderer project/workspace path, Run creation or model execution is authorized. Returned observation_only remains true; it is not an IDE capability. No blocking scope/registration defect found in this limited inspection.

Important existing resolver limitation: approval::resolve_binding calls agentz_task_state::load at line594 and underlying resolve_task_workspace, so entire asynchronous command is not proven disk-readonly; environment registry migration/initialization behavior may still occur through shared resolver. Describe/read_existing fixtures prove their own readonly/no-Run path, not the full host async entry or native scope revocation race. Parent was notified; do not claim global no-write behavior. Environment lease prevents normal cooperative environment mutation during resolution but is not adversarial filesystem atomicity.

New handler does not enable terminal/debug/Send for a returned path. Full GUI/same-task navigation, dirty-buffer protection, ongoing native capability revalidation, managed resolver end-to-end and broader platform acceptance remain unverified. No existing formal Gate auto-pass is inferred.

Fingerprints:
- src-tauri/src/commands/mod.rs: 9640c9cd2ccb5928133a66a90f84500b39090635d8ef61ebb3fa7027ea6b8020
- src-tauri/src/lib.rs: 342dd35b34f6f9a2e53749dfde56fee1a38ac51c22d55c54ab2801e477e467d5
- src-tauri/src/commands/task_view.rs: b9ee3d928e4864f2f51d2dd9592a7616c0f17fd9479854294d3e2b146d62b1e7
