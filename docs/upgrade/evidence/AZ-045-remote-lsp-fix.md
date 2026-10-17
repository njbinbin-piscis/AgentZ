# AZ-045 remote LSP bridge lifecycle candidate

Implemented on 2026-10-07 in `src-tauri/src/remote/lsp.rs`. Existing working-tree formatting changes were retained. No other implementation files or plan ledger were edited by this agent.

## Finding and change

The bridge previously did not start reading child stdout until a WebSocket client completed its handshake. A language server that exited before that point left the bridge awaiting a client, its session `done` flag false, and a dead bridge port eligible for reuse. After connection, waiting solely for the shared output channel to close also allowed a surviving stderr reader to keep the bridge alive after protocol stdout terminated. The original output sender is moved into the stderr reader rather than retained independently; sender ownership alone was not the entire cause.

The bridge now starts its stdout/stderr readers immediately and observes stdout task termination while accepting/handshaking and while serving the client. A scoped `BridgeReaders` guard aborts both tasks on every return path, including handshake errors and cancellation. Returning from the bridge reaches the existing production wrapper that marks the session done; a future `start` removes that retired session instead of returning its dead port.

## Verification status

Single-file `rustfmt --edition 2021 src-tauri/src/remote/lsp.rs` passed. Cargo compilation and tests have **not** been run by this agent; the primary agent will run the combined host checks.

Two new non-ignored controlled local process tests spawn a child waiting on stdin (`powershell.exe` on Windows, `sh` on Unix), kill only that owned child, and require bridge completion, `done = true`, and refusal of new TCP connections to the listener. One has no WebSocket client; the other completes a real WebSocket handshake before terminating the child. They are named `child_exit_before_client_retires_listener` and `child_exit_with_client_retires_listener`.

These tests do not establish real SSH network loss, changed-host-key handling, desktop Monaco recovery, actual language-server restart, or acceptance on other platforms. Session-map concurrent starts and complete process-tree cleanup are outside this isolated change. Stdout termination is treated as connection termination even if stderr remains open; buffered messages still pending in the bridge may be discarded when the connection ends.
