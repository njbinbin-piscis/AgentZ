# AZ-045 remote file command binding candidate

Implemented on 2026-10-07 in `src-tauri/src/commands/remote.rs`; existing formatting edits retained. Only this implementation file and this evidence file were edited by the assigned agent.

`remote_fs_list` and `remote_fs_action` now bind a `RemoteSession` from the original URI, retain its resolved path and authority, and invoke `session.call`. They no longer resolve a path and subsequently consult the manager's current connection. A connection generation change therefore rejects the request through the shared generation-bound write contract rather than targeting a replacement host.

Rename URI destinations are decoded and checked against the source session authority without waiting for or consulting another connection. Another authority or a malformed remote URI is rejected. Plain absolute/relative destination paths retain the existing server protocol semantics. Listing results retain the bound authority and original sorting behavior.

Three new tests cover same-authority URI encoding/case equivalence and plain paths, another authority, and malformed scheme/empty authority/encoded authority separator/NUL path boundaries. They test destination validation; the shared ExtHost generation tests establish stale-session rejection. A real command-to-process authority-switch scenario is not established by these helper tests.

Verification: single-file rustfmt passed. Cargo build/tests were not run by this agent; the primary agent owns combined verification. No SSH, GUI, deployment, or acceptance completion was performed.

Integration note: `remote::bind` uses the registered global manager, matching other newly migrated IDE entry points. Its first-registered-manager contract remains unchanged. If the application gains multiple AppState/manager instances in one process, the global binding API needs explicit manager injection or identity validation; this candidate does not claim correctness for that future architecture. Legacy path-only `read_remote_text`/`write_remote` helpers remain outside this assigned list/action change and should not be used for URI-origin operations without a retained session.
